// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::sync::Arc;
use std::time::Duration;

/// Break action exits the loop it is in.
pub struct Break {
    state: ExecutionState,
}

impl Debug for Break {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "break;")
    }
}

impl Break {
    /// Creates a new Break action
    #[inline(always)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
        })
    }
}

impl Action for Break {
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
        _elapsed: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        if self.state == ExecutionState::Running {
            // signal break interruption
            (
                Duration::ZERO,
                UpdateResult::Interruption(ExecutionInterruption::Break),
            )
        } else {
            self.state
                .transition_to(ExecutionState::Err(ExecutionTransition::Update));
            (Duration::ZERO, UpdateResult::Err)
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

/// Loop is an infinite loop that loops it's body until a Break or an exception terminates the loop.
///
/// - A Loop without a body will complete immediately.
/// - A Loop without a Break in the body will never finish.
/// - A Loop that contains neither `Sleep`, `Sync` or `Await` will never become busy.
///
/// Effectively, a loop with a body containing neither of `Break`, `Sleep`, `Sync`, `Await` or `Throw`
/// will block a program.
pub struct Loop {
    state: ExecutionState,
    body: Option<Box<dyn Action>>,
}

impl Debug for Loop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(body) = &self.body {
            write!(f, "loop {{ {:?} }}", body)
        } else {
            write!(f, "loop <empty>")
        }
    }
}

impl Loop {
    /// Creates a new loop action
    #[inline(always)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            body: None,
        })
    }

    /// Initialize loop with a body
    pub fn with_body(mut self: Box<Self>, body: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add body to initialized `Loop` action");
        }

        self.body = Some(body);
        self
    }
}

