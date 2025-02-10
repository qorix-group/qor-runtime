// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;

use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

pub struct Executor {
    engine: Engine,
}

impl Executor {
    //should take the task chain as input later
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
        }
    }

    pub fn init(&self) {
        self.engine.start().unwrap();

        let name = "Activity1a";
        let init_event = IpcEvent::new(&format!("{name}_init"));
        let init_event_ack = IpcEvent::new(&format!("{name}_init_ack"));

        let pgminit = Program::new().with_action(
            Sequence::new()
                .with_step(Trigger::new(init_event.notifier().unwrap()))
                .with_step(Sync::new(init_event_ack.listener().unwrap())),
        );

        let handle = pgminit.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();
    }

    pub fn run(&self) {
        let timer_event = SingleEvent::new();
        // our simulation period
        const PERIOD: Duration = Duration::from_millis(500);
        let tim_prog = Program::new().with_action(
            ForRange::new(10).with_body(
                Sequence::new()
                    .with_step(Sleep::new(PERIOD))
                    .with_step(Trigger::new(timer_event.notifier().unwrap())),
            ),
        );

        println!("reach exec run");
        let name = "Activity1a";
        let step_event = IpcEvent::new(&format!("{name}_step"));
        let step_event_ack = IpcEvent::new(&format!("{name}_step_ack"));

        let pgminit = Program::new().with_action(
            ForRange::new(10).with_body(
                Sequence::new()
                    .with_step(Sync::new(timer_event.listener().unwrap()))
                    .with_step(Trigger::new(step_event.notifier().unwrap()))
                    .with_step(Sync::new(step_event_ack.listener().unwrap())),
            ),
        );

        let handle = pgminit.spawn(&self.engine).unwrap();
        let handle2 = tim_prog.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();
        let _ = handle2.join().unwrap();

        // here we wait for some time
        std::thread::sleep(Duration::from_secs(2));

        println!("Done");
    }

    pub fn terminate(&self) {
        let name = "Activity1a";
        let term_event = IpcEvent::new(&format!("{name}_term"));
        let term_event_ack = IpcEvent::new(&format!("{name}_term_ack"));

        let pgminit = Program::new().with_action(
            Sequence::new()
                .with_step(Trigger::new(term_event.notifier().unwrap()))
                .with_step(Sync::new(term_event_ack.listener().unwrap())),
        );

        let handle = pgminit.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();

        // Engine shutdown
        self.engine.shutdown().unwrap();
    }
}
