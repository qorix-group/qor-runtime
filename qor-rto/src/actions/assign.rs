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

/// Assign copies the value from an RValue evaluation into an LValue
/// It is coherent with the `=` operator in Rust
pub struct Assign<T>
where
    T: TypeTag + Debug + Send + std::marker::Sync,
{
    state: ExecutionState,
    lhs: Arc<dyn LValue<T>>,
    rhs: Arc<dyn RValue<T>>,
}

impl<T> Debug for Assign<T>
where
    T: TypeTag + Debug + Send + std::marker::Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} = {:?};", self.lhs, self.rhs)
    }
}

impl<T> Assign<T>
where
    T: TypeTag + Debug + Send + std::marker::Sync,
{
    /// Creates a new assign action
    #[inline(always)]
    pub fn new(lhs: Arc<dyn LValue<T>>, rhs: Arc<dyn RValue<T>>) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            lhs,
            rhs,
        })
    }
}

impl<T> Action for Assign<T>
where
    T: TypeTag + Debug + Send + std::marker::Sync,
{
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
                if let Ok(value) = self.rhs.eval() {
                    if self.lhs.assign(value).is_ok() {
                        (Duration::ZERO, UpdateResult::Ready)
                    } else {
                        self.state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Update));
                        (Duration::ZERO, UpdateResult::Err)
                    }
                } else {
                    self.state
                        .transition_to(ExecutionState::Err(ExecutionTransition::Update));
                    (Duration::ZERO, UpdateResult::Err)
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
