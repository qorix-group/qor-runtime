// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

use std::time::Duration;

/// Nop is a no-operation action that does nothing
pub struct Nop {
    state: ExecutionState,
}

impl Nop {
    /// Creates a new no-operation action
    pub fn new() -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
        })
    }
}

impl Debug for Nop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "nop;")
    }
}

impl Action for Nop {
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
        if self
            .state
            .transition_to(self.state.peek_state(ExecutionTransition::Update))
            == ExecutionState::Running
        {
            (Duration::ZERO, UpdateResult::Complete)
        } else {
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
