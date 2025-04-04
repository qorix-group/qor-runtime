// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use core::task::Context;
use std::{rc::Rc, sync::Arc};

use crate::scheduler::waker::create_waker;
use foundation::base::fast_rand::FastRand;
use foundation::containers::spmc_queue::BoundProducerConsumer;
use foundation::prelude::*;
use foundation::threading::thread_wait_barrier::ThreadReadyNotifier;

use super::context::ctx_get_worker_id;
use super::{
    context::{ctx_initialize, ContextBuidler},
    scheduler::Scheduler,
    task::async_task::TaskRef,
    worker_types::*,
};

// When a worker's local queue is empty and it takes work from the global queue, this is the number
// of tasks that are taken in one batch.
const TAKE_GLOBAL_WORK_SIZE: usize = 10;

// The facade to represent this in runtime
pub(super) struct Worker {
    // I need to expose
    // state - searching, idling etc
    // weakup api to notify that i can steal
    thread_handle: Option<std::thread::JoinHandle<()>>,
    id: u8,
    engine_id: u8,
}

#[derive(PartialEq)]
enum LocalState {
    Searching,
    Executing,
    Sleeping,
}

// the actual impl
struct WorkerInner {
    own_interactor: WorkerInteractor,
    producer_consumer: Rc<BoundProducerConsumer<TaskRef>>,
    scheduler: Arc<Scheduler>,

    local_state: LocalState, // small optimization to not touch global atomic state if we don't  really need
    worker_type: WorkerType,
    randomness_source: FastRand,
}

///
/// Async Worker implementation
///
/// TODO:
///     - shutdown
///     - join logic
///     - prio & affinity
///     - migrate to iceoryxbb2 once we know details
///     - ....
///
///
impl Worker {
    pub(super) fn new(prio: Option<u16>, engine_id: u8, id: u8) -> Self {
        Self {
            thread_handle: None,
            id,
            engine_id,
        }
    }

    pub fn start(&mut self, scheduler: Arc<Scheduler>, ready_notifier: ThreadReadyNotifier) {
        self.thread_handle = {
            let worker_type = WorkerType::EngineId((self.engine_id, self.id));
            let interactor = scheduler.worker_access[self.id as usize].clone();
            let id = self.id as u64;

            // Entering a thread
            Some(
                std::thread::Builder::new()
                    .name(format!("runtime_worker_{}", self.id))
                    .spawn(move || {
                        let prod_consumer = interactor.steal_handle.get_boundedl().unwrap();

                        let internal = WorkerInner {
                            own_interactor: interactor,
                            local_state: LocalState::Executing,
                            scheduler: scheduler.clone(),
                            worker_type: worker_type,
                            producer_consumer: Rc::new(prod_consumer),
                            randomness_source: FastRand::new(82382389432984 / (id + 1)), // Random seed for now as const
                        };

                        Self::run_internal(internal, ready_notifier);
                    })
                    .unwrap(),
            )
        };
    }

    fn run_internal(mut worker: WorkerInner, ready_notifier: ThreadReadyNotifier) {
        worker.pre_run();

        // Let the engine know what we are ready to handle tasks
        ready_notifier.ready();

        worker.run();
    }
}

impl WorkerInner {
    fn pre_run(&mut self) {
        let builder = ContextBuidler::new()
            .thread_id(0)
            .with_handle(self.producer_consumer.clone(), self.scheduler.clone())
            .with_worker_type(self.worker_type);

        // Setup context
        ctx_initialize(builder);

        self.own_interactor.state.transition_to_executing();
    }

    fn run(&mut self) {
        loop {
            let (task_opt, should_notify) = self.try_pick_work();

            if let Some(task) = task_opt {
                self.transition_to_execution();

                if (should_notify) {
                    self.scheduler.try_notify_siblings_workers(self.get_worker_id());
                }

                let waker = create_waker(task.clone());
                let mut ctx = Context::from_waker(&waker);
                match task.poll(&mut ctx) {
                    super::task::async_task::TaskPollResult::Done => {
                        // Literally nothing to do ;)
                    }
                    super::task::async_task::TaskPollResult::Notified => {
                        // For now stupid respawn
                        self.scheduler.spawn_from_runtime(task, &self.producer_consumer);
                    }
                }
            } else {
                let mut _guard = self.own_interactor.mtx.lock().unwrap();

                // Scheduler can only move us to searching state under this lock, which means if we are here, there are no spurious changes of state, we can execute logic
                let sleeping_approved = self.try_transition_to_sleeping();

                if !sleeping_approved {
                    continue; // For now simply loop again as apparently we were decided to be woken up
                }

                self.local_state = LocalState::Sleeping;

                // Even we decided sleeping, we again check the global queue just in case there is something already, if not, we will not miss it as our state is observer as sleeping already and CV would be notified
                if !self.try_take_global_work_internal() {
                    trace!("Worker is entering sleep under cond var!");
                    _guard = self.own_interactor.cv.wait_while(_guard, |added| !*added).unwrap();

                    *_guard = false;

                    // When leaving this place, we are in searching state since scheduler woke us and before it did, it set our state
                    self.local_state = LocalState::Searching;
                    trace!("Worker going out of sleep!");
                }
            }
        }
    }

