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
    time::Duration,
};

// our model period
const PERIOD: Duration = Duration::from_millis(50);
struct Model {
    k: f64, // model parameter: amplification factor
    t: f64, // model parameter: time constant

    x: f64, // model state variable
    u: f64, // input signal
    y: f64, // output signal

    cycle: usize,
}

impl Model {
    fn new(k: f64, t: f64) -> Self {
        Self {
            k,
            t: if t == 0.0 { f64::EPSILON } else { t },
            x: 0.0,
            u: 1.0,
            y: 0.0,
            cycle: 0,
        }
    }

    /// A simple PT1 model-function
    fn update(&mut self) -> RoutineResult {
        // output signal
        self.y = self.x;

        // model equation
        let x_dot = (self.k * self.u - self.x) / self.t;

        // integration
        self.x += x_dot * PERIOD.as_secs_f64();

        // because this is a "hello model": output
        self.cycle += 1;
        println!("{:03}, {}", self.cycle, self.y);

        // we are done but can be called again (because we are a model)
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

/// A Hello World program
fn main() {
    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // timer syncronization event
    let timer = SingleEvent::new();

    // our termination event
    let stop = SingleEvent::new();

    // our model with internal state
    let model = Arc::new(Mutex::new(Model::new(1.0, 1.0))); // this is 'our' copy

    // Create a new program with a timer, a model function and a termination event
    let program = Program::new().with_action(
        Computation::new()
            .with_branch(
                // the cyclic timer action, macros just insert the action
                cyclic_timer(PERIOD, timer.notifier().unwrap()),
            )
            .with_branch(
                // the main program:
                Loop::new().with_body(
                    Sequence::new()
                        .with_step(Sync::new(timer.listener().unwrap()))
                        .with_step(Await::new_method_mut(&model, Model::update)),
                ),
            )
            .with_branch(
                // the termination action:
                Sync::new(stop.listener().unwrap()),
            ),
    );

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    // here we wait for some time
    std::thread::sleep(Duration::from_secs(2));

    // let's change the input signal
    {
        model.lock().unwrap().u = 0.0;
    }
    std::thread::sleep(Duration::from_secs(2));

    // ok, enough
    stop.notifier().unwrap().notify();

    // Wait for the program to finish
    let _ = handle.join().unwrap();

    // Engine shutdown
    engine.shutdown().unwrap();
}
