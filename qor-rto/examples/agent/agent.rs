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
use std::collections::HashMap;

fn generate_ipc_events(activities: &Vec<Arc<Mutex<dyn Activity>>>) -> HashMap<String, HashMap<String, Event<IpcEvent>>> {
    let mut events_map: HashMap<String, HashMap<String, Event<IpcEvent>>> = HashMap::new();

    for activity in activities.iter() {
        let mut event_submap: HashMap<String, Event<IpcEvent>>= HashMap::new();

        event_submap.insert("init".to_string(), IpcEvent::new(&format!("{}_init", activity.lock().unwrap().getname())));
        event_submap.insert("init_ack".to_string(), IpcEvent::new(&format!("{}_init_ack", activity.lock().unwrap().getname())));
        event_submap.insert("step".to_string(), IpcEvent::new(&format!("{}_step", activity.lock().unwrap().getname())));
        event_submap.insert("step_ack".to_string(), IpcEvent::new(&format!("{}_step_ack", activity.lock().unwrap().getname())));
        event_submap.insert("term".to_string(), IpcEvent::new(&format!("{}_term", activity.lock().unwrap().getname())));
        event_submap.insert("term_ack".to_string(), IpcEvent::new(&format!("{}_term_ack", activity.lock().unwrap().getname())));

        events_map.insert(activity.lock().unwrap().getname(), event_submap);
    }

    events_map
}



pub struct Agent<'a>{
    engine: Engine,
    ipc_events:HashMap<String, HashMap<String, Event<IpcEvent>>>,
    activities: &'a Vec<Arc<Mutex<dyn Activity>>>
}

impl<'a> Agent<'a> {
    //should take the task chain as input later
    pub fn new(this: &'a Vec<Arc<Mutex<dyn Activity>>>) -> Self {
        Self {
            engine: Engine::default(),
            ipc_events:generate_ipc_events(this),
            activities: this
        }
    }

    fn init(&self)-> Box<dyn Action>{

        let mut top_sequence = Sequence::new();
        
         for activity in self.activities.iter() {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("init").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(activity, Activity::init))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("init_ack").unwrap().notifier().unwrap()));

            top_sequence= top_sequence.with_step(sub_sequence);
     
         }

         top_sequence
    }

    fn step(&self)-> Box<dyn Action>{

        let mut top_sequence = Concurrency::new();
        
         for activity in self.activities {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("step").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(activity, Activity::step))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("step_ack").unwrap().notifier().unwrap()));


            top_sequence= top_sequence.with_branch(sub_sequence);
        
         }
    
         top_sequence
    }

    fn terminate(&self)-> Box<dyn Action>{

        let mut top_sequence = Sequence::new();
        
         for activity in self.activities.iter() {
            let name= &activity.lock().unwrap().getname();
            let sub_sequence =         Sequence::new()
            .with_step(Sync::new(self.ipc_events.get(name).unwrap().get("term").unwrap().listener().unwrap()))
            .with_step(Await::new_method_mut_u(&activity.clone(), Activity::terminate))
            .with_step(Trigger::new(self.ipc_events.get(name).unwrap().get("term_ack").unwrap().notifier().unwrap()));


            top_sequence= top_sequence.with_step(sub_sequence);
        
         }
    
         top_sequence
    }


    pub fn run(&self){
        self.engine.start().unwrap();
        println!("reach");

        let pgminit = Program::new().with_action(
                 Sequence::new()
                     //step
                     .with_step(
                            self.init(),
                )
                 .with_step(
                     ForRange::new(10).with_body(
                        self.step(),
                     )
                 )
                 .with_step(
                    self.terminate(),
                 ),
        );

        
        let handle = pgminit.spawn(&self.engine).unwrap();

                // Wait for the program to finish
        let _ = handle.join().unwrap();

    }


}
