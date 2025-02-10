// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use crate::{base::*, rto_errors};
use iceoryx2::prelude::*;
use qor_core::prelude::*;

use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
};

/// Notify implementation for Single event
#[derive(Debug)]
pub struct SingleNotify {
    event: Arc<SingleEvent>,
}

impl Drop for SingleNotify {
    #[inline(always)]
    fn drop(&mut self) {
        self.event.unregister(self);
    }
}

impl SingleNotify {
    /// Create a new SingleNotify
    #[inline(always)]
    fn new(event: Arc<SingleEvent>) -> Self {
        Self { event }
    }
}

impl Notify for SingleNotify {
    #[inline(always)]
    fn notify(&self) {
        self.event.notify();
    }
}

/// The listener for a single event
#[derive(Debug)]
pub struct SingleListen {
    event: Arc<SingleEvent>,
}

impl Drop for SingleListen {
    #[inline(always)]
    fn drop(&mut self) {
        self.event.unsubscribe(self);
    }
}

impl Clone for SingleListen {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self::new(self.event.clone())
    }
}

impl SingleListen {
    #[inline(always)]
    fn new(event: Arc<SingleEvent>) -> Self {
        Self { event }
    }
}

impl Listen for SingleListen {
    #[inline(always)]
    fn check(&self) -> bool {
        self.event.check()
    }

    #[inline(always)]
    fn check_and_reset(&self) -> bool {
        self.event.check_and_reset()
    }

    #[inline(always)]
    fn wait(&self) -> RtoResult<()> {
        self.event.wait()
    }

    #[inline(always)]
    fn wait_timeout(&self, timeout: std::time::Duration) -> RtoResult<TimeoutStatus> {
        self.event.wait_timeout(timeout).map(|w| {
            if w.timed_out() {
                TimeoutStatus::TimedOut
            } else {
                TimeoutStatus::NotTimedOut
            }
        })
    }
}

/// An event for a single listener
#[derive(Debug)]
pub struct SingleEvent {
    /// number of notifiers (just for reference)
    notifiers: AtomicUsize,

    /// Flag for a set listener
    listener: AtomicBool,

    /// the flag to check
    state: (Mutex<bool>, Condvar),
}

impl From<SingleEvent> for Event<SingleEvent> {
    fn from(val: SingleEvent) -> Self {
        Event::new(val)
    }
}

impl SingleEvent {
    /// Create a new Single event
    #[inline(always)]
    pub fn new_adapter() -> Self {
        Self {
            notifiers: AtomicUsize::new(0),
            listener: AtomicBool::new(false),
            state: (Mutex::new(false), Condvar::new()),
        }
    }

    /// Create a new Single event as `Event<Single>`
    ///
    /// Shortcut for `Event::new(Single::new())`
    #[inline(always)]
    pub fn new() -> Event<Self> {
        Event::new(Self::new_adapter())
    }

    /// Unregister a notifier
    #[inline(always)]
    fn unregister(&self, _notifier: &SingleNotify) {
        self.notifiers.fetch_sub(1, Ordering::AcqRel);
    }

    /// Unsubscribe the listener
    #[inline(always)]
    fn unsubscribe(&self, _listener: &SingleListen) {
        self.listener.store(false, Ordering::Release);
    }

    /// Notify listeners (there is only one)
    #[inline(always)]
    fn notify(&self) {
        let (lock, cvar) = &self.state;
        if let Ok(mut flag) = lock.lock() {
            *flag = true;
            cvar.notify_all();
        }
    }

    /// Check the flag
    #[inline(always)] // inline because this will only be called from listener
    fn check(&self) -> bool {
        let (lock, _) = &self.state;
        if let Ok(flag) = lock.lock() {
            *flag
        } else {
            false
        }
    }

    /// Check the flag and reset it
    #[inline(always)] // inline because this will only be called from listener
    fn check_and_reset(&self) -> bool {
        let (flag, _) = &self.state;
        if let Ok(mut flag) = flag.lock() {
            let result = *flag;
            *flag = false;
            result
        } else {
            false
        }
    }

    /// Wait for the flag to be set
    #[inline(always)] // inline because this will only be called from listener
    fn wait(&self) -> RtoResult<()> {
        let (flag, cvar) = &self.state;

        // guard
        let mut guarded_flag = flag
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // wait
        guarded_flag = cvar
            .wait_while(guarded_flag, |state| !*state)
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // reset
        *guarded_flag = false;
        Ok(())
    }

