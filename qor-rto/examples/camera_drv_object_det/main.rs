// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

pub mod camera_driver;
pub mod object_detection;

use camera_driver::CameraDriver;
use object_detection::ObjectDetection;

use qor_rto::prelude::*;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

fn main() {
    let camdrv = Arc::new(Mutex::new(CameraDriver::new()));
    let objdet = Arc::new(Mutex::new(ObjectDetection::new()));

    // The engine is the central runtime executor
    let engine = Engine::default();
    engine.start().unwrap();

    // timer syncronization event
    let timer_event = SingleEvent::new();

    let start_obj_det = SingleEvent::new();

    // our simulation period
    const PERIOD: Duration = Duration::from_millis(500);
    let tim_prog = Program::new().with_action(
        Loop::new().with_body(
            Sequence::new()
                .with_step(Sleep::new(PERIOD))
                .with_step(Trigger::new(timer_event.notifier().unwrap())),
        ),
    );

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

    // Camera Driver Program
    let program1 = Program::new().with_action(
        Loop::new().with_body(
            Sequence::new()
                .with_step(Sync::new(timer_event.listener().unwrap()))
                .with_step(Await::new_method_mut(&camdrv, CameraDriver::read_input))
                .with_step(Await::new_method_mut(&camdrv, CameraDriver::process))
                .with_step(Await::new_method_mut(&camdrv, CameraDriver::write_output))
                .with_step(Trigger::new(start_obj_det.notifier().unwrap())),
        ),
    );

    // Spawn the program on the engine
    let handle = program.spawn(&engine).unwrap();

    let handle1 = program1.spawn(&engine).unwrap();

    let handle2 = tim_prog.spawn(&engine).unwrap();

    // here we wait for some time
    std::thread::sleep(Duration::from_secs(5));

    // Wait for the program to finish
    let _ = handle.join().unwrap();
    let _ = handle1.join().unwrap();
    let _ = handle2.join().unwrap();

    println!("Task chain simulation finished");

    // Engine shutdown
    engine.shutdown().unwrap();
}
