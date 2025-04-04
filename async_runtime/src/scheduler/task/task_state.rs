// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use foundation::prelude::IoxAtomicU32;

/// TODO: For now no use-case for IDLE, I keep it.
const TASK_STATE_IDLE: u32 = 0b0000_0001;

/// Currently executed
const TASK_STATE_RUNNING: u32 = 0b0000_0010;

/// Task returned Ready() from poll and it's done
const TASK_STATE_COMPLETED: u32 = 0b0000_0100;

/// Task has been canceled
const TASK_STATE_CANCELED: u32 = 0b0000_1000;

/// The join handle attached waker to a task
const TASK_JOIN_HANDLE_ATTACHED: u32 = 0b0001_0000;

/// The task was already notified and waiting for processing `somewhere`
const TASK_NOTIFIED: u32 = 0b0010_0000;

/// The task was already notified and waiting for processing into specific exclusive executor (ie. safety executor). TODO: Store executor id on upper bits of a state, for now this is always safety one
const TASK_NOTIFIED_EXCLUSIVE: u32 = 0b0100_0000;

///
/// Below transitions model small state machine in TaskState to assure correct behavior for callers
///

pub enum TransitionToIdle {
    Done,
    Notified,
    Aborted,
}

pub enum TransitionToRunning {
    Done,
    Aborted,
}

pub enum TransitionToCompleted {
    Done,
    HadConnectedJoinHandle,
}

pub enum TransitionToNotified {
    Done,
    AlreadyNotified,
    Running,
}

///
/// Depict state of task. All states are only set from within Task itself, except `TASK_STATE_CANCELED`. This one is set by connected JoinHandle is requested by user.
///
pub(crate) struct TaskState {
    s: IoxAtomicU32,
}

///
/// TaskState snapshot helper to not spread bitwise operations across real logic
///
#[derive(Clone, Copy)]
pub(crate) struct TaskStateSnapshot(u32);

impl TaskStateSnapshot {
    #[inline(always)]
    pub(crate) fn is_completed(&self) -> bool {
        (self.0 & TASK_STATE_COMPLETED) == TASK_STATE_COMPLETED
    }

    #[inline(always)]
    pub(crate) fn is_running(&self) -> bool {
        (self.0 & TASK_STATE_RUNNING) == TASK_STATE_RUNNING
    }

    #[inline(always)]
    pub(crate) fn is_idle(&self) -> bool {
        (self.0 & TASK_STATE_IDLE) == TASK_STATE_IDLE
    }

    #[inline(always)]
    pub(crate) fn is_canceled(&self) -> bool {
        (self.0 & TASK_STATE_CANCELED) == TASK_STATE_CANCELED
    }

    #[inline(always)]
    pub(crate) fn is_notified(&self) -> bool {
        (self.0 & (TASK_NOTIFIED | TASK_NOTIFIED_EXCLUSIVE)) != 0
    }

    #[inline(always)]
    pub(crate) fn has_join_handle(&self) -> bool {
        (self.0 & TASK_JOIN_HANDLE_ATTACHED) == TASK_JOIN_HANDLE_ATTACHED
    }

    #[inline(always)]
    pub(crate) fn set_join_handle(&mut self) {
        self.0 |= TASK_JOIN_HANDLE_ATTACHED;
    }

    #[inline(always)]
    pub(crate) fn set_running(&mut self) {
        let mask = TASK_STATE_RUNNING | TASK_STATE_IDLE;
        self.0 ^= mask;
    }

    #[inline(always)]
    pub(crate) fn set_idle(&mut self) {
        let mask: u32 = TASK_STATE_RUNNING | TASK_STATE_IDLE;
        self.0 ^= mask;
    }

    #[inline(always)]
    pub(crate) fn set_notified(&mut self) {
        self.0 |= TASK_NOTIFIED;
    }

    #[inline(always)]
    pub(crate) fn unset_notified(&mut self) {
        let mask = !(TASK_NOTIFIED | TASK_NOTIFIED_EXCLUSIVE);

        self.0 &= mask;
    }

    #[inline(always)]
    pub(crate) fn set_notified_exclusive(&mut self) {
        self.0 |= TASK_NOTIFIED_EXCLUSIVE;
    }
}

impl TaskState {
    pub(crate) fn new() -> Self {
        Self {
            s: IoxAtomicU32::new(TASK_STATE_IDLE),
        }
    }

