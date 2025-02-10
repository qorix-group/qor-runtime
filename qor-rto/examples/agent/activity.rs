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

pub struct Activity {
    pub name: String,
    last: Instant,
}

impl Activity {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            last: Instant::now(),
        }
    }

    pub fn init(&mut self) -> RoutineResult {
        let now = Instant::now();
        println!(
            "{:09}: {}: init",
            (now - self.last).as_nanos() / 1000,
            self.name
        );
        self.last = now;
        Ok(())
    }

    pub fn step(&mut self) -> RoutineResult {
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

    pub fn terminate(&mut self) -> RoutineResult {
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
