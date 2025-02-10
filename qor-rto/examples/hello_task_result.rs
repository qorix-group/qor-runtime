// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

/// The Hello World routine
async fn hello_world(name: &str) -> String {
    // return a welcome message
    format!("Hello {}!", name)
}

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // The name to greet
    let name = "World";

    // Create a new task from our hello_world routine an pass the name in a capture
    let task = Task::new(|| async { hello_world(name).await });

    // Spawn the task on the engine
    let handle = engine.spawn(task).unwrap();

    // Wait for the task to finish and collect the message
    let message = handle.join().unwrap();

    // Print the message
    println!("{}", message);

    // Engine shutdown
    engine.shutdown().unwrap();
}