    pub(crate) fn is_completed(&self) -> bool {
        let prev = TaskStateSnapshot(self.s.load(std::sync::atomic::Ordering::SeqCst));
        prev.is_completed()
    }

    ///
    /// Returns result of transition
    ///
    pub(crate) fn transition_to_completed(&self) -> TransitionToCompleted {
        let mask = TASK_STATE_COMPLETED;

        let prev = TaskStateSnapshot(self.s.swap(mask, std::sync::atomic::Ordering::SeqCst));

        // We shall call this only when in running state
        assert!(prev.is_running());
        assert!(!prev.is_completed());

        if prev.has_join_handle() && !prev.is_canceled() {
            // if we were canceled, we are done, we shall not wake anyone
            TransitionToCompleted::HadConnectedJoinHandle
        } else {
            TransitionToCompleted::Done
        }
    }

    ///
    /// Returns result of transition
    ///
    pub(crate) fn transition_to_running(&self) -> TransitionToRunning {
        self.fetch_update_with_return(|prev| {
            assert!(!prev.is_running()); // TODO add error handling
            assert!(prev.is_idle()); // TODO add error handling

            let mut next = prev;

            if prev.is_completed() {
                return (None, TransitionToRunning::Done);
            }

            if prev.is_canceled() {
                return (None, TransitionToRunning::Aborted);
            }

            next.unset_notified(); // we clear notification bit so we can be again notified from this moment as this weights on reschedule action
            next.set_running();

            (Some(next), TransitionToRunning::Done)
        })
    }

    ///
    /// Returns result of transition
    ///
    pub(crate) fn transition_to_idle(&self) -> TransitionToIdle {
        self.fetch_update_with_return(|prev| {
            assert!(prev.is_running()); // TODO add error handling
            assert!(!prev.is_idle()); // TODO add error handling

            let mut next = prev;

            if prev.is_canceled() {
                return (None, TransitionToIdle::Aborted);
            }

            let mut state = TransitionToIdle::Done;

            if prev.is_notified() {
                // If notified once we want to go into idle, we need to reschedule on our own since the notifier did not rescheduled it
                state = TransitionToIdle::Notified;
            }

            next.set_idle();

            (Some(next), state)
        })
    }

    ///
    /// Returns result of transition
    ///
    pub(crate) fn set_waker(&self) -> bool {
        self.fetch_update_with_return(|old: TaskStateSnapshot| {
            if old.is_completed() || old.is_canceled() {
                return (None, false);
            }

            let mut new = old;
            new.set_join_handle();
            (Some(new), true)
        })
    }

    ///
    /// Return true if task was notified before, otherwise false
    ///
    pub(crate) fn set_notified(&self) -> TransitionToNotified {
        self.fetch_update_with_return(|old: TaskStateSnapshot| {
            let mut res = TransitionToNotified::Done;

            if old.is_completed() || old.is_notified() || old.is_canceled() {
                return (None, TransitionToNotified::AlreadyNotified);
            }

            if old.is_running() {
                res = TransitionToNotified::Running;
            }

            let mut new = old;
            new.set_notified();
            (Some(new), res)
        })
    }

    ///
    /// Apply value from action and returns current value
    ///
    fn fetch_update<T: FnMut(TaskStateSnapshot) -> Option<TaskStateSnapshot>>(&self, mut f: T) -> TaskStateSnapshot {
        let mut val = self.s.load(std::sync::atomic::Ordering::Acquire);
        loop {
            match f(TaskStateSnapshot(val)) {
                Some(s) => {
                    let res = self
                        .s
                        .compare_exchange(val, s.0, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire);

                    match res {
                        Ok(_) => break s,
                        Err(actual) => val = actual,
                    }
                }
                None => break TaskStateSnapshot(val),
            }
        }
    }

    ///
    /// Apply value from action and returns value from action
    ///
    fn fetch_update_with_return<T: FnMut(TaskStateSnapshot) -> (Option<TaskStateSnapshot>, U), U>(&self, mut f: T) -> U {
        let mut val = self.s.load(std::sync::atomic::Ordering::Acquire);
        loop {
            let (state, ret) = f(TaskStateSnapshot(val));
            match state {
                Some(s) => {
                    let res = self
                        .s
                        .compare_exchange(val, s.0, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire);

                    match res {
                        Ok(_) => break ret,
                        Err(actual) => val = actual,
                    }
                }
                None => break ret,
            }
        }
    }
}
