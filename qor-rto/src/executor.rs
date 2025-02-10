// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::{base::*, rto_errors};
use qor_core::prelude::*;
//use qor_os::thread;

use crate::task::{Task, TaskControl, TaskHandle, TaskHandleInner, TaskState};
use crate::waker;

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::task::Poll;
use std::thread::JoinHandle;
use std::time::Duration;

/// Engine scheduled tasks
type EngineScheduledTasks = (Mutex<Box<[Option<Arc<dyn TaskControl>>]>>, Condvar);

//
// Execution Scheduling
//

// /// An Executor is the central element of task execution.
// ///
// /// A task is a unit of work that has a defined start, end and result.
// /// The executor schedules tasks and runs them concurrently by utilizing
// /// the resources allocated by the executor. Usually, these resources are
// /// operating system threads.
// ///
// /// Taking a task under the control of the executor is called "spawning" a task.
// /// Once a task is spawned a TaskHandle is returned that can be used to control
// /// the execution of the task and also get the result.
// ///
// /// To work seemlessly with Rust's async/await model, the executor's task handles
// /// are futures that can be awaited. This allows to write async functions that
// /// can be run on the executor.
// ///
// /// # Example
// ///
// /// ```rust
// /// async fn my_async_function() -> () {
// ///  // do something
// /// }
// ///
// /// fn main() {
// ///   let executor = EngineBuilder::new().with_threads(4).build().unwrap();
// ///   let task = Task::new(|| my_async_function());
// ///
// ///   executor.start(); // start the engine
// ///   let handle = executor.spawn(task); // schedule the task. the engine will put it on an available thread and run it.
// ///
// ///   let result = handle.join(); // wait for the task to complete and get the result
// ///   // do something with the result
// /// }
// ///
// pub trait Executor {
//     /// Spawn a new task on the engine.
//     fn spawn<T>(&self, task: Task<T>) -> RtoResult<TaskHandle<T>>
//     where
//         T: Send + 'static;
// }

/// Create a new Engine executor with the given number of threads and tasks.
///
///
#[allow(dead_code)]
pub fn default() -> Engine {
    Engine::default()
}

/// The EngineBuilder configures an Engine as Executor  
#[allow(dead_code)]
pub struct EngineBuilder {
    /// Tag of the engine
    tag: Tag,

    /// Number of threads that the engine should use.
    nthreads: usize,

    /// Number of tasks that the engine should be able to handle.
    ntasks: usize,

    /// Thread priority to be used by the engine.
    priority: Option<u32>,

    /// Core group Id for thread affinity.
    affinity: Option<Id>,

    /// The clock that is used by the engine to measure time
    clock: Box<dyn Clock>,
}

#[allow(dead_code)]
impl EngineBuilder {
    pub const DEFAULT: (usize, usize) = (8, 256);

    /// Create a new builder with default values.
    #[inline(always)]
    pub fn new() -> EngineBuilder {
        EngineBuilder {
            tag: Tag::new(*b"Engine__"),
            nthreads: Self::DEFAULT.0,
            ntasks: Self::DEFAULT.1,
            priority: None,
            affinity: None,
            clock: Box::new(AccurateClock::new()),
        }
    }

    /// Set the name of the engine.
    pub fn with_tag(mut self, tag: Tag) -> EngineBuilder {
        self.tag = tag;
        self
    }

    /// Set the number of threads that the engine should use.
    #[inline(always)]
    pub fn with_threads(mut self, nthreads: usize) -> EngineBuilder {
        self.nthreads = nthreads;
        self
    }

    /// Set the number of tasks that the engine should be able to handle.
    #[inline(always)]
    pub fn with_tasks(mut self, ntasks: usize) -> EngineBuilder {
        self.ntasks = ntasks;
        self
    }

    /// Set the priority of the threads that the engine should use.
    #[inline(always)]
    pub fn with_priority(mut self, priority: u32) -> EngineBuilder {
        self.priority = Some(priority);
        self
    }

    /// Set the core group affinity of the threads that the engine should use.
    #[inline(always)]
    pub fn with_affinity(mut self, affinity: Id) -> EngineBuilder {
        self.affinity = Some(affinity);
        self
    }

    /// Set the clock that the engine should use to measure time.
    #[inline(always)]
    pub fn with_clock(mut self, clock: Box<dyn Clock>) -> EngineBuilder {
        self.clock = clock;
        self
    }

