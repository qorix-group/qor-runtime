// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// our simulation period
const PERIOD: Duration = Duration::from_millis(500);
const DEADLINE: Duration = Duration::from_millis(60);

struct Activity {
    name: String,
    last: Instant,
}

impl Activity {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            last: Instant::now(),
        }
    }

    fn init(&mut self) -> RoutineResult {
        let now = Instant::now();
        println!(
            "{:09}: {}: init",
            (now - self.last).as_nanos() / 1000,
            self.name
        );
        self.last = now;
        Ok(())
    }

    fn step(&mut self) -> RoutineResult {
        let now = Instant::now();
        println!(
            "{:09}: {}: step",
            (now - self.last).as_nanos() / 1000,
            self.name
        );
        self.last = now;

        // some "work"
        for i in 0..1000 {
            // busy wait
            if i % 100 == 0 {
                std::thread::yield_now();
            }
            unsafe {
                std::ptr::read_volatile(&i);
            }
        }

        Ok(())
    }

    fn terminate(&mut self) -> RoutineResult {
        let now = Instant::now();
        println!(
            "{:09}: {}: terminate",
            (now - self.last).as_nanos() / 1000,
            self.name
        );
        self.last = now;
        Ok(())
    }
}

// Macro: A cyclic timer action
fn cyclic_timer<Adapter: EventAdapter + 'static>(
    period: Duration,
    notifier: Notifier<Adapter>,
) -> Box<dyn Action> {
    Loop::new().with_body(
        Sequence::new()
            .with_step(Sleep::new(period))
            .with_step(Trigger::new(notifier)),
    )
}

/// A Task chain simulation. We have 7 activities, 1 receiver, 2 parallel activities with each 2 activities in sequence and 1 sender.
///
/// ```text
///        | [2a] -> [2b] |
/// [1] -> | [3a] -> [3b] | -> [4] -> [5]
/// ```
///
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // timer syncronization event
    let timer_event = SingleEvent::new();

    // our termination event
    let stop_event = SingleEvent::new();

    // our activities & clones for move into the async closures
    let activity_1 = Arc::new(Mutex::new(Activity::new("1 Receiver")));
    let activity_2a = Arc::new(Mutex::new(Activity::new("2a")));
    let activity_2b = Arc::new(Mutex::new(Activity::new("2b")));
    let activity_3a = Arc::new(Mutex::new(Activity::new("3a")));
    let activity_3b = Arc::new(Mutex::new(Activity::new("3b")));
    let activity_4 = Arc::new(Mutex::new(Activity::new("4")));
    let activity_5 = Arc::new(Mutex::new(Activity::new("5 Sender")));

    // Create a new program with a timer, a model function and a termination event
    let program = Program::new().with_action(
        Sequence::new()
            // Init all activities
            .with_step(
                Concurrency::new()
                    .with_branch(Await::new_method_mut(&activity_1, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_2a, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_2b, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_3a, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_3b, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_4, Activity::init))
                    .with_branch(Await::new_method_mut(&activity_5, Activity::init)),
            )
            // The main loop
            .with_step(
                Computation::new()
                    // the cyclic timer action, macros just insert the action
                    .with_branch(cyclic_timer(PERIOD, timer_event.notifier().unwrap()))
                    // the termination action:
                    .with_branch(Sync::new(stop_event.listener().unwrap()))
                    // the main program:
                    .with_branch(
                        Loop::new().with_body(
                            Sequence::new()
                                // wait for timer event
                                .with_step(Sync::new(timer_event.listener().unwrap()))
                                .with_step(
                                    TryCatch::new(ExecutionExceptionFilter::for_timeout())
                                        .with_try(
                                            Computation::new()
                                                .with_branch(
                                                    // The good path: Taskchain
                                                    Sequence::new()
                                                        .with_step(Await::new_method_mut(
                                                            &activity_1,
                                                            Activity::step,
                                                        ))
                                                        .with_step(
                                                            Concurrency::new()
                                                                .with_branch(
                                                                    Sequence::new()
                                                                        .with_step(
                                                                            Await::new_method_mut(
                                                                                &activity_2a,
                                                                                Activity::step,
                                                                            ),
                                                                        )
                                                                        .with_step(
                                                                            Await::new_method_mut(
                                                                                &activity_2b,
                                                                                Activity::step,
                                                                            ),
                                                                        ),
                                                                )
                                                                .with_branch(
                                                                    Sequence::new()
                                                                        .with_step(
                                                                            Await::new_method_mut(
                                                                                &activity_3a,
                                                                                Activity::step,
                                                                            ),
                                                                        )
                                                                        .with_step(
                                                                            Await::new_method_mut(
                                                                                &activity_3b,
                                                                                Activity::step,
                                                                            ),
                                                                        ),
                                                                ),
                                                        )
                                                        .with_step(Await::new_method_mut(
                                                            &activity_4,
                                                            Activity::step,
                                                        ))
                                                        .with_step(Await::new_method_mut(
                                                            &activity_5,
                                                            Activity::step,
                                                        )),
                                                )
                                                .with_branch(
                                                    // The bad path: Throw on timeout
                                                    Sequence::new()
                                                        .with_step(Sleep::new(DEADLINE))
                                                        .with_step(Throw::new(
                                                            ExecutionException::timeout(),
                                                        )),
                                                ),
                                        )
                                        .with_catch(Invoke::new(|_| {
                                            println!("Task chain timing violation!");
                                            (Duration::ZERO, UpdateResult::Complete)
                                        })),
                                ),
                        ),
                    ),
            )
            // Terminate all activities
            .with_step(
                Concurrency::new()
                    .with_branch(Await::new_method_mut(&activity_1, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_2a, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_2b, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_3a, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_3b, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_4, Activity::terminate))
                    .with_branch(Await::new_method_mut(&activity_5, Activity::terminate)),
            ),
    );

    println!("\nStarting task chain simulation");

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    // here we wait for some time
    std::thread::sleep(Duration::from_secs(5));

    // ok, enough
    stop_event.notifier().unwrap().notify();

    // Wait for the program to finish
    let _ = handle.join().unwrap();

    println!("Task chain simulation finished");

    // Engine shutdown
    engine.shutdown().unwrap();
}
