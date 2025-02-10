// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
#![allow(unused)]

use std::sync::{Arc, Condvar, Mutex};
use std::task::{RawWaker, RawWakerVTable, Waker};

/// A no-operation waker that useful to acquire a poll result from a task
/// when the state of the task is known to be completed.
pub(crate) fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn noop_raw_waker() -> RawWaker {
    RawWaker::new(
        std::ptr::null(),
        &RawWakerVTable::new(|_| noop_raw_waker(), |_| {}, |_| {}, |_| {}),
    )
}

/// A custom waker that can be used to wake up a waiting engine thread
pub(crate) struct Signal {
    signal: Arc<(Mutex<bool>, Condvar)>,
}

impl Signal {
    /// Create a new waker
    pub fn new() -> Self {
        Self {
            signal: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    /// Wait for the waker to be woken up
    pub fn wait(&self) {
        let (lock, sig) = &*self.signal;
        if let Ok(mut flag) = lock.lock() {
            // wait until the flag is set to true
            while !*flag {
                flag = sig.wait(flag).unwrap();
                let _ = 42;
            }

            // and reset flag
            *flag = false;
        }
    }

    /// Wait for the waker to be woken up or until the timeout is reached
    pub fn wait_timeout(&self, timeout: std::time::Duration) -> bool {
        let (lock, sig) = &*self.signal;
        if let Ok(mut flag) = lock.lock() {
            // wait until the flag is set to true
            while !*flag {
                let result = sig.wait_timeout(flag, timeout).unwrap();
                flag = result.0;

                // check timout
                if result.1.timed_out() {
                    return false;
                }
            }

            // and reset flag
            *flag = false;
            true
        } else {
            false
        }
    }
}

pub(crate) fn signal_waker(arc: Arc<Signal>) -> Waker {
    let raw_waker = RawWaker::new(Arc::into_raw(arc) as *const (), &SIGNAL_VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

/// Clone of the RawMaker
unsafe fn signal_raw_waker_clone(data: *const ()) -> RawWaker {
    // generate our Arc from the raw pointer
    let arc = Arc::from_raw(data as *const Signal);

    // clone the Arc
    let arc_clone = arc.clone();
    std::mem::forget(arc); // prevent original Arc from being freed (we only want to clone it)

    // return the clone
    RawWaker::new(Arc::into_raw(arc_clone) as *const (), &SIGNAL_VTABLE)
}

/// Consuming wake of the RawMaker
unsafe fn signal_raw_waker_wake(data: *const ()) {
    // generate our Arc from the raw pointer (and we will not forget it, so it is consumed)
    let arc = Arc::from_raw(data as *const Signal);

    // notify the waiting thread
    {
        let (lock, sig) = &*arc.signal;
        if let Ok(mut latch) = lock.lock() {
            *latch = true;
            sig.notify_one();
        }
    }; // <- this semicolon is important. It is a statement that drops the lock guard before the arc is dropped

    // here the Arc is dropped and the memory is freed
}

/// Non-consuming wake of the RawMaker
unsafe fn signal_raw_waker_wake_by_ref(data: *const ()) {
    // generate our Arc from the raw pointer (and we will forget it, so it is _not_ consumed)
    let arc = Arc::from_raw(data as *const Signal);

    // notify the waiting thread
    {
        let (lock, sig) = &*arc.signal;
        if let Ok(mut latch) = lock.lock() {
            *latch = true;
            sig.notify_one();
        }
    }; // <- this semicolon is important. It is a statement that drops the lock guard before the arc is dropped
    std::mem::forget(arc); // prevent the Arc from being freed (we only want to wake it up)
}

/// Drop of the RawMaker
unsafe fn signal_raw_waker_drop(data: *const ()) {
    // generate our Arc from the raw pointer and immediately drop it
    let _ = Arc::from_raw(data as *const Signal);
}

/// The VTable for the RawWaker
static SIGNAL_VTABLE: RawWakerVTable = RawWakerVTable::new(
    signal_raw_waker_clone,
    signal_raw_waker_wake,
    signal_raw_waker_wake_by_ref,
    signal_raw_waker_drop,
);
