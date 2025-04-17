// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use core::types::{box_future, FutureBox};
use std::future::Future;
use std::sync::Arc;

use scheduler::{
    context::{ctx_get_handler, ctx_get_worker_id},
    join_handle::JoinHandle,
    task::async_task::{AsyncTask, TaskRef},
};

pub mod core;
pub mod futures;
pub mod runtime;
pub mod scheduler;
///
/// Spawns a given future into runtime and let it execute on any of configured workers
///
pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
where
    T: Future + 'static + Send,
    T::Output: Send,
{
    let boxed = box_future(future);
    spawn_from_boxed(boxed)
}

///
/// Same as `spawn` but from already boxed future
///
pub fn spawn_from_boxed<T>(boxed: FutureBox<T>) -> JoinHandle<T>
where
    T: 'static,
{
    if let Some(handler) = ctx_get_handler() {
        let task = Arc::new(AsyncTask::new(boxed, ctx_get_worker_id(), handler.scheduler.clone()));
        handler.spawn(task)
    } else {
        panic!("For now we don't allow runtime API to be called outside of runtime")
    }
}
