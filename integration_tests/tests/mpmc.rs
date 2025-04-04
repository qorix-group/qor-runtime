// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use async_runtime::{runtime::runtime::AsyncRuntimeBuilder, scheduler::execution_engine::ExecutionEngineBuilder, spawn};
    use std::{
        sync::{Arc, Mutex},
        thread,
        time::{Duration, Instant},
    };

    //
    // This is a test and the following should happen:
    // * create an AsyncRuntime with 1 worker and a queue size of 2.
    // * spawn a task on the runtime that spawns 5 tasks on the same worker again
    // * task0 and task1 should add to the workers local queue
    // * when task2 is added, there is no more room in the local queue and task0 should push to the
    //   global queue
    // * when task3 is added, task1 is pushed to global queue
    // * when task4 is added, task2 is pushed to global queue
    // * task3 and task4 are executed
    // * worker is beginning to search for work: task 0 and 1 are taken from global queue to the
    //   workers local queue
    // * task0 and task1 are executed
    // * worker is again searching for work: task2 is taken from global to local queue
    // * task2 is executed
    // * worker is going to sleep
    //
    // The test checks if every task is executed (at least) once. It also checks the expected
    // execution order: 3,4,0,1,2

    /// Until we don't have shutdown routine no miri
    #[cfg(not(miri))]
    #[test]
    fn mpmc_integration_test() {
        let mut runtime = AsyncRuntimeBuilder::new()
            .with_engine(ExecutionEngineBuilder::new().task_queue_size(2).workers(1))
            .build()
            .unwrap();

        let var0 = Arc::new(Mutex::new(1u32));
        let var1 = Arc::new(Mutex::new(0u32));
        let var2 = Arc::new(Mutex::new(0u32));
        let var3 = Arc::new(Mutex::new(0u32));
        let var4 = Arc::new(Mutex::new(0u32));
        let var0c = var0.clone();
        let var1c = var1.clone();
        let var2c = var2.clone();
        let var3c = var3.clone();
        let var4c = var4.clone();

        let i0 = Arc::new(Mutex::new(Instant::now()));
        let i1 = Arc::new(Mutex::new(Instant::now()));
        let i2 = Arc::new(Mutex::new(Instant::now()));
        let i3 = Arc::new(Mutex::new(Instant::now()));
        let i4 = Arc::new(Mutex::new(Instant::now()));

        let i0c = i0.clone();
        let i1c = i1.clone();
        let i2c = i2.clone();
        let i3c = i3.clone();
        let i4c = i4.clone();

        let _ = runtime.enter_engine(async {
            let handle0 = spawn(async {
                let mut var0c = var0c;
                *var0c.lock().unwrap() = 0;

                let mut i0c = i0c;
                *i0c.lock().unwrap() = Instant::now();
                0
            });

            let handle1 = spawn(async {
                let mut var1c = var1c;
                *var1c.lock().unwrap() = 1;

                let mut i1c = i1c;
                *i1c.lock().unwrap() = Instant::now();
                1
            });

            let handle2 = spawn(async {
                let mut var2c = var2c;
                *var2c.lock().unwrap() = 2;

                let mut i2c = i2c;
                *i2c.lock().unwrap() = Instant::now();
                2
            });

            let handle3 = spawn(async {
                let mut var3c = var3c;
                *var3c.lock().unwrap() = 3;

                let mut i3c = i3c;
                *i3c.lock().unwrap() = Instant::now();
                3
            });

            let handle4 = spawn(async {
                let mut var4c = var4c;
                *var4c.lock().unwrap() = 4;

                let mut i4c = i4c;
                *i4c.lock().unwrap() = Instant::now();
                4
            });
        });

        thread::sleep(Duration::new(2, 0));
        let v0 = var0.lock().unwrap();
        let v1 = var1.lock().unwrap();
        let v2 = var2.lock().unwrap();
        let v3 = var3.lock().unwrap();
        let v4 = var4.lock().unwrap();
        // Was every task executed?
        assert_eq!(*v0, 0);
        assert_eq!(*v1, 1);
        assert_eq!(*v2, 2);
        assert_eq!(*v3, 3);
        assert_eq!(*v4, 4);

        let i0 = i0.lock().unwrap();
        let i1 = i1.lock().unwrap();
        let i2 = i2.lock().unwrap();
        let i3 = i3.lock().unwrap();
        let i4 = i4.lock().unwrap();
        // Check execution order: 3,4,0,1,2
        assert!(*i3 < *i4);
        assert!(*i4 < *i0);
        assert!(*i0 < *i1);
        assert!(*i1 < *i2);
    }
}
