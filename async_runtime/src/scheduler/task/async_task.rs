// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{core::types::*, ctx_get_handler, scheduler::scheduler::Scheduler};
use core::{ptr::NonNull, task::Context, task::Waker};
use foundation::prelude::*;
use std::cell::UnsafeCell;
use std::mem;

use super::task_state::*;

///
/// Table of pointers to access API of generic `AsyncTask` without a need to know it's type along a way
///
struct TaskVTable {
    ///
    /// Drops one ref count from associated ArcInternal<AsyncTask<T>>
    ///
    drop: unsafe fn(NonNull<TaskHeader>),

    ///
    /// Adds one ref count from associated ArcInternal<AsyncTask<T>>
    ///
    clone: unsafe fn(NonNull<TaskHeader>),

    ///
    /// Reschedules task into runtime
    ///
    schedule: unsafe fn(NonNull<TaskHeader>, TaskRef),

    ///
    ///
    ///
    poll: unsafe fn(this: NonNull<TaskHeader>, ctx: &mut Context) -> TaskPollResult,
}

///
/// Task stage in which we are in
///
pub(crate) enum TaskStage<T, ResultType> {
    Extracted,             // No data available anymore
    InProgress(T),         // Holds "a thing" to execute (could be future, could be fn)
    Completed(ResultType), // Future finished and return value is stored in here
}

///
/// TODO: WIP
///
pub(crate) struct TaskHeader {
    pub(in crate::scheduler) state: TaskState,
    pub(super) id: TaskId,

    vtable: &'static TaskVTable, // API entrypoint to typed task
}

impl TaskHeader {
    pub(crate) fn new<T: 'static>(worker_id: u8) -> Self {
        Self {
            state: TaskState::new(),
            id: TaskId::new(worker_id),
            vtable: create_task_vtable::<T>(),
        }
    }
}

pub enum TaskPollResult {
    Done,
    Notified,
}

///
/// Represents async task with a future. Keep in mind that this can only be accessed via `Task`` trait after construction
///
/// Safety:
///  - has to be repr(C) to make TaskRef work
///
/// Design decision
///    We took a decision for `T` being a `Future Return Type`` and not the Future itself. This is because each Future has different size and `AsyncTask` need to somehow dynamically allocated. Assuming
///    Future is Boxed, `AsyncTask` will have predictable sizes which are easier to map into some simple allocators like MemPool. If we some day device to abandon this, there will be not so much change
///    around here or we can even implement new Task class out of current building blocks and it will fit to whole system seamlessly.
#[repr(C)]
pub(crate) struct AsyncTask<T: 'static> {
    pub(in crate::scheduler) header: TaskHeader,
    stage: UnsafeCell<TaskStage<FutureBox<T>, T>>, // Describe which stage we are in (progress, done, etc)

    handle_waker: UnsafeCell<Option<Waker>>, // Waker for the one that hold the JoinHandle, for now Option, but potentially needs a wrapper for sharing too
    scheduler: ArcInternal<Scheduler>,
}

// We protect *Cells by task state, making sure there is no concurrency on those
unsafe impl<T: 'static> Sync for AsyncTask<T> {}

impl<T> AsyncTask<T> {
    pub(crate) fn new(future: FutureBox<T>, worker_id: u8, scheduler: ArcInternal<Scheduler>) -> Self {
        Self {
            header: TaskHeader::new::<T>(worker_id),
            stage: UnsafeCell::new(TaskStage::InProgress(future)),
            handle_waker: UnsafeCell::new(None),
            scheduler,
        }
    }

    pub(crate) fn id(&self) -> TaskId {
        self.header.id
    }

    pub(crate) fn set_waker(&self, waker: Waker) {
        unsafe {
            *(self.handle_waker.get()) = (Some(waker));
        }
    }

    pub(crate) fn poll(&self, cx: &mut Context) -> TaskPollResult {
        match self.header.state.transition_to_running() {
            TransitionToRunning::Done => {
                // means we are the only one who could be now running this future
                self.poll_core(cx)
            }
            TransitionToRunning::Aborted => {
                // TODO: Add abort handler, but this mean we are basically done for outer level, for now marked as todo!() for final impl
                todo!();
            }
        }
    }