    /// Build the engine with the given configuration.
    #[inline(always)]
    pub fn build(self) -> RtoResult<Engine> {
        if self.priority.is_some() {
            Err(Error::const_new(
                qor_core::core_errors::UNSUPPORTED_FEATURE,
                "Thread priority is not supported yet",
            ))
        } else if self.affinity.is_some() {
            Err(Error::const_new(
                qor_core::core_errors::UNSUPPORTED_FEATURE,
                "Thread affinity is not supported yet",
            ))
        } else {
            Ok(Engine::new(
                self.tag,
                self.clock,
                self.nthreads,
                self.ntasks,
            ))
        }
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// An engine runs tasks concurrently on threads. It is the central executor of runtime orchestration.
///
/// The engine uses a threadpool to run tasks concurrently. Tasks are scheduled on the engine and run on the next
/// available thread.
///
/// The engine can be obtained by either calling `executor::default()` or by utilizing an `executor::EngineBuilder` which gives
/// control over the configuration of the engine.
///
/// The engine is compliant with Rust's async executor model so the following is possible:
///
/// ```rust
/// use qor_rto::{EngineBuilder, Task};
///
/// async fn my_async_function() -> () {
///   // do something
/// }
///
/// fn main() {
///   let executor = EngineBuilder::new().with_threads(4).build().unwrap();
///   let task = Task::new(|| my_async_function());
///
///   executor.start(); // start the engine
///   let handle = executor.spawn(task); // schedule the task. the engine will put it on an available thread and run it.
///
///   let result = handle.expect("valid handle").join(); // wait for the task to complete and get the result
///   // do something with the result
/// }
/// ```
///
pub struct Engine {
    inner: Arc<EngineInner>,
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(
            Tag::new(*b"Default_"),
            Box::new(AccurateClock::new()),
            8,
            256,
        )
    }
}

impl Engine {
    /// The maximum number of threads that can be used by the engine.
    pub const MAX_THREADS: usize = 256;

    /// The maximum number of tasks that can be scheduled on the engine.
    pub const MAX_TASKS: usize = 8 * Self::MAX_THREADS;

    /// Create a new engine with the given number of threads and tasks.
    ///
    /// The thread number will be adjusted to the allowed range `[1..Engine::MAX_THREADS]`.
    /// The task number will be adjusted to the allowed range `[8..Engine::MAX_TASKS]`.
    ///
    /// After the engine is created it must still be started to become active.
    fn new(tag: Tag, clock: Box<dyn Clock>, nthreads: usize, ntasks: usize) -> Engine {
        Engine {
            inner: EngineInner::new(tag, clock, nthreads, ntasks),
        }
    }

    /// Get the inner engine structure.
    pub(crate) fn inner(&self) -> &Arc<EngineInner> {
        &self.inner
    }

    /// Get the current state of the engine.
    #[inline(always)]
    pub fn state(&self) -> ExecutionState {
        self.inner.state()
    }

    /// Check if the engine is ready to be started.
    ///
    /// The engine is ready if it has been initialized or reset and has not reveived a stop signal.
    #[inline(always)]
    pub fn is_ready(&self) -> bool {
        self.state() == ExecutionState::Ready && !self.is_terminating()
    }

    /// Check if the engine is running and can execute tasks.
    ///
    /// The engine is running if it has been started and has not received a stop signal.
    #[inline(always)]
    pub fn is_running(&self) -> bool {
        self.state() == ExecutionState::Running && !self.is_terminating()
    }

    /// Check if the engine is terminating.
    ///
    /// The engine is terminating if it has received a stop signal and has not finished the shutdown process.
    #[inline(always)]
    pub fn is_terminating(&self) -> bool {
        let state = self.state();

        match state {
            ExecutionState::Uninitialized
            | ExecutionState::Completed
            | ExecutionState::Terminated => false,
            ExecutionState::Ready | ExecutionState::Running => {
                self.inner.stop_signal.load(Ordering::Relaxed)
            }

            _ => false,
        }
    }

    /// Get the clock that is used by the engine to measure time.
    pub fn clock(&self) -> &dyn Clock {
        self.inner.clock()
    }

    /// Get the number of threads that can be used by the engine.
    #[inline(always)]
    pub fn thread_capacity(&self) -> usize {
        self.inner.nthreads
    }

    /// Get the utilization of the engine's threads.
    ///
    /// The tuple contains the number of working threads busy with a task and the number of running threads.
    pub fn thread_utilization(&self) -> (usize, usize) {
        (
            self.inner.working_threads.load(Ordering::Relaxed),
            self.inner.running_threads.load(Ordering::Relaxed),
        )
    }

    /// Get the number of tasks that can be scheduled on the engine.
    pub fn task_capacity(&self) -> usize {
        self.inner.ntasks
    }

    /// Get the utilization of the tasks spawned on the engine.
    ///
    /// The tuple contains the number of tasks on working threads and the total number of tasks on the engine.
    /// This value is a snapshot and may change immediately after the call. It also may contain glitches as
    /// the call may happen when a task is in transit from scheduling queue into working thread.
    pub fn task_utilization(&self) -> (usize, usize) {
        let waiting_tasks = self.inner.waiting_tasks.load(Ordering::Relaxed);
        let working_threads = self.inner.working_threads.load(Ordering::Relaxed);
        (working_threads, waiting_tasks + working_threads)
    }

