// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;
use std::time::Duration;

use super::scheduler::*;
use super::task::async_task::TaskRef;
use super::worker::Worker;
use super::worker_types::*;

use foundation::containers::mpmc_queue::MpmcQueue;
use foundation::prelude::*;
use foundation::threading::thread_wait_barrier::ThreadWaitBarrier;

pub struct ExecutionEngine {
    pub(super) workers: Vec<Worker>,

    queues: Vec<TaskStealQueue>,

    id: u8, // Engine ID

    scheduler: Arc<Scheduler>,
}

impl ExecutionEngine {
    pub(crate) fn id(&self) -> u8 {
        self.id
    }

    pub(crate) fn start(&mut self, entry_task: TaskRef) {
        {
            //TODO: Total hack, injecting task before we run any workers so they will pick it
            let pc = self.queues[0].get_local().unwrap();
            pc.push(entry_task, &self.scheduler.global_queue)
                .unwrap_or_else(|e| panic!("Failed to enter runtime while pushing init task"));
        }

        let start_barrier = Arc::new(ThreadWaitBarrier::new(self.workers.len() as u32));

        self.workers.iter_mut().for_each(|w| {
            w.start(self.scheduler.clone(), start_barrier.get_notifier().unwrap());
        });

        debug!("Engine starts waiting for workers to be ready");

        let res = start_barrier.wait_for_all(Duration::new(5, 0));
        match res {
            Ok(_) => {
                debug!("Workers ready, continue...");
            }
            Err(_) => {
                panic!("Timeout on starting engine, not all workers reported ready, stopping...");
            }
        }
    }

    pub(crate) fn get_scheduler(&self) -> Arc<Scheduler> {
        self.scheduler.clone()
    }
}

pub struct ExecutionEngineBuilder {
    workers_cnt: usize,
    queue_size: usize,
    priority: Option<u16>,
}

impl ExecutionEngineBuilder {
    pub fn new() -> Self {
        Self {
            workers_cnt: 1,
            queue_size: 256,
            priority: None,
        }
    }

    pub fn workers(mut self, cnt: usize) -> Self {
        self.workers_cnt = cnt;
        self
    }

    pub fn task_queue_size(mut self, size: usize) -> Self {
        self.queue_size = size;
        self
    }

    pub fn priority(mut self, prio: u16) -> Self {
        self.priority = Some(prio);
        todo!()
    }

    pub(crate) fn build(self) -> ExecutionEngine {
        let mut worker_interactors = Box::<[WorkerInteractor]>::new_uninit_slice(self.workers_cnt);
        let mut queues: Vec<TaskStealQueue> = Vec::new(self.workers_cnt);

        for i in 0..self.workers_cnt {
            queues.push(create_steal_queue(self.queue_size));

            unsafe {
                worker_interactors[i].as_mut_ptr().write(WorkerInteractor::new(queues[i].clone()));
            }
        }

        let global_queue = MpmcQueue::new(32);
        let sched = Arc::new(Scheduler {
            worker_access: unsafe { worker_interactors.assume_init() },
            num_of_searching_workers: IoxAtomicU8::new(0),
            global_queue,
        });

        let mut workers = Vec::new(self.workers_cnt);

        for i in 0..self.workers_cnt {
            workers.push(Worker::new(self.priority, 0, i as u8));
        }

        ExecutionEngine {
            id: 0,
            workers: workers,
            queues: queues,
            scheduler: sched,
        }
    }
}
