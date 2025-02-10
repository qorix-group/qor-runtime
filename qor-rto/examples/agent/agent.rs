// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::activity::Activity;

use qor_rto::prelude::*;
use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

pub struct Agent{
    engine: Engine,
}

impl Agent {
    //should take the task chain as input later
    pub fn new() -> Self {
        Self {
            engine:Engine::default()
        }
    }

    pub fn init(&self,this: &Arc<Mutex<Activity>>){
        self.engine.start().unwrap();

        let name = "Activity1a";
        let init_event = IpcEvent::new(&format!("{name}_init"));
        let init_event_ack = IpcEvent::new(&format!("{name}_init_ack"));

        let pgminit = Program::new().with_action(
                Sequence::new()
                .with_step(Sync::new(init_event.listener().unwrap()))
                .with_step(Await::new_method_mut(this, Activity::init))
                .with_step(Trigger::new(init_event_ack.notifier().unwrap()))
        );

        let handle = pgminit.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();


    }

    pub fn run(&self,this: &Arc<Mutex<Activity>>) {

        println!("reach");
        let name = "Activity1a";

        let step_event = IpcEvent::new(&format!("{name}_step"));
        let step_event_ack = IpcEvent::new(&format!("{name}_step_ack"));

        let pgminit = Program::new().with_action(
            ForRange::new(10).with_body(
                Sequence::new()
                    //step
                    .with_step(Sync::new(step_event.listener().unwrap()))
                    .with_step(Await::new_method_mut(&this, Activity::step))
                    .with_step(Trigger::new(step_event_ack.notifier().unwrap())),
            ),
        );

        let handle = pgminit.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();

    }

    pub fn terminate(&self,this: &Arc<Mutex<Activity>>){

        let name = "Activity1a";
        let term_event = IpcEvent::new(&format!("{name}_term"));
        let term_event_ack = IpcEvent::new(&format!("{name}_term_ack"));

        let pgminit = Program::new().with_action(
                Sequence::new()
                    //terminate
                    .with_step(Sync::new(term_event.listener().unwrap()))
                    .with_step(Await::new_method_mut(&this, Activity::terminate))
                    .with_step(Trigger::new(term_event_ack.notifier().unwrap())),
        );

        let handle = pgminit.spawn(&self.engine).unwrap();

        // Wait for the program to finish
        let _ = handle.join().unwrap();

        // Engine shutdown
        self.engine.shutdown().unwrap();

    }
}