    /// Checks if the engine is idle. The engine is idle if
    ///
    /// - The engine is running and no tasks are scheduled
    /// - The engine is not running
    pub fn idle(&self) -> bool {
        ExecutionState::from(self.inner.state.load(Ordering::Relaxed)) != ExecutionState::Running
            || self.inner.waiting_tasks.load(Ordering::Relaxed) == 0
    }

    /// Clear the task queue.
    ///
    /// After this all scheduled tasks are removed from the engine.
    /// Running tasks are not affected by the clear operation.
    pub fn clear(&self) -> RtoResult<()> {
        self.inner.clear()
    }

    /// Initialize the engine.
    ///
    /// This creates the thread pool and prepares the engine for task execution.
    /// In the engine implementation the init() is a separate function for two reasons:
    ///
    /// - It is in line with the exectution lifetime model of the `ExecutionStateMachine`.
    /// - It allows to defer the initialization from the start, significantly speeding up the engine start operation.
    ///
    pub fn init(&self) -> RtoResult<ExecutionState> {
        self.inner.init()
    }

    /// Reset the engine into post-initialization state.
    ///
    /// This clears the task queue, cancels all running tasks and waits for the running tasks to finish.
    /// Reset cannot be called when the engine is uninitialized or terminated.
    pub fn reset(&self) -> RtoResult<ExecutionState> {
        self.inner.reset()
    }

    /// Start the engine.
    ///
    /// If the engine has not been initialized yet, this will automatically do so.
    /// This will start the threads and make the engine ready to run tasks.
    /// This function will return after the engine is started.
    pub fn start(&self) -> RtoResult<ExecutionState> {
        self.inner.start()
    }

    /// Send a stop signal to the engine. The engine will shut down running tasks and stop the threads.
    pub fn shutdown(&self) -> RtoResult<ExecutionState> {
        self.inner.signal_stop()?;
        self.inner.join()?;
        self.inner.terminate()
    }

    /// Spawn a new task on the engine.
    ///
    /// The task will be scheduled on the engine and run on the next available thread.
    /// If scheduling is successful this returns the TaskHandle to be used for joining the task and obtaining the result.
    pub fn spawn<T>(&self, task: Task<T>) -> RtoResult<TaskHandle<T>>
    where
        T: Send + 'static,
    {
        self.inner.spawn(task)
    }
}

// impl Executor for Engine {
//     fn spawn<T>(&self, task: Task<T>) -> RtoResult<TaskHandle<T>>
//     where
//         T: Send + 'static,
//     {
//         self.inner.spawn(task)
//     }
// }

/// The state of the running state machine of the engine.
#[derive(Copy, Clone, PartialEq)]
enum ThreadState {
    Created = 0,
    Ready = 1,
    RunningIdle = 2,
    RunningBusy = 3,
    Stopping = 4,
    Resetting = 5,
    Terminated = 6,
    Error,
}

impl From<u32> for ThreadState {
    fn from(value: u32) -> ThreadState {
        ThreadState::from_u32(value)
    }
}

impl From<ThreadState> for u32 {
    fn from(val: ThreadState) -> Self {
        val as u32
    }
}

impl ThreadState {
    #[inline(always)]
    const fn from_u32(value: u32) -> ThreadState {
        match value {
            0 => ThreadState::Created,
            1 => ThreadState::Ready,
            2 => ThreadState::RunningIdle,
            3 => ThreadState::RunningBusy,
            4 => ThreadState::Stopping,
            5 => ThreadState::Resetting,
            6 => ThreadState::Terminated,
            _ => ThreadState::Error,
        }
    }
}

/// Engine internal thread control structure, used by the engine to manage threads.
struct ThreadControl {
    handle: JoinHandle<RtoResult<()>>,
    context: Arc<ThreadContext>,
}

impl ThreadControl {
    /// Create a new thread control structure.
    const fn new(handle: JoinHandle<RtoResult<()>>, context: Arc<ThreadContext>) -> ThreadControl {
        ThreadControl { handle, context }
    }

    /// Join the thread and wait for it to finish.
    /// Extract the result from the thread.
    fn join(self) -> RtoResult<()> {
        if let Ok(result) = self.handle.join() {
            result
        } else {
            Err(Error::const_new(
                rto_errors::THREAD_JOIN_ERROR,
                "Thread failed to join",
            ))
        }
    }
}

/// Engine internal thread context structure, used by the threads to control their state.
struct ThreadContext {
    state: AtomicU32,
    reset_signal: AtomicBool,
    signal: Arc<crate::waker::Signal>,
}

impl ThreadContext {
    /// Create a new thread context structure.
    fn new() -> ThreadContext {
        ThreadContext {
            state: AtomicU32::new(ThreadState::Ready.into()),
            reset_signal: AtomicBool::new(false),
            signal: Arc::new(crate::waker::Signal::new()),
        }
    }

    fn state(&self) -> ThreadState {
        ThreadState::from(self.state.load(Ordering::Acquire))
    }

