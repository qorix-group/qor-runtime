// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ops::Deref;
use std::sync::{self, Arc};

use foundation::containers::spmc_queue::*;
use foundation::prelude::*;

use super::task::async_task::TaskRef;

pub(super) type TaskStealQueue = Arc<SpmcStealQueue<TaskRef>>;
pub(super) type StealHandle = TaskStealQueue;

pub(super) fn create_steal_queue(size: usize) -> TaskStealQueue {
    Arc::new(SpmcStealQueue::new(size as u32))
}

pub(super) const WORKER_STATE_SLEEPING: u8 = 0b00000000;
pub(super) const WORKER_STATE_SEARCHING: u8 = 0b00000001;
pub(super) const WORKER_STATE_EXECUTING: u8 = 0b00000010;
pub(super) const WORKER_STATE_SHUTINGDOWN: u8 = 0b0000011;

#[derive(Copy, Clone, Debug)]
pub(super) enum WorkerType {
    EngineId((u8, u8)),    // 0 - engine id, 1 - worker id
    DedicatedWorkerId(u8), // dedicated worker id
    None,
}

#[derive(Clone)]
pub(crate) struct WorkerInteractor {
    inner: Arc<WorkerInteractorInnner>,
}

impl Deref for WorkerInteractor {
    type Target = WorkerInteractorInnner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

unsafe impl Send for WorkerInteractor {}

pub(crate) struct WorkerState(IoxAtomicU8);

impl WorkerState {
    pub(crate) fn new(val: u8) -> Self {
        Self(IoxAtomicU8::new(val))
    }

    pub(crate) fn get(&self) -> u8 {
        self.0.load(sync::atomic::Ordering::SeqCst)
    }

    ///
    /// Returns true if we went to searching state from any other state, otherwise false. At the end, we are finally in searching state
    ///
    pub(crate) fn transition_to_searching(&self) -> bool {
        WORKER_STATE_SEARCHING != self.0.swap(WORKER_STATE_SEARCHING, sync::atomic::Ordering::SeqCst)
    }

    pub(crate) fn transition_to_executing(&self) -> bool {
        WORKER_STATE_EXECUTING != self.0.swap(WORKER_STATE_EXECUTING, sync::atomic::Ordering::SeqCst)
    }

    ///
    /// Returns Ok() when transition to sleeping happened (it's allowed only from execution), otherwise Err() where value is current state
    ///
    pub(crate) fn transition_to_sleep(&self) -> bool {
        self.0.swap(WORKER_STATE_SLEEPING, sync::atomic::Ordering::SeqCst);
        true
    }
}

pub(crate) struct WorkerInteractorInnner {
    pub(crate) steal_handle: StealHandle,

    pub(super) state: WorkerState,
    pub(super) mtx: std::sync::Mutex<bool>,
    pub(super) cv: std::sync::Condvar,
}

impl WorkerInteractor {
    pub fn new(handle: StealHandle) -> Self {
        Self {
            inner: Arc::new(WorkerInteractorInnner {
                mtx: std::sync::Mutex::new(false),
                cv: std::sync::Condvar::new(),
                steal_handle: handle,
                state: WorkerState::new(WORKER_STATE_EXECUTING),
            }),
        }
    }
}
