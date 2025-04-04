// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{thread, time::Duration};

use async_runtime::{core::types::box_future, runtime::runtime::AsyncRuntimeBuilder, scheduler::execution_engine::*};
use foundation::prelude::*;
use logging_tracing::{TraceScope, TracingLibraryBuilder};
use orchestration::{
    actions::{action::ActionTrait, concurrency::Concurrency},
    prelude::*,
    program::{Program, ProgramBuilder},
};
use std::{cell::RefCell, rc::Rc};

pub struct SomeAction {}

impl SomeAction {
    fn new() -> Box<SomeAction> {
        Box::new(Self {})
    }
}

impl ActionTrait for SomeAction {
    fn execute(&mut self) -> orchestration::actions::action::ActionFuture {
        box_future(async {
            info!("SomeAction was executed ;)");
            Ok(())
        })
    }

    fn name(&self) -> &'static str {
        "SomeAction"
    }

    fn dbg_fmt(&self, nest: usize, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let indent = " ".repeat(nest);
        writeln!(f, "{}|-{}", indent, self.name())
    }

    fn fill_runtime_info(&mut self, p: &mut orchestration::actions::action::ActionRuntimeInfoProvider) {}
}

fn test() -> ActionResult {
    info!("Testing this one!");
    Ok(())
}

async fn testasync() -> ActionResult {
    info!("Testing this one from testasync!");

    {
        async_runtime::spawn(async move {
            // println!("x {}", x2);
        });
    }

    Ok(())
}

pub struct X {}
impl X {
    fn test(&self) -> ActionResult {
        info!("Testing this from not owned ARC!");
        Ok(())
    }

    fn test_mut(&mut self) -> ActionResult {
        info!("Testing this from owned mutable!");
        Ok(())
    }
}

/// emulate some sleep as workaround until sleep is supported in runtime
fn busy_sleep() -> ActionResult {
    info!("Start sleeping");
    let mut ctr = 1000000;
    while ctr > 0 {
        ctr -= 1;
    }
    info!("End sleeping");
    Ok(())
}

async fn wait_ends() -> ActionResult {
    info!("Test_Event_1 triggered");
    Ok(())
}

async fn wait_ends2() -> ActionResult {
    info!("Test_Event_2 triggered");
    Ok(())
}

async fn wait_ends3() -> ActionResult {
    info!("Test_Event_3 triggered");
    Ok(())
}

// playground for testing
fn main() {
    let mut logger = TracingLibraryBuilder::new()
        .global_log_level(Level::INFO)
        .enable_tracing(TraceScope::AppScope)
        .enable_logging(true)
        .build();

    logger.init_log_trace();

    let mut runtime = AsyncRuntimeBuilder::new()
        .with_engine(ExecutionEngineBuilder::new().task_queue_size(256).workers(3))
        .build()
        .unwrap();

    {
        // Start the event handling thread.
        // This can be moved inside engine.
        Event::get_instance().lock().unwrap().create_polling_thread();
    }

    let _ = runtime.enter_engine(async {
        let x = RefCell::new(1);
        let event_name: &str = "Test_Event_1";
        let event_name2: &str = "Test_Event_2";
        let event_name3: &str = "Test_Event_3";
        let mut program = ProgramBuilder::new("basic")
            .with_body(
                Concurrency::new_with_id(NamedId::new_static("some_tracking_string"))
                    .with_branch(SomeAction::new())
                    .with_branch(
                        Sequence::new()
                            .with_step(SomeAction::new())
                            .with_step(Concurrency::new().with_branch(SomeAction::new()))
                            .with_step(SomeAction::new()),
                    )
                    .with_branch(SomeAction::new())
                    .with_branch(Invoke::from_async(testasync))
                    .with_branch(Sequence::new().with_step(Sync::new(event_name)).with_step(Invoke::from_async(wait_ends)))
                    .with_branch(Sequence::new().with_step(Invoke::from_fn(busy_sleep)).with_step(Trigger::new(event_name)))
                    .with_branch(
                        Sequence::new()
                            .with_step(Sync::new(event_name2))
                            .with_step(Invoke::from_async(wait_ends2)),
                    )
                    .with_branch(Sequence::new().with_step(Invoke::from_async(wait_ends3)))
                    .with_branch(
                        Sequence::new()
                            // .with_step(Invoke::from_fn(busy_sleep))
                            .with_step(Trigger::new(event_name3)),
                    )
                    .with_branch(
                        Sequence::new()
                            .with_step(Invoke::from_fn(busy_sleep))
                            .with_step(Trigger::new(event_name2)),
                    ),
            )
            .with_cycle_time(Duration::from_millis(1000))
            .with_shutdown_notification(Sync::new(event_name3))
            .build();

        println!("{:?}", program);

        let res = program.run().await;
        info!("Done {:?} {}", res, x.borrow());
    });

    // wait for some time to allow the engine finishes the last action
    thread::sleep(Duration::new(50, 0));
    println!("Exit.");
}