    fn transition_to(&self, state: ThreadState) -> ThreadState {
        self.state.store(state.into(), Ordering::Release);
        state
    }

    fn signal_reset(&self) {
        self.reset_signal.store(true, Ordering::Release);
    }
}

/// The inner engine. This is the threading workhorse of the runtime orchestrator.
pub(crate) struct EngineInner {
    /// The tag of the engine.
    tag: Tag,

    /// The clock that is used by the engine to measure time.
    clock: Box<dyn Clock>,

    /// The execution state of the engine.
    /// This uses the u32 representation of the `ExecutionState` enum.
    state: AtomicU32,

    /// The number of threads that can be used by the engine.
    nthreads: usize,

    /// The number of threads that are currently running.
    running_threads: AtomicUsize,

    /// The number of threads that are currently processing a task.
    working_threads: AtomicUsize,

    /// The threads that are currently used by the engine.
    threads: Mutex<Vec<ThreadControl>>,

    /// A signal to stop the engine.
    stop_signal: AtomicBool,

    /// The number of tasks that can be scheduled on the engine.
    ntasks: usize,

    /// The number of tasks that are currently waiting in scheduling queue.
    waiting_tasks: AtomicUsize,

    /// Scheduled tasks queue, locked with signalling condition.
    ///
    /// Note: We do not use a lockfree queue here as we need to signal waiting threads anyway.
    /// The signalling requires a mutex so we can easy atomize operations by protecting the entire queue .
    scheduled_tasks: EngineScheduledTasks,

    /// head of the scheduled tasks queue. We use an UnsafeCell to allow inner mutability.
    scheduled_head: UnsafeCell<usize>,

    /// tail of the scheduled tasks queue. We use an UnsafeCell to allow inner mutability.
    scheduled_tail: UnsafeCell<usize>,
}

unsafe impl Send for EngineInner {}
unsafe impl Sync for EngineInner {}

impl EngineInner {
    /// Create a new engine inner structure.
    fn new(tag: Tag, clock: Box<dyn Clock>, nthreads: usize, ntasks: usize) -> Arc<EngineInner> {
        let nthreads = nthreads.clamp(1, Engine::MAX_THREADS);
        let ntasks = ntasks.clamp(8, Engine::MAX_TASKS);

        let mut tasks = Vec::with_capacity(ntasks);
        tasks.resize(ntasks, None);

        Arc::new(EngineInner {
            tag,
            state: AtomicU32::new(ExecutionState::new().into()),
            clock,
            nthreads,
            threads: Mutex::new(Vec::with_capacity(nthreads)),
            running_threads: AtomicUsize::new(0),
            working_threads: AtomicUsize::new(0),
            stop_signal: AtomicBool::new(false),
            ntasks,
            waiting_tasks: AtomicUsize::new(0),
            scheduled_tasks: (Mutex::new(tasks.into_boxed_slice()), Condvar::new()),
            scheduled_head: UnsafeCell::new(0),
            scheduled_tail: UnsafeCell::new(0),
        })
    }

    /// Get the current state of the engine.
    #[inline(always)]
    fn state(self: &Arc<Self>) -> ExecutionState {
        ExecutionState::from(self.state.load(Ordering::Relaxed))
    }

    /// Get the clock that is used by the engine to measure time.
    pub fn clock(&self) -> &dyn Clock {
        &*self.clock
    }

    /// Clear the task queue.
    ///
    /// After this all scheduled tasks are removed from the engine.
    /// Running tasks are not affected by the clear operation.
    fn clear(self: &Arc<Self>) -> RtoResult<()> {
        // get our utilities
        let (scheduled_tasks, _) = &self.scheduled_tasks;

        // acquire the lock
        let mut scheduled_tasks = scheduled_tasks
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // iterate over the queue and clear all tasks
        while unsafe { *self.scheduled_tail.get() != *self.scheduled_head.get() } {
            // queue is not empty: clear it
            let task = scheduled_tasks[unsafe { *self.scheduled_head.get() }].take();
            unsafe { *self.scheduled_head.get() = (*self.scheduled_head.get() + 1) % self.ntasks };

            // cancel task
            if let Some(task) = task {
                // remove from waiting
                self.waiting_tasks.fetch_sub(1, Ordering::Relaxed);

                // cancel the task
                debug_assert_eq!(task.state(), TaskState::Scheduled);
                let ack = task.cancel();
                if !ack {
                    // task could not be cancelled
                    return Err(Error::from_code(rto_errors::TASK_CANCEL_ERROR));
                }
            }
        }

        Ok(())
    }

