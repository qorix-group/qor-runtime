// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::{future::Future, pin::Pin, sync::Mutex, time::Duration};

/// Invoke routine
type InvokeRoutine = Box<dyn FnMut(&Duration) -> (Duration, UpdateResult) + Send + 'static>;

/// The lifecycle of an activity.
pub trait ActivityLifecycle: Send {
    fn init(&mut self) -> Result<(), Error>;
    fn start(&mut self) -> Result<(), Error>;
    fn update(&mut self, delta: &Duration) -> (Duration, UpdateResult);
    fn finalize(&mut self) -> Result<(), Error>;
    fn terminate(&mut self) -> Result<(), Error>;
}

/// An Activity is an action that controls the user-implemented Lifecycle model.
///
/// The activity will internally manage the action states but otherwise delegate the execution to the user-defined routine.
pub struct Activity {
    state: ExecutionState,
    lifecycle: Box<dyn ActivityLifecycle>,
}

impl Activity {
    /// Creates a new Activity action with given user-defined action.
    #[inline(always)]
    pub fn new(action: impl ActivityLifecycle + 'static) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            lifecycle: Box::new(action),
        })
    }
}

impl Debug for Activity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ptr: *const () = &*self.lifecycle as *const _ as *const ();
        write!(f, "activity {:?};", ptr)
    }
}

impl Action for Activity {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                if self.lifecycle.init().is_err() {
                    self.state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Init))
                } else {
                    self.state.transition_to(ExecutionState::Ready)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Init)),
        }
    }

    fn start(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                if self.lifecycle.start().is_err() {
                    self.state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Start))
                } else {
                    self.state.transition_to(ExecutionState::Running)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Start)),
        }
    }

    fn update(
        &mut self,
        delta: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        match self.state.peek_state(ExecutionTransition::Update) {
            ExecutionState::Running => self.lifecycle.update(delta),
            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if self.lifecycle.finalize().is_err() {
                    self.state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Finalize))
                } else {
                    self.state.transition_to(ExecutionState::Completed)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize)),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                if self.lifecycle.terminate().is_err() {
                    self.state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Terminate))
                } else {
                    self.state.transition_to(ExecutionState::Terminated)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Terminate)),
        }
    }
}

/// Update is a synchronous call into a user-defined function that acts as a coroutine.
///
/// Update will block until the coroutine returns. This action is provided for simple and fast-returning coroutine.
///
/// The coroutine gets the elapsed time since the last invocation and must return the remaining time and the update result.
/// The `UpdateResult` indicates the state of the coroutine after the invocation. Utilizing `UpdateResult` provides the full
/// flexibility to the user to control the coroutine execution.
///
/// The coroutine must return immediately from it's invocation. If the coroutine needs to block, it must implement a state machine internally
/// and return `UpdateResult::Busy` when it needs more time to complete.
///
/// For long-running routines, consider using the `Spawn` and `Await` actions.
pub struct Invoke {
    state: ExecutionState,
    routine: InvokeRoutine,
}

impl Invoke {
    /// Creates a new Call action with given routine
    #[inline(always)]
    pub fn new(
        routine: impl FnMut(&Duration) -> (Duration, UpdateResult) + Send + 'static,
    ) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            routine: Box::new(routine),
        })
    }
}

impl Debug for Invoke {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let func_ptr: *const () = &*self.routine as *const _ as *const ();
        write!(f, "invoke {:?};", func_ptr)
    }
}

impl Action for Invoke {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Init))
    }

    fn start(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Start))
    }

    fn update(
        &mut self,
        delta: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        match self.state.peek_state(ExecutionTransition::Update) {
            ExecutionState::Running => (self.routine)(delta),
            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Finalize))
    }

    fn terminate(&mut self) -> ExecutionState {
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Terminate))
    }
}

/// Await is an asynchronous call into a user-defined function that acts as a coroutine.
///
/// The action is designed to work with Rust's async functions.
/// ```rust
/// use qor_rto::{Await, RoutineResult};
///
/// async fn await_action() -> RoutineResult {
///     Ok(())
/// }
///
/// let action = Await::new_fn(await_action);
/// ```
///
/// Await adopts Rust's async/await syntax to provide a seamless integration with the Rust ecosystem.
/// The precise signature of the function expected is `Fn() -> (dyn Future<Output = Result<Duration, Error>> + Send + 'static)`.
/// This is fulfilled by functions that are marked as `async` and return a `Result<Duration, Error>`.
/// Any user-defined function that fulfills this signature can be used with the Await action.
///  
pub struct Await {
    state: ExecutionState,