    fn try_pick_work(&mut self) -> (Option<TaskRef>, bool) {
        // First check our queue for work
        let mut task = self.producer_consumer.pop();
        if task.is_some() {
            return (task, false);
        }

        // Now we enter searching if there is no enough contention already. We use also local state to not need to do atomic operations if ie. we were already moved to searching by scheduler
        let res = (self.local_state == LocalState::Searching) || self.scheduler.try_transition_worker_to_searching(&self.own_interactor);

        if !res {
            trace!("Decided to not steal and sleep!");
            return (None, false); // Seems there is enough workers doing contended access, we shall sleep
        }

        self.local_state = LocalState::Searching;

        // Next, try steal from other workers. Do this only, if no more than half the workers are
        // already searching for work.
        let mut should_notify = false;

        (task, should_notify) = self.try_steal_work();
        if task.is_some() {
            return (task, should_notify);
        }

        // Next, check global queue
        (task, should_notify) = self.try_take_global_work();
        if task.is_some() {
            return (task, should_notify);
        }

        (None, false)
    }

    fn try_steal_work(&mut self) -> (Option<TaskRef>, bool) {
        let current_worker = ctx_get_worker_id() as usize;

        let start_idx = self.randomness_source.next() as usize;
        let cnt = self.scheduler.worker_access.len();

        let mut stolen = 0;

        // Start from random worker
        for idx in 0..cnt {
            let real_idx = (start_idx + idx) % cnt;

            if real_idx == current_worker {
                continue;
            }

            let res = self.scheduler.worker_access[real_idx]
                .steal_handle
                .steal_into(&self.own_interactor.steal_handle, None);

            stolen += res.unwrap_or_default();
        }

        trace!("Stolen {:?}", stolen);
        (self.producer_consumer.pop(), stolen > 1)
    }

    //
    // Tries to take  TAKE_GLOBAL_WORK_SIZE `TaskRef` items from the global_queue into the local task queue. Returns
    // the first `TaskRef` if that did work, or None if that did not work or the global_queue lock
    // could not be acquired.
    //
    // NOTE: This is currently double copying: 1. From global_queue into `mem` here and 2. From
    // `mem` to local_queue. Maybe we can optimize this in the future.
    //
    fn try_take_global_work(&self) -> (Option<TaskRef>, bool) {
        if self.try_take_global_work_internal() {
            (self.producer_consumer.pop(), true)
        } else {
            (None, false)
        }
    }

    fn try_take_global_work_internal(&self) -> bool {
        let mut mem: [Option<TaskRef>; TAKE_GLOBAL_WORK_SIZE] = [const { None }; TAKE_GLOBAL_WORK_SIZE];
        let capacity = (self.producer_consumer.capacity() as usize).min(mem.len());
        let mut slice = &mut mem[0..capacity];
        self.scheduler.global_queue.pop_slice(&mut slice);
        // TODO: Add something like push_many to producer_consumer to avoid many atomics here and if there is 1 >= skip first elem so you can return it directly

        let mut cnt = 0;

        for item in mem {
            if let Some(item) = item {
                self.producer_consumer.push(item, &self.scheduler.global_queue);
                cnt += 1;
            } else {
                break;
            }
        }

        trace!("Taken from global queue {}", cnt);
        cnt > 0
    }

    ///
    /// X -> Sleeping can be done only from worker
    ///
    fn try_transition_to_sleeping(&self) -> bool {
        let res = self.own_interactor.state.transition_to_sleep();

        if self.local_state == LocalState::Searching {
            self.scheduler.num_of_searching_workers.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }

        // For now we don't need anything more, let see later
        res
    }

    ///
    /// Searching -> Executing can be done only from worker
    ///
    fn transition_to_execution(&mut self) {
        if self.local_state != LocalState::Executing {
            if self.local_state == LocalState::Searching {
                self.scheduler.num_of_searching_workers.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            }

            self.own_interactor.state.transition_to_executing();
            self.local_state = LocalState::Executing;
        }
    }

    fn get_worker_id(&self) -> Option<usize> {
        if let WorkerType::EngineId((_, worker_id)) = self.worker_type {
            return Some(worker_id as usize);
        }

        None
    }
}
