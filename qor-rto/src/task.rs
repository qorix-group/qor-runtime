// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::{base::*, rto_errors};
use qor_core::prelude::*;

use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Poll, Waker};
use std::time::Duration;

/// Task cancel callback
type TaskCancelCallback<T> = Option<Box<dyn Fn(&Task<T>) -> bool + Send + 'static>>;

/// Task progress callback
type TaskProgressCallback<T> = Option<Box<dyn Fn(&Task<T>) -> (usize, usize) + Send + 'static>>;

/// Task notifier callback
type TaskNotifierCallback<T> = Option<Box<dyn Fn(&Task<T>, TaskState, TaskState) + Send + 'static>>;

//
// Task types
//

/// Create a new task from an async function.
///
/// This is a convenience funktion for `Task<T>::new(func)`.
///
/// # Example
///
/// ```rust
/// use qor_rto::prelude::*;
///
/// // The Hello World routine
/// async fn hello_world() {
///     println!("Hello World!");
/// }
///
/// // A Hello World program
/// fn main() {
///     // The engine is the central runtime executor
///     let engine = Engine::default();
///     engine.start().unwrap();
///
///     // Spawn the task on the engine passing the async function
///     let handle = engine.spawn(Task::new(hello_world)).unwrap();
///
///     // Wait for the task to finish
///     let _ = handle.join().unwrap();
///
///     // Engine shutdown
///     engine.shutdown().unwrap();
/// }
/// ```
///
#[allow(unused)]
#[inline(always)]
pub fn from_async<T, F, Fut>(routine: F) -> Task<T>
where
    T: Send + 'static,
    F: Fn() -> Fut + 'static,
    Fut: Future<Output = T> + Send + 'static,
{
    Task::new(routine)
}

/// Create a new task from a future.
///
/// This is a convenience funktion for `Task<T>::from(future)`.
///
/// # Example
///
/// ```rust
/// use qor_rto::prelude::*;
///
/// // The Hello World routine
/// async fn hello_world() {
///     println!("Hello World!");
/// }
///
/// // A Hello World program
/// fn main() {
///     // The engine is the central runtime executor
///     let engine = Engine::default();
///     engine.start().unwrap();
///
///     // Spawn the task on the engine using the future directly
///     let handle = engine.spawn(Task::from_future(hello_world())).unwrap();
///
///     // Wait for the task to finish
///     let _ = handle.join().unwrap();
///
///     // Engine shutdown
///     engine.shutdown().unwrap();
/// }
/// ```
///
#[allow(unused)]
#[inline(always)]
pub fn from_future<T, Fut>(future: Fut) -> Task<T>
where
    T: Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
{
    Task::from_future(future)
}

/// The execution state of a task while processed by the executor.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TaskState {
    /// Task is ready to be scheduled
    Ready = 0,

    /// Task is scheduled for execution
    Scheduled = 1,

    /// Task is currently running
    Running = 2,

    /// Task is currently cancelling
    Cancelling = 3,

    /// Task has terminated by cancellation
    Cancelled = 4,

    /// Task has been sucessfully completed
    Completed = 5,

    /// Task has an error state
    Error,
}

impl From<u32> for TaskState {
    fn from(value: u32) -> Self {
        TaskState::from_u32(value)
    }
}

impl From<TaskState> for u32 {
    fn from(val: TaskState) -> Self {
        val as u32
    }
}

impl TaskState {
    #[inline(always)]
    const fn new() -> Self {
        TaskState::Ready
    }

    #[inline(always)]
    const fn from_u32(value: u32) -> TaskState {
        match value {
            0 => TaskState::Ready,
            1 => TaskState::Scheduled,
            2 => TaskState::Running,
            3 => TaskState::Cancelling,
            4 => TaskState::Cancelled,
            5 => TaskState::Completed,
            _ => TaskState::Error,
        }
    }
}

/// Internal trait for scheduled tasks under the control of the executor.
/// This wraps the TaskHandleInner<T> struct and provides the necessary methods for scheduling.
pub(crate) trait TaskControl {
    /// Get the current state of the executable.
    fn state(&self) -> TaskState;

    /// Transition the state of the task.
    /// Task will execute the state change an also notify it's callback if set.
    fn transition_to(&self, to: TaskState);

    /// Cancel the task.
    /// Forwards to Task<T>::cancel
    fn cancel(&self) -> bool;

    /// Get the progress of the task.
    /// Forwards to `Task<T>::progress`
    #[allow(unused)]
    fn progress(&self) -> (usize, usize);