    ///
    /// Safety: outer caller needs to ensure that the stage is synchronized by task state
    ///
    fn poll_core(&self, ctx: &mut Context) -> TaskPollResult {
        // Once we are here, we are the only one who can be running this future and/or stage itself

        let ref_to_stage = unsafe { &mut *self.stage.get() };

        match ref_to_stage {
            TaskStage::Extracted => {
                panic!("We shall never be in TaskStage::Extracted in poll as we handle that before (task state == completed)")
            }
            TaskStage::InProgress(ref mut future) => {
                // Lets poll future
                match future.as_mut().poll(ctx) {
                    std::task::Poll::Ready(ret) => {
                        // Store result
                        *ref_to_stage = TaskStage::Completed(ret);

                        let status = self.header.state.transition_to_completed();
                        match status {
                            TransitionToCompleted::Done => {} // No join handle connected yet, means join handle will make sure its not sleeping on its own on first poll
                            TransitionToCompleted::HadConnectedJoinHandle => match unsafe { &*self.handle_waker.get() } {
                                Some(v) => {
                                    v.wake_by_ref();
                                }
                                None => panic!("We shall never be here if we have HadConnectedJoinHandle set!"),
                            },
                        }

                        TaskPollResult::Done
                    }
                    std::task::Poll::Pending => {
                        match self.header.state.transition_to_idle() {
                            TransitionToIdle::Done => TaskPollResult::Done,
                            TransitionToIdle::Notified => TaskPollResult::Notified,
                            TransitionToIdle::Aborted => {
                                // TODO: Add abort handler, but this mean we are basically done for outer level, for now marked as todo!() for final impl
                                todo!();
                            }
                        }
                    }
                }
            }
            TaskStage::Completed(_) => {
                panic!("We shall never be in TaskStage::Completed in poll as we handle that before (task state == completed)")
            }
        }
    }

    pub fn get_future_ret(&self) -> Result<T, CommonErrors> {
        if !self.header.state.is_completed() {
            return Err(CommonErrors::NoData);
        }

        let ref_to_stage = unsafe { &mut *self.stage.get() };

        let should_take = match ref_to_stage {
            TaskStage::Extracted => false,
            TaskStage::InProgress(_) => false,
            TaskStage::Completed(_) => true,
        };

        let prev = mem::replace(&mut *ref_to_stage, TaskStage::Extracted);

        if should_take {
            if let TaskStage::Completed(v) = prev {
                Ok(v)
            } else {
                Err(CommonErrors::AlreadyDone)
            }
        } else {
            Err(CommonErrors::AlreadyDone)
        }
    }

    fn schedule(&self, task: TaskRef) {
        // TODO: For now we do simple reschedule (even not in instance, for now not needed, but later will be needed) without:
        // - no spawn into certain place (certain worker)
        // - ...

        match self.header.state.set_notified() {
            TransitionToNotified::Done => {
                // We need to reschedule on our own

                if let Some(handler) = ctx_get_handler() {
                    handler.respawn(task);
                } else {
                    self.scheduler.spawn_outside_runtime(task);
                }
            }
            TransitionToNotified::AlreadyNotified => {
                // Do nothing as someone already did it
            }
            TransitionToNotified::Running => {
                // Do nothing was we will be rescheduled by poll itself, still notification was marked
            }
        }
    }

    /// instance unbounded functions that can still bind a correct T so we can extract real instance from their arg. They forward calls to bounded API
    fn drop_vtable(this: NonNull<TaskHeader>) {
        let instance = this.as_ptr().cast::<AsyncTask<T>>();
        unsafe { drop(ArcInternal::from_raw(instance)) };
    }

    fn clone_vtable(this: NonNull<TaskHeader>) {
        let instance = this.as_ptr().cast::<AsyncTask<T>>();
        unsafe { ArcInternal::increment_strong_count(instance) };
    }

    fn schedule_vtable(this: NonNull<TaskHeader>, task: TaskRef) {
        let instance = this.as_ptr().cast::<AsyncTask<T>>();
        unsafe { (*instance).schedule(task) }
    }

    fn poll_vtable(this: NonNull<TaskHeader>, ctx: &mut Context) -> TaskPollResult {
        let instance = this.as_ptr().cast::<AsyncTask<T>>();
        unsafe { (*instance).poll(ctx) }
    }
}

///
/// Creates static reference to VTable for specific `T` using `const promotion` of Rust
///
fn create_task_vtable<T: 'static>() -> &'static TaskVTable {
    &TaskVTable {
        drop: AsyncTask::<T>::drop_vtable,
        clone: AsyncTask::<T>::clone_vtable,
        schedule: AsyncTask::<T>::schedule_vtable,
        poll: AsyncTask::<T>::poll_vtable,
    }
}

///
/// Task reference type that can track real `AsyncTask<T>` without knowing it's type. It make sure the ArcInternal<> holding a task has correct ref count
///
pub(crate) struct TaskRef {
    header: NonNull<TaskHeader>,
}

unsafe impl Send for TaskRef {}

impl TaskRef {
    pub(crate) fn new<T>(arc_task: ArcInternal<AsyncTask<T>>) -> Self {
        // Take raw pointer without any borrows
        let val = NonNull::new(ArcInternal::as_ptr(&arc_task) as *mut TaskHeader).unwrap();
        std::mem::forget(arc_task); // we took over ref count from arg into ourself
        Self { header: val }
    }

