// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

pub mod object_detection;

use object_detection::ObjectDetection;

use qor_rto::prelude::*;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn main() {
    let objdet = Arc::new(Mutex::new(ObjectDetection::new()));

    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    let start_obj_det = IpcEvent::new("start_event");

    // object detection program
    let program = Program::new().with_action(
        Loop::new().with_body(
            Sequence::new()
                .with_step(Sync::new(start_obj_det.listener().unwrap()))
                .with_step(Await::new_method_mut(
                    &objdet,
                    ObjectDetection::pre_processing,
                ))
                .with_step(
                    Concurrency::new()
                        .with_branch(Await::new_method_mut(&objdet, ObjectDetection::drive_q1))
                        .with_branch(Await::new_method_mut(&objdet, ObjectDetection::drive_q2))
                        .with_branch(Await::new_method_mut(&objdet, ObjectDetection::drive_q3)),
                )
                .with_step(Await::new_method_mut(
                    &objdet,
                    ObjectDetection::object_fusion,
                )),
        ),
    );

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    // here we wait for some time
    std::thread::sleep(Duration::from_secs(5));

    // Wait for the program to finish
    let _ = handle.join().unwrap();

    println!("object detection process simulation finished");

    // Engine shutdown
    engine.shutdown().unwrap();
}
