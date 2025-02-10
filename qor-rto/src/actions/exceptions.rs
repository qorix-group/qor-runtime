// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::time::Duration;

/// Throw action throws an exception with an optional timeout
/// The timeout comes in handy when the throw is used in a parallel action in a `Computation`,
/// thus controlling the main execution flow.
pub struct Throw {
    state: ExecutionState,
    exception: ExecutionException,
    timeout: Duration,
    remaining: Duration,
}

impl Throw {
    /// Creates an (initially immediate) throw action for the given exception
    #[inline(always)]
    pub fn new(exception: ExecutionException) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            exception,
            timeout: Duration::ZERO,
            remaining: Duration::ZERO,
        })
    }

    /// Creates a timeout exception with a given timeout
    #[inline(always)]
    pub fn timeout(timeout: Duration) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            exception: ExecutionException::timeout(),
            timeout,
            remaining: Duration::ZERO,
        })
    }

    /// Add a timeout to the exception
    pub fn with_timeout(mut self: Box<Self>, timeout: Duration) -> Box<Self> {
        self.timeout = timeout;
        self
    }
}

impl Debug for Throw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "throw {:?};", self.exception)
    }
}

impl Action for Throw {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        self.state
            .transition_to(self.state.peek_state(ExecutionTransition::Init))
    }

    fn start(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // reset timeout
                self.remaining = self.timeout;
                self.state.transition_to(ExecutionState::Running)
            }

            _ => ExecutionState::Err(ExecutionTransition::Start),
        }
    }

    fn update(
        &mut self,
        delta: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                // check for timeout
                if *delta < self.remaining {
                    self.remaining -= *delta;
                    (*delta, UpdateResult::Busy)
                } else {
                    // throw exception
                    (
                        self.remaining,
                        UpdateResult::Interruption(ExecutionInterruption::Exception(
                            self.exception,
                        )),
                    )
                }
            }

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

/// TryCatch is an action that executes a try action and catches exceptions with a given
/// exception mask. The catch action is executed if an exception is thrown that matches the mask.
pub struct TryCatch {
    state: ExecutionState,
    try_action: Option<Box<dyn Action>>,
    catch_action: Option<Box<dyn Action>>,
    filter: ExecutionExceptionFilter,

    try_active: bool,
}

impl Debug for TryCatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "try {{")?;
        if let Some(action) = &self.try_action {
            write!(f, " {:?} ", action)?;
        }
        write!(f, "}} catch ({}) {{", self.filter)?;
        if let Some(action) = &self.catch_action {
            write!(f, " {:?} ", action)?;
        }
        write!(f, "}}")
    }
}

impl TryCatch {
    /// Creates a new empty try-catch action
    #[inline(always)]
    pub fn new(filter: ExecutionExceptionFilter) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            try_action: None,
            catch_action: None,
            filter,
            try_active: false,
        })
    }

    /// Creates a new try-catch action that catches all exceptions
    #[inline(always)]
    pub fn catch_all() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            try_action: None,
            catch_action: None,
            filter: ExecutionExceptionFilter::for_all(),
            try_active: false,
        })
    }

    /// Set the try action
    pub fn with_try(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add `try` action to initialized `TryCatch` action");
        }
        self.try_action = Some(action);
        self
    }

    /// Set the catch action
    pub fn with_catch(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add `catch` action to initialized `TryCatch` action");
        }
        self.catch_action = Some(action);
        self
    }
}