impl Action for Loop {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                if let Some(body) = &mut self.body {
                    if body.init() != ExecutionState::Ready {
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
                if let Some(body) = &mut self.body {
                    if body.start(ctx) != ExecutionState::Running {
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
                if let Some(body) = &mut self.body {
                    let mut remaining = *delta;
                    let mut total_elapsed = Duration::ZERO;

                    // run the loop.
                    // apart from errors, this will exit only on break or other interruptions
                    loop {
                        let (elapsed, result) = body.update(&remaining, ctx);

                        // adjust delta and total elapsed
                        debug_assert!(elapsed <= remaining);
                        remaining -= elapsed;
                        total_elapsed += elapsed;

                        match result {
                            UpdateResult::Busy => return (total_elapsed, UpdateResult::Busy), // backpropagate busy

                            UpdateResult::Complete | UpdateResult::Ready => {
                                // finalize body...
                                if body.finalize(ctx) != ExecutionState::Completed {
                                    self.state.transition_to(ExecutionState::Err(
                                        ExecutionTransition::Update,
                                    ));
                                    return (total_elapsed, UpdateResult::Err);
                                }

                                // ...and restart
                                if body.start(ctx) != ExecutionState::Running {
                                    self.state.transition_to(ExecutionState::Err(
                                        ExecutionTransition::Update,
                                    ));
                                    return (total_elapsed, UpdateResult::Err);
                                }
                            }

                            UpdateResult::Interruption(interruption) => {
                                // check on break
                                match interruption {
                                    ExecutionInterruption::Break => {
                                        // break the loop
                                        return (total_elapsed, UpdateResult::Complete);
                                    }

                                    _ => {
                                        // back-propagate other interruptions
                                        return (
                                            total_elapsed,
                                            UpdateResult::Interruption(interruption),
                                        );
                                    }
                                }
                            }

                            UpdateResult::Err => return (total_elapsed, UpdateResult::Err), // backpropagate
                        }
                    }
                } else {
                    // we have no body
                    (Duration::ZERO, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if let Some(body) = &mut self.body {
                    if body.finalize(ctx) != ExecutionState::Completed {
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
                if let Some(body) = &mut self.body {
                    if body.terminate() != ExecutionState::Terminated {
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

/// ForRange is a finite loop that loops a fixed number of times without an explicit iterator, start and end value.
///
/// - A Break or an exception can terminate the loop early.
/// - A ForRange loop without a body will terminate immediately.
/// - A ForRange loop that contains neither `Sleep`, `Sync` or `Await` will never become busy.
pub struct ForRange {
    state: ExecutionState,
    body: Option<Box<dyn Action>>,
    iter: usize,
    count: usize,
}

impl Debug for ForRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(body) = &self.body {
            write!(f, "for {:?} {{ {:?} }}", self.count, body)
        } else {
            write!(f, "for {:?} {{}}", self.count)
        }
    }
}

impl ForRange {
    /// Creates a new loop action
    #[inline(always)]
    pub fn new(count: usize) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            body: None,
            iter: 0,
            count,
        })
    }

    /// Initialize loop with a body
    pub fn with_body(mut self: Box<Self>, body: Box<dyn Action>) -> Box<Self> {
        if self.state != ExecutionState::Uninitialized {
            panic!("Cannot add body to initialized `ForCount` action");
        }

        self.body = Some(body);
        self
    }
}

impl Action for ForRange {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                if let Some(body) = &mut self.body {
                    if body.init() != ExecutionState::Ready {
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
                // reset iterator
                self.iter = self.count;

                // check body
                if let Some(body) = &mut self.body {
                    // and go
                    if body.start(ctx) != ExecutionState::Running {
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
                if let Some(body) = &mut self.body {
                    let mut remaining = *delta;
                    let mut total_elapsed = Duration::ZERO;

                    // run the loop.
                    while self.iter > 0 {
                        let (elapsed, result) = body.update(&remaining, ctx);
                        debug_assert!(elapsed <= remaining);

                        // adjust delta and total elapsed
                        remaining -= elapsed;
                        total_elapsed += elapsed;

                        match result {
                            UpdateResult::Busy => {
                                // back-propagate busy
                                return (total_elapsed, UpdateResult::Busy);
                            }

                            UpdateResult::Complete | UpdateResult::Ready => {
                                // decrement iterator
                                self.iter -= 1;

                                // check for another round.
                                // We only finalize if we need to rerun.
                                // Otherwise notifying Complete will cause us and our body to finalize later
                                if self.iter > 0 {
                                    // finalize body...
                                    if body.finalize(ctx) != ExecutionState::Completed {
                                        self.state.transition_to(ExecutionState::Err(
                                            ExecutionTransition::Update,
                                        ));
                                        return (total_elapsed, UpdateResult::Err);
                                    }

                                    // ...and restart
                                    if body.start(ctx) != ExecutionState::Running {
                                        self.state.transition_to(ExecutionState::Err(
                                            ExecutionTransition::Update,
                                        ));
                                        return (total_elapsed, UpdateResult::Err);
                                    }
                                }
                            }

                            UpdateResult::Interruption(interruption) => {
                                return (total_elapsed, UpdateResult::Interruption(interruption))
                            }

                            UpdateResult::Err => return (total_elapsed, UpdateResult::Err), // back-propagate
                        }
                    }

                    // we are done with the loop
                    (total_elapsed, UpdateResult::Complete)
                } else {
                    // we have no body
                    (Duration::ZERO, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if let Some(body) = &mut self.body {
                    if body.finalize(ctx) != ExecutionState::Completed {
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
                if let Some(body) = &mut self.body {
                    if body.terminate() != ExecutionState::Terminated {
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

/// For is a finite loop that loops it's body for a definite number of times
/// A Break or an exception can terminate the loop early.
/// A For loop without a body will complete immediately.
/// A For loop that contains neither `Sleep`, `Sync` or `Await` will never become busy.
pub struct For {
    state: ExecutionState,
    body: Option<Box<dyn Action>>,
    iter: Arc<dyn Variable<StateType>>,
    begin: Arc<dyn RValue<StateType>>,
    end: Arc<dyn RValue<StateType>>,
}

impl Debug for For {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(body) = &self.body {
            write!(
                f,
                "for {:?} in {:?}..{:?} {{ {:?} }}",
                self.iter, self.begin, self.end, body
            )
        } else {
            write!(
                f,
                "for {:?} in {:?}..{:?} {{}}",
                self.iter, self.begin, self.end
            )
        }
    }
}

impl For {
    /// Creates a For loop action with iterator, begin and end values
    pub fn new(
        iter: Arc<dyn Variable<StateType>>,
        begin: Arc<dyn RValue<StateType>>,
        end: Arc<dyn RValue<StateType>>,
    ) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            body: None,
            iter,
            begin,
            end,
        })
    }

    /// Creates a new For loop action with a count
    /// This is a convenience function that creates a ForCount loop.
    #[inline(always)]
    pub fn range(count: usize) -> Box<ForRange> {
        ForRange::new(count)
    }

    /// Initialize loop with a body
    pub fn with_body(mut self: Box<Self>, body: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add body to initialized `For` action");
        }

        self.body = Some(body);
        self
    }
}

impl Action for For {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                if let Some(body) = &mut self.body {
                    if body.init() != ExecutionState::Ready {
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
                // check body
                if let Some(body) = &mut self.body {
                    // reset iterator
                    if let Ok(begin) = self.begin.eval() {
                        let _ = self.iter.assign(begin);
                    }

                    // and go
                    if body.start(ctx) != ExecutionState::Running {
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
                if let Some(body) = &mut self.body {
                    let mut remaining = *delta;
                    let mut total_elapsed = Duration::ZERO;

                    // run the loop.
                    loop {
                        if let Ok(mut iter) = self.iter.eval() {
                            if let Ok(end) = self.end.eval() {
                                if iter < end {
                                    let (elapsed, result) = body.update(&remaining, ctx);
                                    debug_assert!(elapsed <= remaining);

                                    // adjust delta and total elapsed
                                    remaining -= elapsed;
                                    total_elapsed += elapsed;

                                    match result {
                                        UpdateResult::Busy => {
                                            // back-propagate busy
                                            return (total_elapsed, UpdateResult::Busy);
                                        }

                                        UpdateResult::Complete | UpdateResult::Ready => {
                                            // increment iterator
                                            iter += 1;
                                            let _ = self.iter.assign(iter);

                                            // check for another round.
                                            // We only finalize if we need to rerun.
                                            // Otherwise notifying Complete will cause us and our body to finalize later
                                            if iter < end {
                                                // finalize body...
                                                if body.finalize(ctx) != ExecutionState::Completed {
                                                    self.state.transition_to(ExecutionState::Err(
                                                        ExecutionTransition::Update,
                                                    ));
                                                    return (total_elapsed, UpdateResult::Err);
                                                }

                                                // ...and restart
                                                if body.start(ctx) != ExecutionState::Running {
                                                    self.state.transition_to(ExecutionState::Err(
                                                        ExecutionTransition::Update,
                                                    ));
                                                    return (total_elapsed, UpdateResult::Err);
                                                }
                                            } else {
                                                // iterator is at the end. We are done here
                                                // (finalization will be issued by parent)
                                                return (total_elapsed, UpdateResult::Complete);
                                            }
                                        }

                                        UpdateResult::Interruption(interruption) => {
                                            return (
                                                total_elapsed,
                                                UpdateResult::Interruption(interruption),
                                            )
                                        }

                                        UpdateResult::Err => {
                                            return (total_elapsed, UpdateResult::Err)
                                        } // back-propagate
                                    }
                                } else {
                                    return (total_elapsed, UpdateResult::Complete);
                                }
                            } else {
                                self.state.transition_to(ExecutionState::Err(
                                    ExecutionTransition::Update,
                                ));
                                return (total_elapsed, UpdateResult::Err);
                            }
                        } else {
                            // reading our own iterator failed. This should never happen
                            debug_assert!(false);
                            self.state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Update));
                            return (total_elapsed, UpdateResult::Err);
                        }
                    }
                } else {
                    // we have no body
                    (Duration::ZERO, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if let Some(body) = &mut self.body {
                    if body.finalize(ctx) != ExecutionState::Completed {
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
                if let Some(body) = &mut self.body {
                    if body.terminate() != ExecutionState::Terminated {
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
