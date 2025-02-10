// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use crate::rto_errors;

use std::time::Duration;

/// Sequence is an action that executes a sequence of actions called step in consecutive order
///
pub struct Sequence {
    state: ExecutionState,
    actions: Vec<Box<dyn Action>>,
    current: usize,
}

/// Implement the From trait for `Vec<Box<dyn Action>>` to convert a vector of actions into a Sequence
impl From<Vec<Box<dyn Action>>> for Sequence {
    fn from(actions: Vec<Box<dyn Action>>) -> Self {
        Self {
            state: ExecutionState::new(),
            actions,
            current: 0,
        }
    }
}

/// Implement the From trait to derive a vector of actions from a sequence
impl From<Sequence> for Vec<Box<dyn Action>> {
    fn from(val: Sequence) -> Self {
        val.actions
    }
}

impl Debug for Sequence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;

        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 {
                write!(f, " -> ")?;
            }

            write!(f, "{:?}", action)?;
        }

        write!(f, "}}")
    }
}

impl Sequence {
    /// Creates a new Sequence action
    #[inline(always)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            actions: Vec::new(),
            current: 0,
        })
    }

    /// Adds a new action to the Sequence at initialization
    pub fn with_step(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add action to initialized `Sequence` action");
        }
        self.actions.push(action);
        self
    }

    /// Adds a new action to the Sequence
    pub fn add_step(&mut self, action: Box<dyn Action>) -> RtoResult<()> {
        if self.state() != ExecutionState::Uninitialized {
            return Err(Error::const_new(
                rto_errors::INVALID_STATE,
                "Cannot add action to initialized `Sequence` action",
            ));
        }

        self.actions.push(action);
        Ok(())
    }
}

impl Action for Sequence {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                for action in self.actions.iter_mut() {
                    if action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                    }
                }

                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                self.current = 0;

                // start first action, if present
                if self.current < self.actions.len()
                    && self.actions[self.current].start(ctx) != ExecutionState::Running
                {
                    return self
                        .state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                }

                // and transition to running state
                self.state.transition_to(ExecutionState::Running)
            }

            _ => ExecutionState::Err(ExecutionTransition::Start),
        }
    }

    fn update(&mut self, delta: &Duration, ctx: &mut ExecutionContext) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                let mut remaining = *delta;
                let mut total_elapsed = Duration::ZERO;

                while self.current < self.actions.len() {
                    let (elapsed, result) = self.actions[self.current].update(&remaining, ctx);

                    // adjust delta and total elapsed
                    debug_assert!(elapsed <= remaining); // the child action consumed more time than was provided
                    remaining -= elapsed;
                    total_elapsed += elapsed;

                    // index in range, update current action
                    match result {
                        // Busy action makes busy Sequence
                        UpdateResult::Busy => {
                            return (total_elapsed, UpdateResult::Busy);
                        }

                        // Ready or Complete action iterates Sequence
                        UpdateResult::Complete | UpdateResult::Ready => {
                            // stop current action
                            if self.actions[self.current].finalize(ctx) != ExecutionState::Completed
                            {
                                self.state.transition_to(ExecutionState::Err(
                                    ExecutionTransition::Update,
                                ));
                                return (total_elapsed, UpdateResult::Err);
                            }

                            // iterate to next action
                            self.current += 1;

                            // start next action, if present
                            if self.current < self.actions.len()
                                && self.actions[self.current].start(ctx) != ExecutionState::Running
                            {
                                self.state.transition_to(ExecutionState::Err(
                                    ExecutionTransition::Update,
                                ));
                                return (total_elapsed, UpdateResult::Err);
                            }
                        }

                        UpdateResult::Interruption(interruption) => {
                            // Interruptions back-propagate with added elapsed time
                            return (total_elapsed, UpdateResult::Interruption(interruption));
                        }

                        // Err back-propagates
                        UpdateResult::Err => return (total_elapsed, UpdateResult::Err),
                    }
                }
                (total_elapsed, UpdateResult::Complete)
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                // check if we have an action active
                if self.current < self.actions.len() {
                    // then finalize
                    if self.actions[self.current].finalize(ctx) != ExecutionState::Completed {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Finalize));
                    }
                }

                self.state.transition_to(ExecutionState::Completed)
            }

            _ => ExecutionState::Err(ExecutionTransition::Finalize),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                for action in self.actions.iter_mut() {
                    if action.terminate() != ExecutionState::Terminated {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Terminate));
                    }
                }

                self.state.transition_to(ExecutionState::Terminated)
            }

            _ => ExecutionState::Err(ExecutionTransition::Terminate),
        }
    }
}

/// A Concurrency is an action that executes other actions in multiple branches concurrently.
/// It completes when all concurrent actions have completed.
/// One branch throwing an exception will cause the Concurrency to throw the exception as well.
pub struct Concurrency {
    state: ExecutionState,
    actions: Vec<Box<dyn Action>>,
}

