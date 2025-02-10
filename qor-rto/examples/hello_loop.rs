// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;
use std::time::Duration;

/// The Hello World Routine with the correct signature
fn hello_world(_delta: &Duration) -> (Duration, UpdateResult) {
    println!("Hello World!"); // Hello the world
    (Duration::ZERO, UpdateResult::Complete) // And declare the action as complete
}

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // Create a new program with a loop of 10 invoke actions
    let program = Program::new().with_action(
        ForRange::new(10).with_body(
            Sequence::new()
                .with_step(Invoke::new(hello_world))
                .with_step(Sleep::new(Duration::from_secs(1))),
        ),
    );

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    // Wait for the program to finish
    let _ = handle.join().unwrap();

    // Engine shutdown
    engine.shutdown().unwrap();
}