    /// Wait for the flag to be set with a timeout
    #[inline(always)] // inline because this will only be called from listener
    fn wait_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> RtoResult<std::sync::WaitTimeoutResult> {
        let (flag, cvar) = &self.state;

        // guard
        let guarded_flag = flag
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // wait
        let mut result = cvar
            .wait_timeout_while(guarded_flag, timeout, |state: &mut bool| !*state)
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // reset
        *result.0 = false;
        Ok(result.1)
    }
}

impl EventAdapter for SingleEvent {
    type NotifierInner = SingleNotify;
    type ListenerInner = SingleListen;

    fn notifier(self: &Arc<Self>) -> RtoResult<Self::NotifierInner> {
        self.notifiers.fetch_add(1, Ordering::AcqRel);
        Ok(SingleNotify::new(self.clone()))
    }

    fn listener(self: &Arc<Self>) -> RtoResult<Self::ListenerInner> {
        // check if there is already a listener
        if self.listener.swap(true, Ordering::AcqRel) {
            return Err(Error::from_code(rto_errors::EVENT_SUBSCRIBERS_FULL));
        }

        // ok
        Ok(SingleListen::new(self.clone()))
    }
}

/// Notify implementation for Ipc event
#[derive(Debug)]
pub struct IpcNotify {
    notifier: iceoryx2::port::notifier::Notifier<ipc::Service>,
}

impl IpcNotify {
    /// Create a new IpcNotify
    #[inline(always)]
    fn new(event: Arc<IpcEvent>) -> Self {
        let notifier = event.builder.notifier_builder().create().unwrap();
        Self { notifier }
    }
}

impl Notify for IpcNotify {
    #[inline(always)]
    fn notify(&self) {
        self.notifier.notify().unwrap();
    }
}

/// The listener for a single event
#[derive(Debug)]
pub struct IpcListen {
    listener: iceoryx2::port::listener::Listener<ipc::Service>,
}

impl IpcListen {
    #[inline(always)]
    fn new(event: Arc<IpcEvent>) -> Self {
        let listener = event.builder.listener_builder().create().unwrap();
        Self { listener }
    }
}

impl Listen for IpcListen {
    #[inline(always)]
    fn check(&self) -> bool {
        self.check_and_reset()
    }

    #[inline(always)]
    fn check_and_reset(&self) -> bool {
        let res = self.listener.try_wait_one();
        match res {
            Ok(None) => false,
            Ok(_event_id) => true,
            Err(_) => false,
        }
    }

    #[inline(always)]
    fn wait(&self) -> RtoResult<()> {
        match self.listener.blocking_wait_one() {
            Ok(_opt_trigger_id) => Ok(()),
            Err(e) => Err(Error::new(0, e.to_string())),
        }
    }

    #[inline(always)]
    fn wait_timeout(&self, timeout: std::time::Duration) -> RtoResult<TimeoutStatus> {
        match self.listener.timed_wait_one(timeout) {
            Ok(_opt_trigger_id) => Ok(TimeoutStatus::NotTimedOut),
            Err(e) => Err(Error::new(0, e.to_string())),
        }
    }
}

/// An event for a single listener
#[derive(Debug)]
pub struct IpcEvent {
    builder: Arc<iceoryx2::service::port_factory::event::PortFactory<ipc::Service>>,
}

impl From<IpcEvent> for Event<IpcEvent> {
    fn from(val: IpcEvent) -> Self {
        Event::new(val)
    }
}

impl IpcEvent {
    /// Create a new IpcEvent
    #[inline(always)]
    fn new_event(name: &str) -> Self {
        let node = NodeBuilder::new().create::<ipc::Service>().unwrap();
        let port_factory = node
            .service_builder(&name.try_into().unwrap())
            .event()
            .open_or_create()
            .unwrap();

        let builder = Arc::new(port_factory);

        Self { builder }
    }

    /// Create a new IpcEvent as `Event<IpcEvent>`
    ///
    /// Shortcut for `Event::new(IpcEvent::new_event())`
    #[inline(always)]
    pub fn new(name: &str) -> Event<Self> {
        Event::new(Self::new_event(name))
    }
}

impl EventAdapter for IpcEvent {
    type NotifierInner = IpcNotify;
    type ListenerInner = IpcListen;

    fn notifier(self: &Arc<Self>) -> RtoResult<Self::NotifierInner> {
        Ok(IpcNotify::new(self.clone()))
    }

    fn listener(self: &Arc<Self>) -> RtoResult<Self::ListenerInner> {
        Ok(IpcListen::new(self.clone()))
    }
}