impl Concurrency {
    /// Creates a new Concurrency action
    #[inline(always)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            actions: Vec::new(),
        })
    }

    /// Adds a new action to the Concurrency at initialization
    pub fn with_branch(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add action to initialized `Concurrency` action");
        }
        self.actions.push(action);
        self
    }

    /// Adds an action as branch to an existing Concurrency
    pub fn add_branch(&mut self, action: Box<dyn Action>) -> RtoResult<()> {
        if self.state() != ExecutionState::Uninitialized {
            return Err(Error::const_new(
                rto_errors::INVALID_STATE,
                "Cannot add action to initialized `Concurrency` action",
            ));
        }

        self.actions.push(action);
        Ok(())
    }
}

/// Implement the From trait for `Vec<Box<dyn Action>>` to convert a vector of actions into a Sequence
impl From<Vec<Box<dyn Action>>> for Concurrency {
    fn from(actions: Vec<Box<dyn Action>>) -> Self {
        Self {
            state: ExecutionState::new(),
            actions,
        }
    }
}

/// Implement the From trait to derive a vector of actions from a Concurrency
impl From<Concurrency> for Vec<Box<dyn Action>> {
    fn from(val: Concurrency) -> Self {
        val.actions
    }
}

impl Debug for Concurrency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;

        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 {
                write!(f, " || ")?;
            }

            write!(f, "{:?}", action)?;
        }

        write!(f, "}}")
    }
}

impl Action for Concurrency {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                for action in self.actions.iter_mut() {
                    if action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Init));
                    }
                }

                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // start all actions
                for action in self.actions.iter_mut() {
                    if action.start(ctx) != ExecutionState::Running {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                    }
                }

                // and transition to running state
                self.state.transition_to(ExecutionState::Running)
            }

            _ => ExecutionState::Err(ExecutionTransition::Start),
        }
    }

    fn update(&mut self, delta: &Duration, ctx: &mut ExecutionContext) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                let mut total_elapsed = Duration::ZERO;

                // update all `Running` branches
                let mut busy = false;
                for action in self.actions.iter_mut() {
                    if action.state() == ExecutionState::Running {
                        let (elapsed, result) = action.update(delta, ctx);

                        // adjust delta and total elapsed
                        debug_assert!(elapsed <= *delta); // the child action consumed more time than was provided
                        total_elapsed = total_elapsed.max(elapsed); // longest branch matters (actually, they are supported to either give ZERO or `delta`)

                        // update with same delta for all
                        match result {
                            UpdateResult::Busy => {
                                // one branch busy makes me busy as well
                                busy = true;
                            }

                            UpdateResult::Complete | UpdateResult::Ready => {
                                // finalize this branch
                                if action.finalize(ctx) != ExecutionState::Completed {
                                    self.state.transition_to(ExecutionState::Err(
                                        ExecutionTransition::Update,
                                    ));
                                    return (total_elapsed, UpdateResult::Err);
                                }
                            }

                            UpdateResult::Interruption(interruption) => {
                                // Interruptions back-propagate with added elapsed time
                                return (total_elapsed, UpdateResult::Interruption(interruption));
                            }

                            UpdateResult::Err => return (total_elapsed, UpdateResult::Err),
                        }
                    }
                }

                // now we are either busy or done.
                if busy {
                    (total_elapsed, UpdateResult::Busy)
                } else {
                    (total_elapsed, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                // finalize all actions
                for action in self.actions.iter_mut() {
                    // check if we are still active
                    if action.state() == ExecutionState::Running {
                        // then finalize
                        if action.finalize(ctx) != ExecutionState::Completed {
                            return self
                                .state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize));
                        }
                    }
                }

                self.state.transition_to(ExecutionState::Completed)
            }

            _ => ExecutionState::Err(ExecutionTransition::Finalize),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                for action in self.actions.iter_mut() {
                    if action.terminate() != ExecutionState::Terminated {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Terminate));
                    }
                }

                self.state.transition_to(ExecutionState::Terminated)
            }

            _ => ExecutionState::Err(ExecutionTransition::Terminate),
        }
    }
}

/// A Computation is an action that executes multiple actions concurrently.
/// It completes
///
/// - with `UpdateResult::Ready` when all of the concurrent actions are `UpdateState::Ready` or
/// - with `UpdateResult::Complete` one of them is `UpdateState::Complete`.
///
/// A branch throwing an exception will cause the Computation to throw the exception as well.
///
/// The Computation is a complex concurrent action. It can be used to model runtime races as well as repetitive calculations
/// as required in generators or simulations.
///
/// Example: Execute a task and monitor it through a timeout
/// ```ignore
///     // computation {
///     //   await(my_task),
///     //   throw(Timeout, 100ms)
///     // }
///     let Computation = Computation::new()
///                         .with(Box::new(Await::new(async { /* my task */})))           // Await the asynchronous task
///                         .with(Box::new(Throw::timeout(Duration::from_millis(100))));  // Race against a timeout
/// ```
///
/// Explanation: The Computation will concurrently execute the asynchronous task and the `Throw Timeout` action with a timeout of 100ms.
/// Both task and timeout will report `UpdateState::Busy` until the task completes or the timeout expires. If the task completes first
/// the Computation will complete successfully. If the timeout expires first, the Computation will throw the timeout exception.
/// Wrapping this action into a `TryCatch` action will allow to catch the timeout exception and handle it:
///
/// ```ignore
///   // try {
///   //    computation {
///   //       await(my_task),
///   //       throw(Timeout, 100ms)
///   //    }
///   // } catch(Timeout) {
///   //    /* exception handling goes here */
///   // }
///   //
///   let trycatch = TryCatch::for_timeout()
///     .with_try(Box::new(Computation))
///     .with_catch(Box::new(Await::new(async { /* handle timeout */ })));
///
///    trycatch.init().unwrap();
///    trycatch.start().unwrap();
///    assert_eq!(trycatch.update(&Duration::from_millis(50), UpdateResult::Busy));
///    assert_eq!(trycatch.update(&Duration::from_millis(50), UpdateResult::Complete));
///    trycatch.finalize().unwrap();
///    trycatch.terminate().unwrap();
/// ```   
pub struct Computation {
    state: ExecutionState,
    actions: Vec<Box<dyn Action>>,
}