    /// The routine wrapper. We need to Pin-Box the yet unknown future.
    /// We achieve this by creating a wrapping closure during `new` implementation.
    routine: Box<
        dyn Fn() -> Pin<Box<dyn Future<Output = RoutineResult> + Send + 'static>> + Send + 'static,
    >,

    /// The join handle of the task that runs the async function.
    join_handle: Option<TaskHandle<RoutineResult>>,
}

impl Await {
    /// Creates a new Await action with given routine.
    ///
    /// The Await action is prepared to work with Rust' async functions.
    ///
    /// ```rust
    /// use qor_rto::{Await, RoutineResult};
    ///
    /// async fn await_action() -> RoutineResult {
    ///     Ok(())
    /// }
    ///
    /// let action = Await::new_fn(await_action);
    /// ```
    #[inline(always)]
    pub fn new_fn<F, Fut>(routine: F) -> Box<Self>
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = RoutineResult> + Send + 'static,
    {
        Box::new(Await {
            state: ExecutionState::new(),

            // wrap the routine in something we know, so no dynamic sizes here.
            routine: Box::new(move || Box::pin(routine())),
            join_handle: None,
        })
    }

    /// Create a new Await action with a given method of a type instance, taking
    /// `&self` as argument
    pub fn new_method<T, F>(this: &Arc<Mutex<T>>, routine: F) -> Box<Self>
    where
        T: Send + 'static,
        F: Fn(&T) -> RoutineResult + Send + core::marker::Sync + 'static,
    {
        let this_clone = this.clone();
        let routine = Arc::new(routine);
        Self::new_fn(move || {
            let this_clone = this_clone.clone();
            let routine = routine.clone();
            async move { routine(&this_clone.lock().unwrap()) }
        })
    }

    /// Create a new Await action with a given method of a type instance, taking
    /// `&mut self` as argument
    pub fn new_method_mut<T, F>(this: &Arc<Mutex<T>>, routine: F) -> Box<Self>
    where
        T: Send + 'static,
        F: Fn(&mut T) -> RoutineResult + Send + core::marker::Sync + 'static,
    {
        let this_clone = this.clone();
        let routine = Arc::new(routine);
        Self::new_fn(move || {
            let this_clone = this_clone.clone();
            let routine = routine.clone();
            async move { routine(&mut this_clone.lock().unwrap()) }
        })
    }
}

impl Debug for Await {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let func_ptr: *const () = &*self.routine as *const _ as *const ();
        write!(f, "await {:?};", func_ptr)
    }
}

impl Action for Await {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        //TODO: Here we must get the Executor context
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Init))
    }

    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // create a task for the async function by calling the wrapper to create the future
                let task = Task::from_future((self.routine)());

                // spawn the task and transition to running
                match ctx.spawn(task) {
                    Ok(join_handle) => {
                        // ok, we have a join handle and can transition to running
                        self.join_handle = Some(join_handle);
                        self.state.transition_to(ExecutionState::Running)
                    }
                    Err(_) => {
                        // when spawn fails we are screwed here
                        self.state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Start))
                    }
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Start)),
        }
    }

    fn update(
        &mut self,
        delta: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        match self.state.peek_state(ExecutionTransition::Update) {
            ExecutionState::Running => {
                if let Some(join_handle) = &self.join_handle {
                    // check if the join handle is finished
                    if join_handle.is_finished() {
                        // we are done. so perform the join and return the result
                        if let Ok(result) = self.join_handle.take().unwrap().join() {
                            match result {
                                Ok(_) => (Duration::ZERO, UpdateResult::Complete),
                                Err(_) => (Duration::ZERO, UpdateResult::Err),
                            }
                        } else {
                            // join failed for whatever reason
                            (Duration::ZERO, UpdateResult::Err)
                        }
                    } else if join_handle.has_error() {
                        // we have an error
                        (Duration::ZERO, UpdateResult::Err)
                    } else {
                        // we need more time
                        (*delta, UpdateResult::Busy)
                    }
                } else {
                    // we are still running but have no join handle: very bad
                    (Duration::ZERO, UpdateResult::Err)
                }
            }
            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if let Some(join_handle) = self.join_handle.take() {
                    // we have not joined yet but we need to finalize
                    let _ = join_handle.join().unwrap();
                }
                self.state.transition_to(ExecutionState::Completed)
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize)),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        if let Some(join_handle) = self.join_handle.take() {
            // we have not joined yet but we need to terminate
            let _ = join_handle.join().unwrap();
        }

        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Terminate))
    }
}
