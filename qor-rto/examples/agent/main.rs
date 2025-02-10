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

pub mod activity;
pub mod agent;

use activity::Activity;
use agent::Agent;

fn main() {
    let act = Arc::new(Mutex::new(Activity::new("activity1a")));
    let agent=Agent::new();
    agent.init(&act);
    agent.run(&act);
    agent.terminate(&act);
}
