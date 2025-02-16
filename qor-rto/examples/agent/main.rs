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
    vec
};

pub mod activity;
pub mod agent;

use crate::activity::Activity;
use crate::activity::Activity1a;
use agent::Agent;

fn main() {
    let act:Arc<Mutex<Activity1a>> = Arc::new(Mutex::new(Activity1a::new("Activity1a".to_string())));
    let acta:Arc<Mutex<Activity1a>> = Arc::new(Mutex::new(Activity1a::new("Activity1b".to_string())));
    let actb:Arc<Mutex<Activity1a>> = Arc::new(Mutex::new(Activity1a::new("Activity2a".to_string())));
    let actc:Arc<Mutex<Activity1a>> = Arc::new(Mutex::new(Activity1a::new("Activity2b".to_string())));
    let activities = vec![act,acta,actb,actc];
    
    let agent = Agent::new(&activities);
    // agent.init();
    // agent.run();
    // agent.terminate();

    agent.run();
}
