// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

/// The Hello World async routine
async fn hello_world() -> RoutineResult {
    println!("Hello World!"); // Hello the world
    Ok(()) // And declare the action as complete
}

/// A Hello World program with an synchronous routine
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // Create a new program
    let program = Program::new().with_action(Await::new_fn(hello_world));

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    // Wait for the program to finish
    let _ = handle.join().unwrap();

    // Engine shutdown
    engine.shutdown().unwrap();
}