impl Action for TryCatch {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                // init try action
                if let Some(try_action) = &mut self.try_action {
                    if try_action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Init));
                    }
                }

                // init catch action
                if let Some(catch_action) = &mut self.catch_action {
                    if catch_action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Init));
                    }
                }

                // ok
                self.try_active = false;
                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                // start try action
                if let Some(try_action) = &mut self.try_action {
                    if try_action.start(ctx) != ExecutionState::Running {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                    }
                }
                self.try_active = true;
                self.state.transition_to(ExecutionState::Running)
            }

            _ => ExecutionState::Err(ExecutionTransition::Start),
        }
    }

    fn update(&mut self, delta: &Duration, ctx: &mut ExecutionContext) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                let mut total_elapsed = Duration::ZERO;

                if self.try_active {
                    // try branch is active
                    if let Some(try_action) = &mut self.try_action {
                        let (elapsed, result) = try_action.update(delta, ctx);
                        debug_assert!(elapsed <= *delta);

                        // adjust total elapsed time
                        total_elapsed += elapsed;

                        match result {
                            UpdateResult::Busy => return (total_elapsed, UpdateResult::Busy),

                            UpdateResult::Ready | UpdateResult::Complete => {
                                // try is done, no exception thrown: continue
                                return (total_elapsed, UpdateResult::Complete);
                            }

                            UpdateResult::Interruption(interruption) => {
                                // try is done, we got an interruption. check if it is an exception
                                if let ExecutionInterruption::Exception(ex) = interruption {
                                    // check catched exception against mask
                                    if self.filter.matches(&ex) {
                                        // exception matches mask -> execute catch action
                                        if let Some(catch_action) = &mut self.catch_action {
                                            // finalize try action
                                            if try_action.finalize(ctx) != ExecutionState::Completed
                                            {
                                                self.state.transition_to(ExecutionState::Err(
                                                    ExecutionTransition::Update,
                                                ));
                                                return (total_elapsed, UpdateResult::Err);
                                            }

                                            if catch_action.start(ctx) != ExecutionState::Running {
                                                self.state.transition_to(ExecutionState::Err(
                                                    ExecutionTransition::Update,
                                                ));
                                                return (total_elapsed, UpdateResult::Err);
                                            }

                                            // activate catch action, continue below
                                            self.try_active = false;
                                        } else {
                                            // no catch action -> we are done
                                            return (total_elapsed, UpdateResult::Complete);
                                        }
                                    } else {
                                        // exception does not match mask -> back-propagate interruption
                                        return (
                                            total_elapsed,
                                            UpdateResult::Interruption(interruption),
                                        );
                                    }
                                } else {
                                    // not an exception -> back-propagate interruption
                                    return (
                                        total_elapsed,
                                        UpdateResult::Interruption(interruption),
                                    );
                                }

                                // if we get here catch has to be active
                                debug_assert!(!self.try_active);
                                debug_assert_eq!(self.state, ExecutionState::Running);
                            }

                            UpdateResult::Err => return (total_elapsed, UpdateResult::Err),
                        }
                    } else {
                        // no try action -> nothing to do
                        return (Duration::ZERO, UpdateResult::Complete);
                    }
                }

                // handle the catch action
                if !self.try_active {
                    // remaining delta
                    let remaining = *delta - total_elapsed;

                    if let Some(action) = &mut self.catch_action {
                        let (elapsed, result) = action.update(&remaining, ctx);
                        debug_assert!(elapsed <= remaining);
                        total_elapsed += elapsed;

                        match result {
                            UpdateResult::Busy => (total_elapsed, UpdateResult::Busy),

                            UpdateResult::Complete | UpdateResult::Ready => {
                                (total_elapsed, UpdateResult::Complete)
                            }

                            UpdateResult::Interruption(interruption) => {
                                // back-propagate any interruption from catch action
                                (total_elapsed, UpdateResult::Interruption(interruption))
                            }

                            UpdateResult::Err => (total_elapsed, UpdateResult::Err),
                        }
                    } else {
                        // no catch action -> we are done
                        (total_elapsed, UpdateResult::Complete)
                    }
                } else {
                    // this is for branch completeness. we never get here
                    panic!();
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if self.try_active {
                    // finalize try action
                    if let Some(try_action) = &mut self.try_action {
                        if try_action.finalize(ctx) != ExecutionState::Completed {
                            return self
                                .state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize));
                        }
                    }
                } else {
                    // finalize catch action
                    if let Some(catch_action) = &mut self.catch_action {
                        if catch_action.finalize(ctx) != ExecutionState::Completed {
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
                // terminate try action
                if let Some(try_action) = &mut self.try_action {
                    if try_action.terminate() != ExecutionState::Terminated {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Terminate));
                    }
                }

                // terminate catch action
                if let Some(catch_action) = &mut self.catch_action {
                    if catch_action.terminate() != ExecutionState::Terminated {
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
