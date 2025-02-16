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

pub mod executor;

use executor::Executor;

use std::collections::HashMap;

fn main() {

    let names = vec!["Activity1a","Activity1b","Activity2a","Activity2b"];

    let dependency_graph: HashMap<&str, Vec<&str>> = HashMap::from([
        ("Activity1a", vec![]),
         ("Activity1a", vec!["Activity1b"]),     // B depends on A
         ("Activity1b", vec!["Activity2a","Activity2b"]),     // C depends on A
        // ("Activity3b", vec!["Activity2b"]),        // A has no dependencies
    ]);

    let exec = Executor::new(&names);

    exec.run(&dependency_graph);
}
