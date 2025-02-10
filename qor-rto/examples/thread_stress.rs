// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

use qor_rto::prelude::*;

/// The Hello World Routine with the correct signature
async fn sim_task() -> RtoResult<()> {
    let n = 0;

    for i in 0..100000 {
        // busy wait
        if i % 100 == 0 {
            std::thread::yield_now();
        }
        unsafe {
            std::ptr::read_volatile(&n);
        }
    }

    Ok(())
}

const N_PROGRAMS: usize = 8;
const N_TASKS: usize = 250;
const N_THREADS: usize = 256;

fn make_program() -> Program {
    // Numbers of Hellos
    let mut flow = Concurrency::new();
    for _ in 0..N_TASKS {
        flow.add_branch(Await::new_fn(sim_task)).unwrap();
    }

    let flow = Sequence::new()
        .with_step(flow)
        .with_step(Sleep::new(Duration::from_millis(1000)));

    Program::new().with_action(ForRange::new(10).with_body(flow))
}

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = EngineBuilder::new()
        .with_threads(N_THREADS)
        .with_tasks(N_TASKS * N_PROGRAMS + 1)
        .build()
        .unwrap();

    engine.init().unwrap();

    // wait for the engine to start
    while engine.thread_utilization().1 < N_THREADS {
        std::thread::sleep(Duration::from_millis(10));
    }

    // initial setting
    println!(
        "Tasks: {:?}, Threads: {:?}",
        engine.task_utilization(),
        engine.thread_utilization()
    );

    // spawn N programs
    let mut handles = vec![];

    // flood the engine with N programs
    for _ in 0..N_PROGRAMS {
        let program = make_program();
        let handle = program.spawn(&engine).unwrap();
        handles.push(handle);
    }

    // start engine
    engine.start().unwrap();

    // Wait for the program to finish
    let mut finished = false;
    while !finished {
        finished = true;
        for handle in &handles {
            if !handle.is_finished() {
                finished = false;
                break;
            }
        }

        print!(
            "\rTasks: {:?}, Threads: {:?}        ",
            engine.task_utilization(),
            engine.thread_utilization()
        );

        //std::thread::yield_now();
        std::thread::sleep(Duration::from_millis(50));
    }

    println!("\nAll programs finished");
    for handle in handles {
        let _ = handle.join().unwrap();
    }

    // Engine shutdown
    engine.shutdown().unwrap();
}
