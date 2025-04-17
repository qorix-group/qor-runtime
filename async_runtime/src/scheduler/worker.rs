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

        self.local_state = LocalState::Executing;
        self.own_interactor
            .state
            .0
            .store(WORKER_STATE_EXECUTING, std::sync::atomic::Ordering::SeqCst);
    }

    fn run(&mut self) {
        loop {
            let (task_opt, should_notify) = self.try_pick_work();

            if let Some(task) = task_opt {
                self.run_task(task, should_notify);
                continue;
            }

            self.park_worker();
            self.local_state = LocalState::Executing;
        }
    }

    fn park_worker(&mut self) {
        if self
            .scheduler
            .transition_to_parked(self.local_state == LocalState::Searching, self.get_worker_id().unwrap())
        {
            trace!("Last searcher is trying to sleep, inspect all work sources");

            // we transition ourself but we are last one who is going to sleep, let's recheck all queues, otherwise something may stuck there
            let gc_empty = self.scheduler.global_queue.is_empty();

            if !gc_empty {
                debug!("Unparking during parking due to global queue having work");
                self.scheduler.transition_from_parked(self.get_worker_id().unwrap());
                return;
            }

            for access in &self.scheduler.worker_access {
                if access.steal_handle.count() > 0 {
                    debug!("Unparking during parking due to some steal queue having work");
                    self.scheduler.transition_from_parked(self.get_worker_id().unwrap());
                    return;
                }
            }
        }

        let mut guard = self.own_interactor.mtx.lock().unwrap();

        match self.own_interactor.state.0.compare_exchange(
            WORKER_STATE_EXECUTING,
            WORKER_STATE_SLEEPING_CV,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        ) {
            Ok(_) => {
                debug!("Definite sleep decision");
            }
            Err(WORKER_STATE_NOTIFIED) => {
                // We were notified before, so we shall continue
                self.scheduler.transition_from_parked(self.get_worker_id().unwrap());

                self.own_interactor
                    .state
                    .0
                    .store(WORKER_STATE_EXECUTING, std::sync::atomic::Ordering::SeqCst);
                debug!("Notified while try to sleep, searching again");
                return;
            }
            Err(s) => {
                panic!("Inconsistent state when parking: {}", s);
            }
        }

        loop {
            guard = self.own_interactor.cv.wait(guard).unwrap();

            match self.own_interactor.state.0.compare_exchange(
                WORKER_STATE_NOTIFIED,
                WORKER_STATE_EXECUTING,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.scheduler.transition_from_parked(self.get_worker_id().unwrap());
                    debug!("Woken up from sleep");
                    break;
                }
                Err(_) => {
                    continue; // spurious wake-up
                }
            }
        }
    }

    fn run_task(&mut self, task: TaskRef, should_notify: bool) {
        self.transition_to_executing();

        if should_notify {
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
    }

    fn try_pick_work(&mut self) -> (Option<TaskRef>, bool) {
        // First check our queue for work
        let mut task = self.producer_consumer.pop();
        if task.is_some() {
            return (task, false);
        }

        // Now we enter searching if there is no enough contention already.
        let res = self.try_transition_to_searching();

        if !res {
            trace!("Decided to not steal and sleep!");
            return (None, false); // Seems there is enough workers doing contended access, we shall sleep
        }

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
        let taken = self.try_take_global_work_internal();

        if taken > 0 {
            (self.producer_consumer.pop(), taken > 1)
        } else {
            (None, false)
        }
    }

    fn try_take_global_work_internal(&self) -> usize {
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
        cnt
    }

    fn try_transition_to_searching(&mut self) -> bool {
        let mut res = true;

        if self.local_state != LocalState::Searching {
            res = self.scheduler.try_transition_worker_to_searching();

            if res {
                self.local_state = LocalState::Searching;
            }
        }

        res
    }

    fn transition_to_executing(&mut self) {
        if self.local_state != LocalState::Executing {
            self.scheduler.transition_worker_to_executing();
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
