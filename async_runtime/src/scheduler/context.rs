// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use core::cell::Cell;
use core::cell::RefCell;
use foundation::containers::spmc_queue::BoundProducerConsumer;

use std::{rc::Rc, sync::Arc};

use crate::core::types::TaskId;
use crate::AsyncTask;
use crate::JoinHandle;

use super::{scheduler::Scheduler, task::async_task::TaskRef, worker_types::WorkerType};

pub struct Handler {
    pub(crate) scheduler: Arc<Scheduler>,
    prod_con: Rc<BoundProducerConsumer<TaskRef>>, // Worker queue producer-consumer object
}

///
/// Implements proxy API between Runtime API and {Scheduler, ...}
///
impl Handler {
    pub(crate) fn spawn<T>(&self, task: Arc<AsyncTask<T>>) -> JoinHandle<T> {
        let handle = JoinHandle::new(task.clone());
        let task_ref = TaskRef::new(task.clone());

        self.scheduler.spawn_from_runtime(task_ref, &self.prod_con);
        handle
    }

    pub(super) fn respawn(&self, task_ref: TaskRef) {
        self.scheduler.spawn_from_runtime(task_ref, &self.prod_con);
    }
}

///
/// This is an entry point for public API that is filled by each worker once it's created
///
pub(crate) struct WorkerContext {
    /// OS Thread ID that the current context was bound too
    thread_id: u64,

    /// The ID of task that is currently run by worker
    running_task_id: Cell<Option<TaskId>>,

    /// WorkerID and EngineID
    worker_type: Cell<WorkerType>,

    /// Access to scheduler and others, mounted in pre_run phase of each Worker
    pub(super) handler: RefCell<Option<Rc<Handler>>>,
}

thread_local! {
    static CTX: RefCell<Option<WorkerContext>> = RefCell::new(None)
}

pub(crate) struct ContextBuidler {
    tid: u64,
    handle: Option<Handler>,
    worker_type: Option<WorkerType>,
}

impl ContextBuidler {
    pub(crate) fn new() -> Self {
        Self {
            tid: 0,
            handle: None,
            worker_type: None,
        }
    }

    pub(crate) fn thread_id(mut self, id: u64) -> Self {
        self.tid = id;
        self
    }

    pub(crate) fn with_worker_type(mut self, t: WorkerType) -> Self {
        self.worker_type = Some(t);
        self
    }

    pub(crate) fn with_handle(mut self, pc: Rc<BoundProducerConsumer<TaskRef>>, s: Arc<Scheduler>) -> Self {
        let handle = Handler { prod_con: pc, scheduler: s };

        self.handle = Some(handle);
        self
    }

    pub(crate) fn build(self) -> WorkerContext {
        WorkerContext {
            thread_id: self.tid,
            running_task_id: Cell::new(None),
            worker_type: Cell::new(self.worker_type.expect("Worker type must be set in context builder!")),
            handler: RefCell::new(Some(Rc::new(self.handle.expect("Handler type must be set in context builder!")))),
        }
    }
}

///
/// Needs to be called at worker thread init, so runtime API is functional from worker context
///
pub(crate) fn ctx_initialize(builder: ContextBuidler) {
    let _ = CTX
        .try_with(|ctx| {
            let prev = ctx.replace(Some(builder.build()));
            if prev.is_some() {
                panic!("Double init of WorkerContext is not allowed!");
            }
        })
        .map_err(|e| {
            panic!("Something is really bad here, error {}!", e);
        });
}

///
/// Returns `Handler` for scheduler or None if not in context.
///
pub(crate) fn ctx_get_handler() -> Option<Rc<Handler>> {
    CTX.try_with(|ctx| ctx.borrow().as_ref()?.handler.borrow().as_ref().cloned())
        .unwrap_or(None)
}

///
/// Sets currently running `task id`
///
pub(crate) fn ctx_set_running_task_id(id: TaskId) {
    let _ = CTX
        .try_with(|ctx| {
            ctx.borrow().as_ref().expect("Called before CTX init?").running_task_id.replace(Some(id));
        })
        .map_err(|e| {
            panic!("Something is really bad here, error {}!", e);
        });
}

///
/// Clears currently running `task id`
///
pub(crate) fn ctx_unset_running_task_id() {
    let _ = CTX
        .try_with(|ctx| {
            ctx.borrow().as_ref().expect("Called before CTX init?").running_task_id.replace(None);
        })
        .map_err(|e| {});
}

///
/// Gets currently running `task id`
///
pub(crate) fn ctx_get_running_task_id() -> Option<TaskId> {
    CTX.try_with(|ctx| ctx.borrow().as_ref().expect("Called before CTX init?").running_task_id.get())
        .unwrap_or_else(|e| {
            panic!("Something is really bad here, error {}!", e);
        })
}

///
/// Worker id bound to this context
///
pub(crate) fn ctx_get_worker_id() -> u8 {
    CTX.try_with(|ctx| {
        let handler = ctx.borrow();
        let b = handler.as_ref().expect("Called before CTX init?");

        let typ = b.worker_type.get();
        match typ {
            WorkerType::EngineId((_, id)) => id,
            WorkerType::DedicatedWorkerId(_) => todo!(),
            WorkerType::None => todo!(),
        }
    })
    .unwrap_or_else(|e| {
        panic!("Something is really bad here, error {}!", e);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_no_init_panic_handler() {
        assert!(ctx_get_handler().is_none());
    }

    #[test]
    #[should_panic]
    fn test_context_no_init_panic_task_id() {
        ctx_get_running_task_id();
    }

    #[test]
    #[should_panic]
    fn test_context_no_init_panic() {
        ctx_get_worker_id();
    }

    #[test]
    #[should_panic]
    fn test_context_no_init_panic_set_task_id() {
        ctx_set_running_task_id(TaskId::new(1));
    }

    #[test]
    #[should_panic]
    fn test_context_no_init_panic_unset_task_id() {
        ctx_unset_running_task_id();
    }
}
