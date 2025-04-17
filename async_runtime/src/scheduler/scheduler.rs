// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{scheduler::context::ctx_get_worker_id, TaskRef};

use super::{worker::Worker, worker_types::*};
use foundation::{
    containers::{mpmc_queue::MpmcQueue, spmc_queue::BoundProducerConsumer},
    prelude::*,
};

pub(crate) const SCHEDULER_MAX_SEARCHING_WORKERS_DIVIDER: u8 = 2; // Tune point: We allow only half of worker to steal, to limit contention

pub(crate) struct Scheduler {
    pub(super) worker_access: Box<[WorkerInteractor]>,

    // Hot path for figuring out if we shall wake-up someone, or we shall go to sleep from worker
    pub(super) num_of_searching_workers: IoxAtomicU8,

    pub(super) parked_workers_indexes: std::sync::Mutex<std::vec::Vec<usize>>,

    pub(super) global_queue: MpmcQueue<TaskRef>,
}

impl Scheduler {
    ///
    /// Spawns from runtime directly
    /// This means we can push into some local queue
    ///
    pub(crate) fn spawn_from_runtime(&self, task: TaskRef, local_queue: &BoundProducerConsumer<TaskRef>) {
        match local_queue.push(task, &self.global_queue) {
            Ok(_) => {
                let current_worker = ctx_get_worker_id() as usize;
                self.try_notify_siblings_workers(Some(current_worker));
            }
            Err(_) => {
                // TODO: Add error hooks so we can notify app owner that we are done
                panic!("Cannot push to queue anymore, overflow!");
            }
        }
    }

    ///
    /// Spawns task outside of runtime.
    /// Currently we use IO Thread that can have significant traffic on its side and we need to make sure that the events from here are processed ASAP.
    /// This requires to wake-up even without checking how much threads is stealing. The above is suboptimal but deliver smaller latency.
    /// TODO: This part of code has to be revised once we implement real IO handling with time/timers handling
    ///
    ///
    pub(crate) fn spawn_outside_runtime(&self, task: TaskRef) {
        if self.global_queue.push(task) {
            self.try_notify_siblings_worker_unconditional(None);
        } else {
            // TODO: Add error hooks so we can notify app owner that we are done
            panic!("Cannot push to global queue anymore, overflow!");
        }
    }

    ///
    /// Tries to move worker to searching state if conditions are met. No more than half of workers shall be in searching state to avoid too much contention on stealing queue
    ///

    pub(super) fn try_transition_worker_to_searching(&self) -> bool {
        let searching = self.num_of_searching_workers.load(std::sync::atomic::Ordering::SeqCst);
        let predicted = (searching * SCHEDULER_MAX_SEARCHING_WORKERS_DIVIDER) as usize;

        if predicted >= self.worker_access.len() {
            return false;
        }

        let v = self.num_of_searching_workers.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        true
    }

    pub(super) fn transition_worker_to_executing(&self) {
        self.num_of_searching_workers.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub(super) fn transition_to_parked(&self, was_searching: bool, index: usize) -> bool {
        let mut guard = self.parked_workers_indexes.lock().unwrap();

        let mut num_of_searching = 2; //2 as false condition

        if was_searching {
            num_of_searching = self.num_of_searching_workers.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        }

        guard.push(index);
        num_of_searching == 1
    }

    pub(super) fn transition_from_parked(&self, index: usize) {
        let mut guard = self.parked_workers_indexes.lock().unwrap();

        guard.retain(|&x| x != index);
    }

    ///
    /// Private
    ///

    pub(crate) fn try_notify_siblings_workers(&self, current_worker: Option<usize>) {
        if !self.should_notify_some_worker() {
            trace!("Too many searchers while scheduling, no one will be woken up!");
            return; // Too much searchers already, let them find a job
        }

        self.try_notify_siblings_worker_unconditional(current_worker)
    }

    fn try_notify_siblings_worker_unconditional(&self, _: Option<usize>) {
        let guard = self.parked_workers_indexes.lock().unwrap();
        let index_opt = guard.first();

        if let Some(index) = index_opt {
            trace!("Notifying worker at index {} to wakeup", *index);
            self.worker_access[*index].unpark();
        } else {
            // No one is sleeping but no one is searching, means they are before sleep. For now we simply notify all (as we only use syscall once someone really sleeps)
            for w in &self.worker_access {
                w.unpark();
            }
        }
    }

    //
    // A worker should be notified only if no other workers are already in the searching state.
    //
    fn should_notify_some_worker(&self) -> bool {
        self.num_of_searching_workers.fetch_sub(0, std::sync::atomic::Ordering::SeqCst) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheduler_new(workers_cnt: usize, local_queue_size: usize) -> Scheduler {
        // artificially construct a scheduler
        let mut worker_interactors = Box::<[WorkerInteractor]>::new_uninit_slice(workers_cnt);
        let mut queues: Vec<TaskStealQueue> = Vec::new(workers_cnt);

        for i in 0..workers_cnt {
            let c = create_steal_queue(local_queue_size);

            queues.push(c.clone());

            unsafe {
                worker_interactors[i].as_mut_ptr().write(WorkerInteractor::new(c));
            }
        }

        let global_queue = MpmcQueue::new(32);
        Scheduler {
            worker_access: unsafe { worker_interactors.assume_init() },
            num_of_searching_workers: IoxAtomicU8::new(0),
            parked_workers_indexes: std::sync::Mutex::new(vec![]),
            global_queue,
        }
    }

    #[test]
    fn should_transition_to_searching_test() {
        // scheduler with one worker and a queue size of 2
        let scheduler = scheduler_new(1, 2);
        // if no worker is in searching state one worker should steal work
        assert!(scheduler.try_transition_worker_to_searching());
        // if our one and only worker is in searching state no other worker should steal work
        // transition from searching to searching should fail
        assert!(!scheduler.try_transition_worker_to_searching());

        // scheduler with two workers and a queue size of 2
        let scheduler = scheduler_new(2, 2);

        assert!(scheduler.should_notify_some_worker());
        // if no worker is in searching state one worker should steal work
        assert!(scheduler.try_transition_worker_to_searching());

        assert!(!scheduler.should_notify_some_worker());
        // if one worker is in searching state, half workers are searching, so the other one should
        // not transition to searching also

        // scheduler with 10 workers and a queue size of 2
        let scheduler = scheduler_new(10, 2);
        // 0 searching
        assert!(scheduler.should_notify_some_worker());
        assert!(scheduler.try_transition_worker_to_searching());
        // 1 searching
        assert!(!scheduler.should_notify_some_worker());
        assert!(scheduler.try_transition_worker_to_searching());
        // 2 searching
        assert!(!scheduler.should_notify_some_worker());
        assert!(scheduler.try_transition_worker_to_searching());
        // 3 searching
        assert!(!scheduler.should_notify_some_worker());
        assert!(scheduler.try_transition_worker_to_searching());
        // 4 searching
        assert!(!scheduler.should_notify_some_worker());
        assert!(scheduler.try_transition_worker_to_searching());
        // 5 searching
        assert!(!scheduler.should_notify_some_worker());
        assert!(!scheduler.try_transition_worker_to_searching());
    }
}