    /// Poll the task.
    /// Forwards to `Task<T>::poll` from the Future trait.
    /// As the executor does not use the Poll result, it is set to the unit `()` here.
    fn poll(&self, ctx: &mut std::task::Context<'_>) -> RtoResult<Poll<()>>;
}

/// A Task is the smallest unit of execution that can be scheduled on an executor.
///
/// The task delivers a result of type T upon completion. The implementation of task shall use the
/// Rust async/await pattern for concurrent execution.
pub struct Task<T>
where
    T: Send + 'static,
{
    /// The future that is executed by the task.
    future: Pin<Box<dyn Future<Output = T> + Send + 'static>>,

    /// The waker that is used to wake the executor.
    waker: Option<Waker>,

    /// An optional cancellation handler.
    ///
    /// This handler is called when the task runs and receives a cancellation request.
    /// The signature of a cancellation handler is `fn(&TaskHandle<T>) -> bool`.
    cancel: TaskCancelCallback<T>,

    /// An optional progress handler.
    ///
    /// This handler is called when the task receives a progress update request.
    /// The signature of a progress handler is `fn(&TaskHandle<T>) -> (usize, usize)`.
    ///
    /// The meaning of the progress tuple depends on the task implementation. However, the general
    /// convention should be that the first tuple value is the current progress while the second
    /// is the progress at completion.
    ///
    /// Examples:
    ///
    /// - `(0, 100)` means that the task has not started yet and will be completed after 100%.
    /// - `(50, 100)` means that the task is half way done and will be completed after 100%.
    /// - `(3, 5)` means that the three of a total of five steps are completed.
    ///
    progress: TaskProgressCallback<T>,

    /// An optional state change notifier.
    ///
    /// This handler is called when the task receives a state change by the executor.
    /// The signature of a notifier is `fn(&TaskHandle<T>, TaskState, TaskState) -> ()`.
    ///
    /// The first argument is the current state and the second argument is the new state of the task.
    ///
    notifier: TaskNotifierCallback<T>,

    /// The current state of the task.
    state: AtomicU32,
}

impl<T> Task<T>
where
    T: Send + 'static,
{
    /// Create a new task using the given async function.
    /// The future can be obtained by calling a Rust async function.
    ///
    /// TODO: FIXME: sometimes runs longer than 60 s
    /// ```no_run
    ///    use qor_rto::Engine;
    ///    use qor_rto::Task;
    ///
    ///    let executor = Engine::default();
    ///    let task = Task::new(|| async { 42 });   // create an async closure
    ///
    ///    executor.start().unwrap();
    ///    let handle = executor.spawn(task);
    ///
    ///    assert_eq!(handle.expect("valid handle").join().unwrap(), 42);
    ///
    ///    executor.shutdown().unwrap();
    /// ```
    pub fn new<F, Fut>(routine: F) -> Self
    where
        F: FnOnce() -> Fut + 'static,
        Fut: Future<Output = T> + Send + 'static,
    {
        Self {
            future: Box::pin(routine()),
            waker: None,
            cancel: None,
            progress: None,
            notifier: None,
            state: AtomicU32::new(TaskState::new().into()),
        }
    }

    /// Create a new task using the given future function.
    /// The future can be obtained by calling a Rust async function or by
    /// providing a user specific type that implements the `Future` trait.
    ///
    /// TODO: FIXME: test doesn't end
    /// ```no_run
    ///    use qor_rto::Engine;
    ///    use qor_rto::Task;
    ///
    ///    let executor = Engine::default();
    ///    let task = Task::from_future((|| async { 42 })());  // create an async closure and invoke it immediately to create a future
    ///
    ///    executor.start().unwrap();
    ///    let handle = executor.spawn(task);
    ///
    ///    assert_eq!(handle.expect("valid handle").join().unwrap(), 42);
    ///
    ///    executor.shutdown().unwrap();
    /// ```
    pub fn from_future<Fut>(future: Fut) -> Self
    where
        Fut: Future<Output = T> + Send + 'static,
    {
        Self {
            future: Box::pin(future),
            waker: None,
            cancel: None,
            progress: None,
            notifier: None,
            state: AtomicU32::new(TaskState::new().into()),
        }
    }

    /// Set the cancellation handler for the task.
    ///
    /// The signature of a cancellation handler is `fn(&Task<T>) -> bool`.
    /// It shall return `true` if the task should be cancelled and `false` otherwise.
    pub fn with_cancel(mut self, cancel: impl Fn(&Task<T>) -> bool + Send + 'static) -> Self {
        self.cancel = Some(Box::new(cancel));
        self
    }

    /// Set the progress handler for the task.
    ///
    /// The signature of a progress handler is `fn(&Task<T>) -> (usize, usize)`.
    /// It shall return a tuple where the first value is the current progress and the second value is the progress at completion.
    pub fn with_progress(
        mut self,
        progress: impl Fn(&Task<T>) -> (usize, usize) + Send + 'static,
    ) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    /// Set the state change notifier for the task.
    ///
    /// The signature of a notifier is `fn(&Task<T>, TaskState, TaskState) -> ()`.
    pub fn with_notifier(
        mut self,
        notifier: impl Fn(&Task<T>, TaskState, TaskState) + Send + 'static,
    ) -> Self {
        self.notifier = Some(Box::new(notifier));
        self
    }

    /// We implement Task also as Future so we allow to directly use `await` on a task, even if it is not under control of the Engine.
    /// The usual path to use a Task is to spawn it on an Engine and then join it through the `TaskHandle<T>` received.
    fn poll(&mut self, ctx: &mut std::task::Context<'_>) -> Poll<T> {
        // poll the future
        match self.future.as_mut().poll(ctx) {
            Poll::Ready(result) => {
                // already done: remove the waker
                self.waker = None;
                Poll::Ready(result)
            }

            Poll::Pending => {
                // not done: store the waker
                self.waker = Some(ctx.waker().clone());
                Poll::Pending
            }
        }
    }

    /// Get the current state of the task.
    pub(crate) fn state(&self) -> TaskState {
        TaskState::from(self.state.load(Ordering::Acquire))
    }

    /// Transition the state of the task.
    pub(crate) fn transition_to(&self, to: TaskState) {
        let from = TaskState::from(self.state.load(Ordering::Relaxed));
        self.state.store(to.into(), Ordering::Release);

        // notify of the state change
        if let Some(notifier) = &self.notifier {
            notifier(self, from, to);
        }
    }

    /// Cancel the task if cancellation handler is set.
    pub(crate) fn cancel(&self) -> bool {
        match self.state() {
            TaskState::Scheduled => {
                // scheduled tasks cancel immediately
                self.transition_to(TaskState::Cancelled);
                true
            }

            TaskState::Running => {
                // running tasks can be cancelled if they have a handler and the handler allows it
                if let Some(cancel) = &self.cancel {
                    if cancel(self) {
                        self.transition_to(TaskState::Cancelling);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }

            // all other states cannot be cancelled
            _ => false,
        }
    }

    /// Get the progress of the task.
    pub(crate) fn progress(&self) -> (usize, usize) {
        if let Some(progress) = &self.progress {
            progress(self)
        } else {
            (0, 0)
        }
    }
}

impl<T> Future for Task<T>
where
    T: Send + 'static,
{
    type Output = T;

    /// We implement Task also as Future so we allow to directly use `await` on a task, even if it is not under control of the Engine.
    /// The usual path to use a Task is to spawn it on an Engine and then join it through the `TaskHandle<T>` received.
    fn poll(mut self: Pin<&mut Self>, ctx: &mut std::task::Context<'_>) -> Poll<T> {
        Self::poll(&mut self, ctx)
    }
}

/// Inner task handle structure.
///
/// The Inner holds the task and the task completion state.
pub(crate) struct TaskHandleInner<T>
where
    T: Send + 'static,
{
    /// The task associated with the handle.
    task: UnsafeCell<Task<T>>,

    /// The result of the task together with lock and signal.
    result: (Mutex<Option<T>>, Condvar),
}

// we implement Send and Sync for TaskHandleInner<T> so TaskHandle<T> can be shared between threads
unsafe impl<T> Send for TaskHandleInner<T> where T: Send + 'static {}
unsafe impl<T> Sync for TaskHandleInner<T> where T: Send + 'static {}

impl<T> TaskHandleInner<T>
where
    T: Send + 'static,
{
    pub(crate) fn new(task: Task<T>) -> Self {
        Self {
            task: UnsafeCell::new(task),
            result: (Mutex::new(None), Condvar::new()),
        }
    }

    /// Get the task associated with the handle
    #[inline]
    fn task(&self) -> &Task<T> {
        unsafe { &*self.task.get() }
    }

    /// Get the mutable task associated with the handle (executor only)
    ///
    /// TODO: this needs to be fixed, see
    /// https://rust-lang.github.io/rust-clippy/master/index.html#mut_from_ref for details
    #[allow(clippy::mut_from_ref)]
    #[inline(always)]
    fn task_mut(&self) -> &mut Task<T> {
        unsafe { &mut *self.task.get() }
    }

    /// Join the task and get the result.
    fn join(&self) -> RtoResult<T> {
        let (result_lock, notifier) = &self.result;

        // wait for the task to complete
        let mut maybe_result = result_lock
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // check on the completion state and retreive the result
        loop {
            // if we have a result we take it
            if let Some(result) = maybe_result.take() {
                return Ok(result);
            }

            // check cancellation
            if self.state() == TaskState::Cancelled {
                return Err(Error::const_new(
                    rto_errors::TASK_EMPTY_RESULT,
                    "Task was cancelled.",
                ));
            }

            // and wait for the condvar (or timeout)
            let (result, _) = notifier
                .wait_timeout(maybe_result, Duration::from_millis(100))
                .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

            maybe_result = result;
        }
    }
}

impl<T> TaskControl for TaskHandleInner<T>
where
    T: Send + 'static,
{
    fn state(&self) -> TaskState {
        self.task().state()
    }

    fn transition_to(&self, to: TaskState) {
        self.task().transition_to(to);
    }

    // forward cancellation
    fn cancel(&self) -> bool {
        self.task().cancel()
    }

    // forward progress
    fn progress(&self) -> (usize, usize) {
        self.task().progress()
    }

    // wrap poll and remove T dependency
    fn poll(&self, ctx: &mut std::task::Context<'_>) -> RtoResult<Poll<()>> {
        match self.task_mut().poll(ctx) {
            Poll::Ready(result_from_future) => {
                // store the result and notify the completion
                let (result_lock, completion) = &self.result;

                let mut result_into_handle = result_lock
                    .lock()
                    .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;
                *result_into_handle = Some(result_from_future);

                // notify waiting handle
                completion.notify_one();
                Ok(Poll::Ready(()))
            }
            Poll::Pending => Ok(Poll::Pending),
        }
    }
}

/// A TaskHandle is used to track a task after it has been spawned on an executor.
///  
/// The potential result of the task is provided upon joining.
///
pub struct TaskHandle<T>(Arc<TaskHandleInner<T>>)
where
    T: Send + 'static;

impl<T> TaskHandle<T>
where
    T: Send + 'static,
{
    /// Create a new task handle.
    pub(crate) fn new(handle: Arc<TaskHandleInner<T>>) -> Self {
        Self(handle)
    }

    /// Get the task associated with the handle
    #[inline(always)]
    fn task(&self) -> &Task<T> {
        self.0.task()
    }

    /// Get the current task state
    #[inline(always)]
    pub fn state(&self) -> TaskState {
        self.task().state()
    }

    /// Check if the task has finished.
    ///
    /// This is a convenience function compatible with std::task::JoinHandle::is_finished.
    /// When is_finished() returns true the task can join without blocking.
    ///
    pub fn is_finished(&self) -> bool {
        self.task().state() == TaskState::Completed || self.task().state() == TaskState::Cancelled
    }

    /// Check if the task is in error state.
    pub fn has_error(&self) -> bool {
        self.task().state() == TaskState::Error
    }

    /// Cancel the task
    ///
    /// This sends a cancellation signal to the task. If the task has defined a cancellation handler,
    /// it will be notified and the success of the cancellation will be returned.
    /// If no handler is set the method returns `false`.
    pub fn cancel(&self) -> bool {
        self.task().cancel()
    }

    /// Get the progress of the task
    ///
    /// This returns a tuple where the first value is the current progress and the second value is the progress at completion.
    /// If no progress handler is set the method returns `(0, 0)`.
    pub fn progress(&self) -> (usize, usize) {
        self.task().progress()
    }

    /// Join the task and get the result.
    ///
    /// This will block the current thread if the task has not completed and wait for completion.
    /// The result of the task will be returned upon completion.
    ///
    /// Joining the task may cause several errors:
    ///
    /// - `LOCK_ERROR` if the task completion lock could not be acquired.
    /// - `TASK_EMPTY_RESULT` if the task has completed but did not reveal a result. This can happen if the task has been cancelled
    ///     and could not produce a valid result.
    ///
    pub fn join(&self) -> RtoResult<T> {
        self.0.join()
    }
}

/// `TaskHandle<T>` implements `Future<Output = T>` so it can be used as a future in an `await` statement.
impl<T: Default> Future for TaskHandle<T>
where
    T: Send + 'static,
{
    type Output = T;

    /// Poll the future of the task associated with the task handle.
    fn poll(self: Pin<&mut Self>, ctx: &mut std::task::Context<'_>) -> Poll<T> {
        self.0.task_mut().poll(ctx)
    }
}
