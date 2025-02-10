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

/// IfThenElse is an action that executes a sequence of actions in order
pub struct IfThenElse {
    state: ExecutionState,
    condition: Arc<dyn RValue<bool>>,
    then_action: Option<Box<dyn Action>>,
    else_action: Option<Box<dyn Action>>,
    active_branch: bool,
}

impl Debug for IfThenElse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "if ({:?}) then ", self.condition)?;

        if let Some(action) = &self.then_action {
            write!(f, "{{ {:?} }}", action)?;
        } else {
            write!(f, "{{}}")?;
        }

        if let Some(action) = &self.else_action {
            write!(f, " else {{ {:?} }}", action)?;
        }

        write!(f, "")
    }
}

impl IfThenElse {
    /// Creates a new concurrency action
    #[inline(always)]
    pub fn new(condition: Arc<dyn RValue<bool>>) -> Box<Self> {
        Box::new(Self {
            state: ExecutionState::new(),
            condition,
            then_action: None,
            else_action: None,
            active_branch: false,
        })
    }

    /// Adds a new "then" action to the If-The-Else action
    pub fn with_then(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add `then` action to initialized `IfThenElse` action");
        }
        self.then_action = Some(action);
        self
    }

    /// Adds a new "else" action to the If-The-Else action
    pub fn with_else(mut self: Box<Self>, action: Box<dyn Action>) -> Box<Self> {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add `else` action to initialized `IfThenElse` action");
        }
        self.else_action = Some(action);
        self
    }
}

impl Action for IfThenElse {
    fn state(&self) -> ExecutionState {
        self.state
    }

    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                // init then, if set
                if let Some(action) = &mut self.then_action {
                    if action.init() != ExecutionState::Ready {
                        return self
                            .state
                            .transition_to(ExecutionState::Err(ExecutionTransition::Init));
                    }
                }

                // init else, if set
                if let Some(action) = &mut self.else_action {
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
                // check condition by evaluating rvalue
                if let Ok(condition) = self.condition.eval() {
                    // set active branch
                    self.active_branch = condition;

                    // start then or else action
                    if self.active_branch {
                        if let Some(action) = &mut self.then_action {
                            if action.start(ctx) != ExecutionState::Running {
                                return self.state.transition_to(ExecutionState::Err(
                                    ExecutionTransition::Start,
                                ));
                            }
                        }
                    } else if let Some(action) = &mut self.else_action {
                        if action.start(ctx) != ExecutionState::Running {
                            return self
                                .state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Start));
                        }
                    }
                } else {
                    return ExecutionState::Err(ExecutionTransition::Start);
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
                // check on active branch, update then or else action
                if self.active_branch {
                    if let Some(action) = &mut self.then_action {
                        action.update(delta, ctx)
                    } else {
                        (Duration::ZERO, UpdateResult::Complete)
                    }
                } else if let Some(action) = &mut self.else_action {
                    action.update(delta, ctx)
                } else {
                    (Duration::ZERO, UpdateResult::Complete)
                }
            }

            _ => (Duration::ZERO, UpdateResult::Err),
        }
    }

    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                // finalize then or else action
                if self.active_branch {
                    if let Some(action) = &mut self.then_action {
                        if action.finalize(ctx) != ExecutionState::Completed {
                            return self
                                .state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize));
                        }
                    }
                } else if let Some(action) = &mut self.else_action {
                    if action.finalize(ctx) != ExecutionState::Completed {
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
                // terminate then or else action
                if self.active_branch {
                    if let Some(action) = &mut self.then_action {
                        if action.terminate() != ExecutionState::Terminated {
                            return self.state.transition_to(ExecutionState::Err(
                                ExecutionTransition::Terminate,
                            ));
                        }
                    }
                } else if let Some(action) = &mut self.else_action {
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