impl Computation {
    /// Creates a new Concurrency action
    #[inline(always)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            actions: Vec::new(),
        })
    }

    /// Adds a new action to the Computation at initialization
    pub fn with_branch(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add action to initialized `Computation` action");
        }
        self.actions.push(action);
        self
    }

    /// Adds an action as branch to the Computation
    pub fn add_branch(&mut self, action: Box<dyn Action>) -> RtoResult<()> {
        if self.state() != ExecutionState::Uninitialized {
            return Err(Error::const_new(
                rto_errors::INVALID_STATE,
                "Cannot add action to initialized `Computation` action",
            ));
        }
        self.actions.push(action);
        Ok(())
    }
}

/// Implement the From trait for `Vec<Box<dyn Action>>` to convert a vector of actions into a Sequence
impl From<Vec<Box<dyn Action>>> for Computation {
    fn from(actions: Vec<Box<dyn Action>>) -> Self {
        Self {
            state: ExecutionState::new(),
            actions,
        }
    }
}

/// Implement the From trait to derive a vector of actions from a Computation
impl From<Computation> for Vec<Box<dyn Action>> {
    fn from(val: Computation) -> Self {
        val.actions
    }
}

impl Debug for Computation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{")?;

        for (i, action) in self.actions.iter().enumerate() {
            if i > 0 {
                write!(f, " || ")?;
            }

            write!(f, "{:?}", action)?;
        }

        write!(f, "}}")
    }
}

impl Action for Computation {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                // init all actions
                for action in self.actions.iter_mut() {
                    if action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Init));
                    }
                }

                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // start all actions
                for action in self.actions.iter_mut() {
                    if action.start(ctx) != ExecutionState::Running {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                    }
                }

                // and transition to running state
                self.state.transition_to(ExecutionState::Running)
            }

            _ => ExecutionState::Err(ExecutionTransition::Start),
        }
    }

    fn update(&mut self, delta: &Duration, ctx: &mut ExecutionContext) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                let mut total_elapsed = Duration::ZERO;

                // update all `Running` branches
                let mut busy = false;
                for action in self.actions.iter_mut() {
                    if action.state() == ExecutionState::Running {
                        let (elapsed, result) = action.update(delta, ctx);

                        // adjust delta and total elapsed
                        debug_assert!(elapsed <= *delta); // the child action consumed more time than was provided
                        total_elapsed = total_elapsed.max(elapsed); // longest branch matters (actually, they are supported to either give ZERO or `delta`)

                        // update with same delta for all
                        match result {
                            UpdateResult::Busy => {
                                // one branch busy makes us busy as well
                                busy = true;
                            }

                            UpdateResult::Complete => {
                                // one branch complete completes us immediately (branch order matters!)
                                return (total_elapsed, UpdateResult::Complete);
                            }

                            UpdateResult::Ready => {
                                // one branch ready does nothing, we will rerun the branch next time
                                // This is what `Ready` was made for.
                            }

                            UpdateResult::Interruption(interruption) => {
                                // Interruptions back-propagate with added elapsed time
                                return (total_elapsed, UpdateResult::Interruption(interruption));
                            }

                            UpdateResult::Err => return (total_elapsed, UpdateResult::Err),
                        }
                    }
                }

                // now we are either busy or done.
                if busy {
                    (total_elapsed, UpdateResult::Busy)
                } else {
                    // if we are not busy all branches are ready, which auto-completes the computation with `Ready`
                    (total_elapsed, UpdateResult::Ready)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                // finalize all actions
                for action in self.actions.iter_mut() {
                    // check if we are still active
                    if action.state() == ExecutionState::Running {
                        // then finalize
                        if action.finalize(ctx) != ExecutionState::Completed {
                            return self
                                .state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize));
                        }
                    }
                }

                self.state.transition_to(ExecutionState::Completed)
            }

            _ => ExecutionState::Err(ExecutionTransition::Finalize),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                for action in self.actions.iter_mut() {
                    if action.terminate() != ExecutionState::Terminated {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Terminate));
                    }
                }

                self.state.transition_to(ExecutionState::Terminated)
            }

            _ => ExecutionState::Err(ExecutionTransition::Terminate),
        }
    }
}
