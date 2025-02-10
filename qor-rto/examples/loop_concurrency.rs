// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;
use std::{thread::sleep, time::Duration};

/// The Hello World routine, asynchronous variant
async fn hello_world() -> RoutineResult {
    println!("Hello World!"); // Hello the world
    sleep(Duration::from_millis(800));
    println!("World: Ack\n");
    Ok(()) // And declare the action as complete
}

/// The Hello World routine, asynchronous variant
async fn hello_beyond() -> RoutineResult {
    println!("Hello Beyond!"); // Hello the beyond
    sleep(Duration::from_millis(250));
    println!("Beyond: Ack");
    Ok(()) // And declare the action as complete
}

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // Create a new program with a loop of 10 invoke actions
    let program = Program::new().with_action(
        For::range(10).with_body(
            Sequence::new()
                .with_step(
                    Concurrency::new()
                        .with_branch(Await::new_fn(hello_world))
                        .with_branch(Await::new_fn(hello_beyond)),
                )
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
