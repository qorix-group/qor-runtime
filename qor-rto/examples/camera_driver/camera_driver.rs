// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_rto::prelude::*;
pub struct CameraDriver {}

impl CameraDriver {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read_input(&mut self) -> RoutineResult {
        println!("read_input start");
        Ok(())
    }

    pub fn process(&mut self) -> RoutineResult {
        println!("process start");
        Ok(())
    }

    pub fn write_output(&mut self) -> RoutineResult {
        println!("write_output start");
        Ok(())
    }
}
