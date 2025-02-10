// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::{fmt::Debug, time::Duration};

/// Sleep is an action that waits for a specified duration
pub struct Sleep {
    state: ExecutionState,
    duration: Duration,
    remaining: Duration,
}

impl Sleep {
    /// Creates a new `Sleep` action
    #[inline(always)]
    pub fn new(duration: Duration) -> Box<Self> {
        Box::new(Sleep {
            state: ExecutionState::new(),
            duration,
            remaining: Duration::from_secs(0),
        })
    }
}

impl Debug for Sleep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.duration.as_secs() > 0 {
            write!(f, "sleep({}s);", self.duration.as_secs())
        } else {
            write!(f, "sleep({}ms);", self.duration.as_millis())
        }
    }
}

impl Action for Sleep {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                self.remaining = Duration::ZERO;
                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                self.remaining = self.duration;
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
                if self.remaining > *delta {
                    self.remaining -= *delta;
                    (*delta, UpdateResult::Busy)
                } else {
                    // we consumed the remaining
                    let remaining = self.remaining;
                    self.remaining = Duration::ZERO;

                    // return remaining as passed
                    (remaining, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                self.remaining = Duration::ZERO;
                self.state.transition_to(ExecutionState::Completed)
            }

            _ => ExecutionState::Err(ExecutionTransition::Finalize),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                self.duration = Duration::ZERO;
                self.state.transition_to(ExecutionState::Terminated)
            }

            _ => ExecutionState::Err(ExecutionTransition::Terminate),
        }
    }
}

/// Sync is an action that waits for a given `Event` to occur.
pub struct Sync<Adapter: EventAdapter> {
    state: ExecutionState,
    listener: Listener<Adapter>,
}

impl<Adapter: EventAdapter> Debug for Sync<Adapter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sync({:?});", self.listener)
    }
}

impl<Adapter: EventAdapter> Sync<Adapter> {
    /// Creates a new no-operation action
    #[inline(always)]
    pub fn new(listener: Listener<Adapter>) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            listener,
        })
    }
}

impl<Adapter: EventAdapter> Action for Sync<Adapter> {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                // subscribe to the event
                self.state.transition_to(ExecutionState::Ready)
            }

            _ => ExecutionState::Err(ExecutionTransition::Init),
        }
    }

    fn start(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                self.listener.check_and_reset();
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
                if self.listener.check_and_reset() {
                    (Duration::ZERO, UpdateResult::Complete)
                } else {
                    (*delta, UpdateResult::Busy)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, _ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => self.state.transition_to(ExecutionState::Completed),

            _ => ExecutionState::Err(ExecutionTransition::Finalize),
        }
    }

    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => self.state.transition_to(ExecutionState::Terminated),

            _ => ExecutionState::Err(ExecutionTransition::Terminate),
        }
    }
}

/// Trigger is an action that triggers the given event
pub struct Trigger<Adapter: EventAdapter> {
    state: ExecutionState,
    notifier: Notifier<Adapter>,
}

impl<Adapter: EventAdapter> Debug for Trigger<Adapter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "trigger({:?})", self.notifier)
    }
}

impl<Adapter: EventAdapter> Trigger<Adapter> {
    /// Creates a new `Trigger` action
    #[inline(always)]
    pub fn new(notifier: Notifier<Adapter>) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            notifier,
        })
    }
}

impl<Adapter: EventAdapter> Action for Trigger<Adapter> {
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
        _delta: &Duration,
        _ctx: &mut ExecutionContext,
    ) -> (Duration, UpdateResult) {
        match self.state {
            ExecutionState::Running => {
                // trigger the event
                self.notifier.notify();
                (Duration::ZERO, UpdateResult::Complete)
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