    /// Initialize the inner engine
    //TODO: Change the thread pool to OSAL threads that allow priority and affinity settings.
    fn init(self: &Arc<Self>) -> RtoResult<ExecutionState> {
        let state = ExecutionState::from(self.state.load(Ordering::Relaxed));

        // handle state
        match state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                // update state (we have to do this here as the threads will check the state upon startup)
                self.state
                    .store(ExecutionState::Ready.into(), Ordering::Relaxed);

                // create all threads
                let mut threads = self
                    .threads
                    .lock()
                    .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

                for i in 0..self.nthreads {
                    // the thread context
                    let context = Arc::new(ThreadContext::new());

                    // myself and context reference (with RunState::WaitForStart) to be moved into the thread
                    let self_clone = self.clone();
                    let context_clone: Arc<ThreadContext> = context.clone();

                    // spawn the thread utilizing a builder (for naming)
                    // TODO: replace this builder with a custom builder from qor_os abstraction.
                    let thread = std::thread::Builder::new()
                        .name(format!("{}_Worker_{}", self.tag, i))
                        .spawn(move || Self::run(self_clone, context_clone))
                        .map_err(|_| {
                            Error::const_new(
                                rto_errors::THREAD_SPWAN_ERROR,
                                "Failed to spawn worker thread",
                            )
                        })?;

                    // and store
                    threads.push(ThreadControl::new(thread, context));
                }

                // done
                Ok(ExecutionState::Ready)
            }
            _ => Err(Error::new(
                rto_errors::INVALID_OPERATION,
                format!("Cannot initialize engine from `{}` state", state),
            )),
        }
    }

    /// Reset the engine.
    fn reset(self: &Arc<Self>) -> RtoResult<ExecutionState> {
        let state = ExecutionState::from(self.state.load(Ordering::Relaxed));
        // check state
        if state == ExecutionState::Uninitialized || state == ExecutionState::Terminated {
            return Err(Error::new(
                rto_errors::INVALID_OPERATION,
                format!("Cannot reset a engine from `{}` state", state),
            ));
        }

        // clear task queue
        self.clear()?;

        // when running wait for tasks to finish
        if ExecutionState::from(self.state.load(Ordering::Relaxed)) == ExecutionState::Running {
            // wait for all tasks to finish
            while self.working_threads.load(Ordering::Relaxed) > 0 {
                std::thread::yield_now();
            }
        }

        // send reset signal to all threads
        let threads = self
            .threads
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;
        for control in threads.iter() {
            control.context.signal_reset();
        }

        // update state to Ready
        self.state
            .store(ExecutionState::Ready.into(), Ordering::Release);

        // notify all threads
        self.notify_all();

        // done
        Ok(ExecutionState::Ready)
    }

    /// Start the engine.
    fn start(self: &Arc<Self>) -> RtoResult<ExecutionState> {
        // check stop signal first
        if self.stop_signal.load(Ordering::Relaxed) {
            return Err(Error::const_new(
                rto_errors::INVALID_OPERATION,
                "Cannot start engine while stop signal is set",
            ));
        }

        // load the state
        let mut state = ExecutionState::from(self.state.load(Ordering::Relaxed));

        // auto initialize
        if state == ExecutionState::Uninitialized {
            state = self.init()?;
        }

        // and start
        match state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // lock until all threads are up and running
                {
                    let threads = self
                        .threads
                        .lock()
                        .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

                    // wait until all threads are running
                    while self.running_threads.load(Ordering::Relaxed) < threads.len() {
                        std::thread::yield_now();
                    }
                } // auto release lock here

                // set engine state to running
                self.state
                    .store(ExecutionState::Running.into(), Ordering::Release);

                // notify all threads
                self.notify_all();
                Ok(ExecutionState::Running)
            }

            _ => Err(Error::new(
                rto_errors::INVALID_OPERATION,
                format!("Cannot start engine while in state `{}`", state),
            )),
        }
    }

    /// Send a stop signal to the engine. The engine will shut down running tasks and stop the threads.
    fn signal_stop(self: &Arc<Self>) -> RtoResult<()> {
        let state = ExecutionState::from(self.state.load(Ordering::Relaxed));

        // and check
        if state == ExecutionState::Ready || state == ExecutionState::Running {
            // set the stop signal
            self.stop_signal.store(true, Ordering::Relaxed);

            // tell everyone waiting.
            // as there is only _one_ place where a thread can wait, within the `pop_task_or_wait` function, this single operation is enough.
            // We cannot get the lock in `notify_all` if it is held in `pop_task_or_wait` and vice versa.
            self.notify_all();

            // that was easy
            Ok(())
        } else {
            Err(Error::const_new(
                rto_errors::INVALID_OPERATION,
                "Cannot stop engine while not ready or running",
            ))
        }
    }

    /// Wait for all engine threads to join
    fn join(self: &Arc<Self>) -> RtoResult<ExecutionState> {
        let state = self.state();

        // pre-check state
        match state {
            ExecutionState::Uninitialized => {
                return Err(Error::const_new(
                    rto_errors::INVALID_OPERATION,
                    "Cannot join uninitialized engine",
                ))
            }

            ExecutionState::Ready | ExecutionState::Running => {
                // threads are up (as we are initialized) -> we can join
                if !self.stop_signal.load(Ordering::Relaxed) {
                    // send stop signal automatically
                    self.signal_stop()?;
                }
            }

            ExecutionState::Completed | ExecutionState::Terminated => {
                // we are already completed -> nothing to do here
                return Ok(state);
            }

            _ => {
                return Err(Error::new(
                    rto_errors::INVALID_OPERATION,
                    format!("Cannot join engine in state `{}`", state),
                ))
            }
        }

        // lock thread list
        let mut threads = self
            .threads
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // join all threads
        let mut errors = 0;
        while let Some(thread) = threads.pop() {
            let id = thread.handle.thread().id();

            if let Err(err) = thread.join() {
                match err.code() {
                    rto_errors::THREAD_JOIN_ERROR => {
                        errors += 1;
                    }

                    _ => {
                        println!("Engine Shutdown: Thread {:?} returned error: {}", id, err);
                    }
                }
                errors += 1;
            }
        }

        // update state
        self.state
            .store(ExecutionState::Completed.into(), Ordering::Relaxed);

        // check for errors
        if errors > 0 {
            Err(Error::new(
                rto_errors::THREAD_JOIN_ERROR,
                format!("{} threads failed to join", errors),
            ))
        } else {
            Ok(ExecutionState::Completed)
        }
    }

    /// Terminate the engine.
    pub fn terminate(self: &Arc<Self>) -> RtoResult<ExecutionState> {
        if self.join()? == ExecutionState::Completed {
            self.state
                .store(ExecutionState::Terminated.into(), Ordering::Relaxed);
        }
        Ok(ExecutionState::Terminated)
    }

    /// Notify all threads that a general update is requested.
    fn notify_all(&self) {
        let (scheduled_tasks, notifier) = &self.scheduled_tasks;
        let _ = scheduled_tasks.lock().map(|_| notifier.notify_all());
    }

    /// Push a new task onto the engine's task queue.
    ///
    /// As the task queue is protected my a mutex this will both acquire the mutex and signal waiting threads.
    /// Returns an error if the queue is full.
    fn push_task(self: &Arc<Self>, task: Arc<dyn TaskControl>) -> RtoResult<()> {
        // get our utilities
        let (scheduled_tasks, notifier) = &self.scheduled_tasks;

        // acquire the lock
        let mut scheduled_tasks = scheduled_tasks
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // next tail, with wrap around
        let new_tail = unsafe { (*self.scheduled_tail.get() + 1) % self.ntasks };

        // check queue full
        if new_tail == unsafe { *self.scheduled_head.get() } {
            return Err(Error::from_code(qor_core::core_errors::COLLECTION_FULL));
        }

        // we are good: push the task
        scheduled_tasks[unsafe { *self.scheduled_tail.get() }] = Some(task);

        // update tail
        unsafe { *self.scheduled_tail.get() = new_tail };

        // we hold the lock: signal waiting threads
        self.waiting_tasks.fetch_add(1, Ordering::Release);

        // and notify
        notifier.notify_one();

        // done
        Ok(())
    }

    /// Pop a task from the engine's task queue if there is one.
    ///
    /// This will lock if no task is available and sync on the scheduled tasks queue signalling event.
    /// That way we have a linearization point synching all threads in push and pop operations.
    /// The implementation returns None if a signal was received _and_ the queue is still empty.
    fn pop_task_or_wait(
        self: &Arc<Self>,
        thread_ctx: &Arc<ThreadContext>,
    ) -> Option<Arc<dyn TaskControl>> {
        // get our utilities
        let (scheduled_tasks, notifier) = &self.scheduled_tasks;

        // acquire the lock
        let mut scheduled_tasks = scheduled_tasks
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))
            .ok()?;

        // check our queue on emptyness
        while unsafe { *self.scheduled_tail.get() == *self.scheduled_head.get() } {
            // queue is empty: wait for a signal
            if let Ok(new_lock) = notifier.wait(scheduled_tasks) {
                // update our lock
                scheduled_tasks = new_lock;
            } else {
                // wait failed: very bad. Return None is the best we can do here.
                return None;
            }

            // exit empty in case reset or stop signal have been received
            if self.stop_signal.load(Ordering::Acquire)
                || thread_ctx.reset_signal.load(Ordering::Acquire)
            {
                return None;
            }
        }

        // here we have data in the queue and hold the lock: take the entry
        let task = scheduled_tasks[unsafe { *self.scheduled_head.get() }].take();

        // and update
        unsafe { *self.scheduled_head.get() = (*self.scheduled_head.get() + 1) % self.ntasks };

        // update counter
        self.waiting_tasks.fetch_sub(1, Ordering::Release);

        // done
        task
    }

    /// Run the engine.
    ///
    /// This is the thread procedure of each thread in the pool.
    /// The threads cooperatively run tasks until the queue is empty or the engine is stopped.
    fn run(self: Arc<Self>, thread_ctx: Arc<ThreadContext>) -> RtoResult<()> {
        // signal that the thread is ready
        thread_ctx.transition_to(ThreadState::Ready);
        self.running_threads.fetch_add(1, Ordering::Relaxed);

        // run the state machine until we are stopped
        loop {
            // our idle flag
            // Idle means, we are not in a Running state but wait for it. When we are idle, we yield until we can transit into running or stop.
            // We do not lock on a signal as this would cause a race condition and we would miss eventual wakeups.
            let mut idle = false;

            // check engine stop and reset signal
            if self.stop_signal.load(Ordering::Acquire) {
                // stop signal
                thread_ctx.transition_to(ThreadState::Stopping);
            } else if thread_ctx.reset_signal.load(Ordering::Acquire) {
                // reset signal
                thread_ctx.transition_to(ThreadState::Resetting);
            }

            // process the run state
            match thread_ctx.state() {
                ThreadState::Created => {
                    // this cannot happen as the Created state only persists until this thread starts running.
                    thread_ctx.transition_to(ThreadState::Error);
                    debug_assert!(false);
                }

                ThreadState::Ready => {
                    // we are initialized (or reset) and ready to start
                    if ExecutionState::from_u32(self.state.load(Ordering::Acquire))
                        == ExecutionState::Running
                    {
                        thread_ctx.transition_to(ThreadState::RunningIdle);
                    } else {
                        idle = true;
                    }
                }

                ThreadState::RunningIdle => {
                    // we stay here until we don't
                    loop {
                        // check engine stop and reset signal
                        if self.stop_signal.load(Ordering::Acquire) {
                            // stop signal
                            thread_ctx.transition_to(ThreadState::Stopping);
                            break;
                        } else if thread_ctx.reset_signal.load(Ordering::Acquire) {
                            // reset signal
                            thread_ctx.transition_to(ThreadState::Resetting);
                            break;
                        }

                        // we are running but have no task: get a task or wait for one
                        if let Some(task) = self.pop_task_or_wait(&thread_ctx) {
                            // increase the working threads
                            self.working_threads.fetch_add(1, Ordering::Relaxed);

                            // immediately update state
                            thread_ctx.transition_to(ThreadState::RunningBusy);

                            // help other waiting threads
                            if self.waiting_tasks.load(Ordering::Relaxed) > 0 {
                                self.scheduled_tasks.1.notify_one();
                            }

                            // run the task
                            task.transition_to(TaskState::Running);

                            // create our waker
                            let waker = waker::signal_waker(thread_ctx.signal.clone());

                            // run the task until done. we are, after all, a thread here
                            loop {
                                let task_ctx = &mut std::task::Context::from_waker(&waker);

                                match task.poll(task_ctx)? {
                                    Poll::Ready(_) => {
                                        // task is done, result is unit: break inner loop
                                        task.transition_to(TaskState::Completed);
                                        break;
                                    }
                                    Poll::Pending => {
                                        // task not done: utilize the waker
                                        thread_ctx.signal.wait_timeout(Duration::from_millis(10));
                                    }
                                }
                            }

                            // count active tasks and busy threads
                            self.working_threads.fetch_sub(1, Ordering::Relaxed);
                        }
                    }
                }

                ThreadState::RunningBusy => {
                    // this cannot happen as the RunningBusy state is only set within the RunningIdle state
                    thread_ctx.transition_to(ThreadState::Error);
                    debug_assert!(false);
                }

                ThreadState::Stopping => {
                    // stop the engine, so we stop processing tasks
                    break;
                }

                ThreadState::Resetting => {
                    // reset the reset signal
                    thread_ctx.reset_signal.store(false, Ordering::Release);

                    // reset the thread
                    thread_ctx.transition_to(ThreadState::Ready);
                    idle = false;
                }

                ThreadState::Terminated => {
                    // this cannot happen as the Terminated state is only set when the thread leaving the run loop
                    thread_ctx.transition_to(ThreadState::Error);
                    debug_assert!(false);
                }

                ThreadState::Error => {
                    // error state: terminate the thread
                    break;
                }
            }

            // check if we are idle and yield
            if idle {
                std::thread::yield_now();
            }
        }

        // finally signal termination
        self.running_threads.fetch_sub(1, Ordering::Relaxed);
        thread_ctx.transition_to(ThreadState::Terminated);
        Ok(())
    }

    /// Spawn a new task on the engine.
    ///
    /// This will schedule the task for execution. The task will not immediately start if all available threads of the engine are busy.
    pub(crate) fn spawn<T>(self: &Arc<Self>, task: Task<T>) -> RtoResult<TaskHandle<T>>
    where
        T: Send + 'static,
    {
        // build inner task control
        let handle = Arc::new(TaskHandleInner::new(task));

        // notify task (now through the handle) that it is scheduled
        handle.transition_to(TaskState::Scheduled);

        // push the task wrapped into it's handle onto the engine's task queue
        if let Err(err) = self.push_task(handle.clone()) {
            // notify task
            handle.transition_to(TaskState::Error);
            return Err(err);
        }

        // done
        Ok(TaskHandle::new(handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_function() -> i32 {
        42 // what else?
    }

    #[test]
    fn test_engine_stm() {
        const NTHREADS: usize = 2;
        const NTASKS: usize = 32;

        // engine with builder
        let engine = EngineBuilder::new()
            .with_threads(NTHREADS)
            .with_tasks(NTASKS)
            .build()
            .unwrap();

        // uninitialized state
        assert_eq!(engine.thread_capacity(), NTHREADS);
        assert_eq!(engine.task_capacity(), NTASKS);
        assert_eq!(engine.state(), ExecutionState::Uninitialized);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);
        assert_eq!(engine.thread_utilization(), (0, 0));
        assert_eq!(engine.task_utilization(), (0, 0));

        // Wrong: try to join uninitialized engine
        assert_eq!(
            engine.inner.join(),
            Err(Error::const_new(
                rto_errors::INVALID_OPERATION,
                "Cannot join uninitialized engine"
            ))
        );

        // Good: initialize engine
        assert_eq!(engine.init(), Ok(ExecutionState::Ready));
        assert_eq!(engine.state(), ExecutionState::Ready);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);
        //std::thread::sleep(Duration::from_millis(500)); // give threads time to start
        //assert_eq!(engine.thread_utilization(), (0, engine.thread_capacity()));
        assert_eq!(engine.task_utilization(), (0, 0));

        // Wrong: try to initialize again
        assert_eq!(
            engine.init(),
            Err(Error::new(
                rto_errors::INVALID_OPERATION,
                format!("Cannot initialize engine from `Ready` state")
            ))
        );

        // Good: reset engine
        assert_eq!(engine.reset(), Ok(ExecutionState::Ready));
        std::thread::sleep(Duration::from_millis(500)); // give threads time to start

        // Good: spawn a task in Ready state
        let try_spawn = engine.spawn(Task::new(|| async { 42 }));
        assert_eq!(try_spawn.is_ok(), true); // spawn is ok
        let handle = try_spawn.unwrap();
        assert_eq!(engine.state(), ExecutionState::Ready);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);
        assert_eq!(engine.thread_utilization(), (0, engine.thread_capacity()));
        assert_eq!(engine.task_utilization(), (0, 1)); // no tasks are running, of a total of 1

        // Good: reset engine
        assert_eq!(engine.reset(), Ok(ExecutionState::Ready));
        assert_eq!(engine.state(), ExecutionState::Ready);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);
        assert_eq!(engine.thread_utilization(), (0, engine.thread_capacity()));
        assert_eq!(engine.task_utilization(), (0, 0)); // no tasks are running, of a total of 0

        assert_eq!(handle.state(), TaskState::Cancelled); // quick check on handle, but this is not our focus here

        // Good: start engine
        assert_eq!(engine.start(), Ok(ExecutionState::Running));
        assert_eq!(engine.state(), ExecutionState::Running);
        assert_eq!(engine.is_running(), true);
        assert_eq!(engine.is_terminating(), false);
        assert_eq!(engine.thread_utilization(), (0, engine.thread_capacity()));
        assert_eq!(engine.task_utilization(), (0, 0));

        // Wrong: try to start again
        assert_eq!(
            engine.start(),
            Err(Error::new(
                rto_errors::INVALID_OPERATION,
                format!("Cannot start engine while in state `Running`")
            ))
        );

        // Good: spawn a task in Running state
        let task = crate::task::from_async(test_function);
        let try_spawn = engine.spawn(task);
        assert_eq!(try_spawn.is_ok(), true); // spawn is ok

        // // wait for 10s
        // std::thread::sleep(Duration::from_secs(10));

        // Good: wait for the task to finish and check the result.
        // this is a big one: all thread handling happens in the background. go figure!
        assert_eq!(try_spawn.unwrap().join().unwrap(), 42); // wait for the task to finish

        // test shutdown, by inner phases
        assert_eq!(engine.inner.signal_stop(), Ok(()));
        assert_eq!(engine.state(), ExecutionState::Running);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), true);
        assert_eq!(engine.inner.join(), Ok(ExecutionState::Completed));
        assert_eq!(engine.state(), ExecutionState::Completed);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);

        // Good: terminate engine
        assert_eq!(engine.inner.terminate(), Ok(ExecutionState::Terminated));
        assert_eq!(engine.state(), ExecutionState::Terminated);
        assert_eq!(engine.is_running(), false);
        assert_eq!(engine.is_terminating(), false);
    }

    // #[test]
    // fn test_task_spawn() {
    //     let engine = Engine::new(1, 8);

    //     engine.start().unwrap();

    //     let func = || async { 42 };
    //     let task = Task::new(func());
    //     let handle = engine.spawn(task).unwrap();
    //     assert_eq!(handle.is_finished(), false);
    //     assert_eq!(handle.join().unwrap(), 42);

    //     engine.shutdown().unwrap();
    // }
}
