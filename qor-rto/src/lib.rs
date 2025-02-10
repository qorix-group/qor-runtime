// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

//! Runtime Orchestration (RTO) is the central runtime control element of the Qore stack
//!
//! # Overview
//!
//! # Examples
//!
//! ```rust
//! use qor_rto::prelude::*;
//! use std::time::Duration;
//!
//! // The Hello World Routine with the correct signature
//! fn hello_world(_delta: &Duration) -> (Duration, UpdateResult) {
//!     println!("Hello World!"); // Hello the world
//!     (Duration::ZERO, UpdateResult::Complete) // And declare the action as complete
//! }
//!
//! // A Hello World program
//! fn main() {
//!     // The engine is the central runtime executor
//!     let engine = Engine::default();
//!     engine.start().unwrap();
//!
//!     // Create a new program with a single invoke action
//!     let program = Program::new().with_action(Invoke::new(hello_world));
//!
//!     // Spawn the program on the engine
//!     let handle = program.spawn(&engine).unwrap();
//!
//!     // Wait for the program to finish
//!     let _ = handle.join().unwrap();
//!
//!     // Engine shutdown
//!     engine.shutdown().unwrap();
//! }
//!
//!
//! ```
//!
//! Further examples can be found in the examples directory of the repository.

pub mod prelude {
    pub use crate::base::*;

    pub use crate::Engine;
    pub use crate::EngineBuilder;
    pub use crate::Program;
    pub use crate::Task;
    pub use crate::TaskHandle;
    pub use crate::UpdateResult;

    pub use crate::actions::*;
    pub use crate::event::*;
    pub use crate::expressions::*;
    pub use crate::variables::*;
}

pub mod rto_errors;

/// Base module for the Runtime Orchestration (RTO) stack
mod base;
pub use base::*;

/// Variable and State objects for the Runtime Orchestration (RTO) stack
mod variables;
pub use variables::*;

/// Event objects for the Runtime Orchestration (RTO) stack
mod event;
pub use event::*;

/// Expression objects for the Runtime Orchestration (RTO) stack
mod expressions;
pub use expressions::*;

/// Task objects for the Runtime Orchestration (RTO) stack
mod task;
pub use task::{Task, TaskHandle};

/// Executor module
mod executor;
mod waker;
pub use executor::{Engine, EngineBuilder};

/// Action objects for the Runtime Orchestration (RTO) stack
mod actions;
pub use actions::*;

/// Program module
mod program;
pub use program::*;
