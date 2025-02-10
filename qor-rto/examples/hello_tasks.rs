// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

/// The Hello World routine
async fn hello_crowd(i: u32) {
    println!("Hello Person #{i}!"); // Hello the world
}

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // Our task handles
    let mut handles = Vec::new();

    for i in 0..20 {
        // Create a new task from our hello_world routine
        let task = Task::new(move || hello_crowd(i + 1));

        // Spawn the task on the engine
        let handle = engine.spawn(task).unwrap();
        handles.push(handle);
    }

    // Wait for the tasks to finish
    while let Some(handle) = handles.pop() {
        let _ = handle.join().unwrap();
    }

    // Engine shutdown
    engine.shutdown().unwrap();
}