    pub(crate) fn clone(&self) -> Self {
        unsafe {
            (self.header.as_ref().vtable.clone)(self.header);
        }

        Self { header: self.header }
    }

    ///
    /// Release pointer and does not decrement ref count
    ///
    pub(crate) fn into_raw(this: TaskRef) -> *const TaskHeader {
        let ptr = this.header.as_ptr();
        std::mem::forget(this);
        ptr
    }

    ///
    /// Binds pointer and does not increment ref count. This call has to be paired with `into_raw` - same as ArcInternal<> calls.
    ///
    pub(crate) unsafe fn from_raw(ptr: *const TaskHeader) -> TaskRef {
        Self {
            header: NonNull::new(ptr as *mut TaskHeader).unwrap(),
        }
    }

    ///
    /// Proxy to `AsyncTask<T>::schedule`
    ///
    pub(crate) fn schedule(&self) {
        unsafe {
            (self.header.as_ref().vtable.schedule)(self.header, self.clone());
        }
    }

    ///
    /// Proxy to `AsyncTask<T>::poll`
    ///
    pub(crate) fn poll(&self, ctx: &mut Context) -> TaskPollResult {
        unsafe { (self.header.as_ref().vtable.poll)(self.header, ctx) }
    }
}

impl Drop for TaskRef {
    fn drop(&mut self) {
        unsafe {
            (self.header.as_ref().vtable.drop)(self.header);
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::{
        core::types::{box_future, ArcInternal},
        scheduler::{execution_engine::ExecutionEngineBuilder, task::async_task::TaskRef},
    };

    use super::AsyncTask;

    async fn dummy() {
        println!("test123");
    }

    async fn dummy_ret() -> Result<bool, String> {
        println!("test1234");

        Ok(true)
    }

    #[test]
    fn test_task_ctor() {
        // This code only proves to compile different Future constructs
        {
            let boxed = box_future(dummy());
            let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
            let task = ArcInternal::new(AsyncTask::new(boxed, 1, scheduler));
            let id = task.id();
            assert_eq!(id.0 & 0xFF, 1); // Test some internals
        }

        {
            let boxed = box_future(dummy_ret());
            let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
            let task = AsyncTask::new(boxed, 2, scheduler);
            let id = task.id();
            assert_eq!(id.0 & 0xFF, 2); // Test some internals
            drop(task);
        }

        {
            let boxed = box_future(async {
                println!("some test");
            });
            let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
            let task = AsyncTask::new(boxed, 2, scheduler);
            let id = task.id();
            assert_eq!(id.0 & 0xFF, 2); // Test some internals

            drop(task);
        }

        {
            let v = vec![0, 1, 2];

            let boxed = box_future(async move {
                println!("some test 1 {:?}", v);
            });
            let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
            let task = AsyncTask::new(boxed, 2, scheduler);
            let id = task.id();
            assert_eq!(id.0 & 0xFF, 2); // Test some internals

            drop(task);
        }
    }

    #[test]
    fn test_taskref_counting() {
        let boxed = box_future(dummy());
        let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
        let task = ArcInternal::new(AsyncTask::new(boxed, 1, scheduler));
        let id = task.id();
        assert_eq!(id.0 & 0xFF, 1); // Test some internals

        let task_ref = TaskRef::new(task.clone());

        assert_eq!(ArcInternal::strong_count(&task), 2);
        drop(task_ref);

        assert_eq!(ArcInternal::strong_count(&task), 1);

        {
            let task_ref = TaskRef::new(task.clone());
            let task_ref2 = task_ref.clone();
            let task_ref3 = task_ref2.clone();

            assert_eq!(ArcInternal::strong_count(&task), 4);
        }

        assert_eq!(ArcInternal::strong_count(&task), 1);
    }

    // #[test]
    // fn test_taskref_calling_task_api() {
    //     let boxed = box_future(dummy());
    //     let task = ArcInternal::new(AsyncTask::new(boxed, 1));
    //     let id = task.id();
    //     assert_eq!(id.0 & 0xFF, 1); // Test some internals

    //     let task_ref = TaskRef::new(task.clone());

    //     task_ref.schedule(); TODO: Fix a need for context in UT
    // }

    #[test]
    fn task_taskref_can_be_send_to_another_thread() {
        let boxed = box_future(dummy());
        let scheduler = ExecutionEngineBuilder::new().build().get_scheduler();
        let task = ArcInternal::new(AsyncTask::new(boxed, 1, scheduler));
        let id = task.id();
        assert_eq!(id.0 & 0xFF, 1); // Test some internals

        let task_ref = TaskRef::new(task.clone());

        let handle = {
            let cln: TaskRef = task_ref.clone();
            assert_eq!(ArcInternal::strong_count(&task), 3);
            std::thread::spawn(|| {
                drop(cln);
            })
        };

        handle.join().unwrap();
        assert_eq!(ArcInternal::strong_count(&task), 2);
    }
}
