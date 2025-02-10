// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

// Runtime Orchestration (RTO) is the central runtime control element of the Qore stack

use qor_core::prelude::*;

use crate::base::*;
use crate::executor::{Engine, EngineInner};
use crate::task::Task;
use crate::task::TaskHandle;

mod nop;
pub use nop::Nop;

mod assign;
pub use assign::Assign;

mod sync;
pub use sync::{Sleep, Sync, Trigger};

mod routines;
pub use routines::{Activity, ActivityLifecycle, Await, Invoke};

mod exceptions;
pub use exceptions::{Throw, TryCatch};

mod branches;
pub use branches::IfThenElse;

mod loops;
pub use loops::{Break, For, ForRange, Loop};

mod flows;
pub use flows::{Computation, Concurrency, Sequence};

use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The required result type of a user-defined routine
pub type RoutineResult = Result<(), Error>;

/// The spawn function for creating new tasks
pub type SpawnFn = dyn Fn(Task<RoutineResult>) -> RtoResult<TaskHandle<RoutineResult>>;

/// The context of a program's execution.
// Note: We have the <'a> lifetime parameter as provision to be compatible with the
// std::future::Future Context<'a> implementation.
pub struct ExecutionContext<'a> {
    /// The executor of the program
    engine: Arc<EngineInner>,

    phamtom: std::marker::PhantomData<&'a ()>,
}

impl<'a> ExecutionContext<'a> {
    /// Creates a new execution context
    pub(crate) fn from_inner(engine: Arc<EngineInner>) -> ExecutionContext<'a> {
        ExecutionContext {
            engine,
            phamtom: std::marker::PhantomData,
        }
    }

    /// Creates a new execution context
    pub fn from_engine(engine: &Engine) -> ExecutionContext {
        ExecutionContext {
            engine: engine.inner().clone(),
            phamtom: std::marker::PhantomData,
        }
    }

    /// Spawns a new task
    #[inline(always)]
    pub fn spawn(&mut self, task: Task<RoutineResult>) -> RtoResult<TaskHandle<RoutineResult>> {
        self.engine.spawn(task)
    }

    /// Returns the current timepoint
    #[inline(always)]
    pub fn now(&self) -> Instant {
        self.engine.clock().now()
    }

    /// Sleeps for the until the given `Instant`
    #[inline(always)]
    pub fn sleep_until(&self, timepoint: Instant) {
        self.engine.clock().sleep_until(timepoint);
    }
}

/// The Action trait describes the interface to an executable element we call action.
pub trait Action: Debug + Send {
    /// Returns the current state of the action
    fn state(&self) -> ExecutionState;

    /// Initialize a new action with the given actionable.
    /// This is a valid operation when the action is `Uninitialized` or `Terminated`.
    fn init(&mut self) -> ExecutionState;

    /// Start the action.
    /// This is a valid operation after the action has been initialized `Ready` or stopped `Completed`.
    fn start(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState;

    /// Update the action with the given time delta.
    /// This is a valid operation after the action has been started and is `Running`.
    fn update(
        &mut self,
        delta: &Duration,
        ctx: &mut ExecutionContext<'_>,
    ) -> (Duration, UpdateResult);

    /// Stop and finalize the action.
    /// This is a valid operation when the action is `Running`.
    fn finalize(&mut self, ctx: &mut ExecutionContext<'_>) -> ExecutionState;

    /// Terminate the action.
    /// This is a valid operation when the action is `Ready`, `Completed` or in `Err` state.
    fn terminate(&mut self) -> ExecutionState;
}

#[cfg(test)]
mod test {
    use qor_core::Error;
    use sync::Trigger;

    use super::*;

    use crate::event;
    use crate::executor::EngineBuilder;
    use crate::expressions::*;
    use crate::variables::*;

    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Mutex;
    use std::thread;
    use std::{sync::Arc, time::Duration};

    fn ready_test_engine() -> Engine {
        EngineBuilder::new().with_threads(2).build().unwrap()
    }

    fn running_test_engine(tag: Tag) -> Engine {
        let engine = EngineBuilder::new()
            .with_tag(tag)
            .with_threads(2)
            .build()
            .unwrap();
        engine.start().unwrap();
        engine
    }

    //
    // General state machine test
    //
    fn test_state_machine(
        fab: impl Fn() -> Box<dyn Action>,
        expected_zero: (Duration, UpdateResult),
        expected_ten: (Duration, UpdateResult),
    ) {
        let engine = running_test_engine(Tag::new(*b"StmTest_"));
        let mut ctx = ExecutionContext::from_engine(&engine);

        // good: normal execution
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        // good: terminate without start
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        // good: finalize without update
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        // good: (start-update-finalize) twice
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&Duration::from_millis(10), &mut ctx),
                expected_ten
            );
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        // wrong: init after start
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.init(),
                ExecutionState::Err(ExecutionTransition::Init)
            );
        }

        // wrong: init after finalize
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(
                action.init(),
                ExecutionState::Err(ExecutionTransition::Init)
            );
        }

        // wrong: start without init
        {
            let mut action = fab();
            assert_eq!(
                action.start(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Start)
            );
        }

        // wrong: start after start
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.start(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Start)
            );
        }

        // wrong: update without init
        {
            let mut action = fab();
            assert_eq!(
                action.update(&Duration::ZERO, &mut ctx),
                (Duration::ZERO, UpdateResult::Err)
            );
        }

        // wrong: update without start
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(
                action.update(&Duration::ZERO, &mut ctx),
                (Duration::ZERO, UpdateResult::Err)
            );
        }

        // wrong: finalize without init
        {
            let mut action = fab();
            assert_eq!(
                action.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: finalize without start
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(
                action.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: double finalize
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(
                action.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: terminate without init
        {
            let mut action = fab();
            assert_eq!(
                action.terminate(),
                ExecutionState::Err(ExecutionTransition::Terminate)
            );
        }

        // wrong: terminate on terminate
        {
            let mut action = fab();
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(action.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
            assert_eq!(
                action.terminate(),
                ExecutionState::Err(ExecutionTransition::Terminate)
            );
        }

        engine.shutdown().unwrap();
    }

    //
    // Nop action test, general state machine test
    //
    #[test]
    fn test_nop_stm() {
        test_state_machine(
            || Nop::new(),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    //
    // Assign action tests
    //
    #[test]
    fn test_assign_stm() {
        test_state_machine(
            || {
                Assign::new(
                    Arc::new(VariableStorage::new(0)),
                    Arc::new(Constant::new(42)),
                )
            },
            (Duration::ZERO, UpdateResult::Ready),
            (Duration::ZERO, UpdateResult::Ready),
        );
    }

    //
    // Sync action tests
    //
    // TODO: FIXME: test sometimes runs over 60 s
    // #[test]
    // fn test_sleep_stm() {
    //    test_state_machine(
    //        || Sleep::new(Duration::from_millis(10)),
    //        (Duration::ZERO, UpdateResult::Busy),
    //        (Duration::from_millis(10), UpdateResult::Complete),
    //    );
    // }

    #[test]
    fn test_sleep() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);

        let period = Duration::from_millis(10);

        // sleep(0ms); # 0ms sleep action will update, but consume no time (and never be busy)
        let mut action = Sleep::new(Duration::ZERO);
        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);

        // sleep(10ms); # 10ms sleep action will update, and consume 10ms time (and never be busy)
        let mut action = Sleep::new(period);
        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::from_millis(10), UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);

        // sleep(100ms); # 100ms sleep action will update, and consume 10ms time each round (and be busy for 90ms)
        let mut action = Sleep::new(Duration::from_millis(100));
        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);

        // run 9 updates (for sleep)
        for _ in 0..9 {
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Busy)
            );
        }

        // now sleep must complete after final update
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_sync_stm() {
        let event = event::SingleEvent::new();
        let listener = event.listener().unwrap();

        test_state_machine(
            || Sync::new(listener.clone()),
            (Duration::ZERO, UpdateResult::Busy),
            (Duration::from_millis(10), UpdateResult::Busy),
        );
    }

    #[test]
    fn test_sync() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        let event = event::SingleEvent::new();
        let notifier = event.notifier().unwrap();
        let listener = event.listener().unwrap();

        // sync(event); # sync action will update, and consume 10ms time (and never be busy)
        let mut action = Sync::new(listener);
        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        notifier.notify();
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_trigger_stm() {
        let event = event::SingleEvent::new();
        let notifier = event.notifier().unwrap();

        test_state_machine(
            || Trigger::new(notifier.clone()),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_trigger() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        let event = event::SingleEvent::new();
        let notifier = event.notifier().unwrap();
        let listener = event.listener().unwrap();

        // trigger(event); # trigger action will update, and consume 0ms time (and never be busy)
        let mut action = Trigger::new(notifier);
        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert!(!listener.check_and_reset()); // no trigger in start
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert!(listener.check_and_reset()); // now trigger must be set
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    //
    // Routine action tests
    //

    struct TestActivity {
        /// Test behavior selection:
        /// - 0: no fails;
        /// - 1: fail on init;
        /// - 2: fail on start;
        /// - 3: fail on update,
        /// - 4: first busy then completed,
        /// - 5: fail on finalize,
        /// - 6: fail on terminate
        state: usize,
    }

    impl TestActivity {
        fn new(state: usize) -> Self {
            TestActivity { state }
        }
    }

    impl ActivityLifecycle for TestActivity {
        fn init(&mut self) -> Result<(), Error> {
            if self.state == 1 {
                Err(Error::FUNCTION_NOT_IMPLEMENTED)
            } else {
                Ok(())
            }
        }

        fn start(&mut self) -> Result<(), Error> {
            if self.state == 2 {
                Err(Error::FUNCTION_NOT_IMPLEMENTED)
            } else {
                Ok(())
            }
        }

        fn update(&mut self, elapsed: &Duration) -> (Duration, UpdateResult) {
            match self.state {
                3 => (Duration::ZERO, UpdateResult::Err),
                4 => {
                    self.state = 0;
                    (*elapsed, UpdateResult::Busy)
                }
                _ => (Duration::ZERO, UpdateResult::Complete),
            }
        }

        fn finalize(&mut self) -> Result<(), Error> {
            if self.state == 5 {
                Err(Error::FUNCTION_NOT_IMPLEMENTED)
            } else {
                Ok(())
            }
        }

        fn terminate(&mut self) -> Result<(), Error> {
            if self.state == 6 {
                Err(Error::FUNCTION_NOT_IMPLEMENTED)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn test_activity_stm() {
        test_state_machine(
            || Activity::new(TestActivity::new(0)),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_activity() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        {
            // activity(0); # no fails
            let mut action = Activity::new(TestActivity::new(0));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&period, &mut ctx),
                (Duration::ZERO, UpdateResult::Complete)
            );
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        {
            // activity(4); # busy-complete on update
            let mut action = Activity::new(TestActivity::new(4));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Busy)
            );
            assert_eq!(
                action.update(&period, &mut ctx),
                (Duration::ZERO, UpdateResult::Complete)
            );
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        {
            // activity(1); # fail on init
            let mut action = Activity::new(TestActivity::new(1));
            assert_eq!(
                action.init(),
                ExecutionState::Err(ExecutionTransition::Init)
            );
        }

        {
            // activity(2); # fail on start
            let mut action = Activity::new(TestActivity::new(2));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(
                action.start(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Start)
            );
        }

        {
            // activity(3); # fail on update
            let mut action = Activity::new(TestActivity::new(3));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&period, &mut ctx),
                (Duration::ZERO, UpdateResult::Err)
            );
        }

        {
            // activity(5); # fail on finalize
            let mut action = Activity::new(TestActivity::new(5));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&period, &mut ctx),
                (Duration::ZERO, UpdateResult::Complete)
            );
            assert_eq!(
                action.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        {
            // activity(6); # fail on terminate
            let mut action = Activity::new(TestActivity::new(6));
            assert_eq!(action.init(), ExecutionState::Ready);
            assert_eq!(action.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                action.update(&period, &mut ctx),
                (Duration::ZERO, UpdateResult::Complete)
            );
            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(
                action.terminate(),
                ExecutionState::Err(ExecutionTransition::Terminate)
            );
        }
    }

    #[test]
    fn test_invoke_stm() {
        test_state_machine(
            || Invoke::new(|_| (Duration::ZERO, UpdateResult::Complete)),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_invoke_with_closure() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        let remaining = Arc::new(Mutex::new(Duration::from_millis(20)));

        // update(|elapsed| { if elapsed < remaining { remaining -= elapsed; (elapsed, Busy) } else { (remaining, Complete) } })
        let mut action = Invoke::new({
            let remaining = Arc::clone(&remaining);
            move |elapsed| {
                let mut remaining = remaining.lock().unwrap();
                if *elapsed < *remaining {
                    *remaining -= *elapsed;
                    (*elapsed, UpdateResult::Busy)
                } else {
                    (*remaining, UpdateResult::Complete)
                }
            }
        });
        assert_eq!(action.init(), ExecutionState::Ready);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    struct TestStm {
        state: usize,
    }

    impl TestStm {
        fn new() -> Self {
            TestStm { state: 0 }
        }

        fn update(&mut self, _elapsed: &Duration) -> (Duration, UpdateResult) {
            match self.state {
                0 => {
                    self.state = 1;
                    (Duration::ZERO, UpdateResult::Busy)
                }

                1 => {
                    self.state = 2;
                    (Duration::ZERO, UpdateResult::Complete)
                }

                _ => (Duration::ZERO, UpdateResult::Err),
            }
        }
    }

    #[test]
    fn test_invoke_with_stm() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // build state machine
        let mut stm = TestStm::new();

        // update (stm)
        let mut action = Invoke::new(move |elapsed| stm.update(elapsed));
        assert_eq!(action.init(), ExecutionState::Ready);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    async fn async_test_function() -> Result<(), Error> {
        thread::sleep(Duration::from_millis(20));
        Ok(())
    }

    #[test]
    fn test_await_stm() {
        test_state_machine(
            || Await::new_fn(async_test_function),
            (Duration::ZERO, UpdateResult::Busy),
            (Duration::from_millis(10), UpdateResult::Busy),
        );
    }

    #[test]
    fn test_await() {
        let engine = running_test_engine(Tag::new(*b"Await___"));
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // await(|| { my_function })
        let mut action = Await::new_fn(async_test_function);
        assert_eq!(action.init(), ExecutionState::Ready);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);

        let now = std::time::Instant::now();
        let mut result = action.update(&period, &mut ctx);
        while result.1 == UpdateResult::Busy {
            thread::yield_now();
            result = action.update(&period, &mut ctx);
        }
        let _elapsed = std::time::Instant::now().duration_since(now);
        //assert!(elapsed < 25 * period / 10); // give 25% tolerance (as we will debug often and have some jitter on 10ms period)
        assert_eq!(result, (Duration::ZERO, UpdateResult::Complete));
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        assert_eq!(action.terminate(), ExecutionState::Terminated);

        engine.shutdown().unwrap();
    }

    //
    // Branch action tests
    //
    #[test]
    fn test_ifthenelse_stm() {
        test_state_machine(
            || {
                IfThenElse::new(Arc::new(VariableStorage::new(false)))
                    .with_then(Nop::new())
                    .with_else(Nop::new())
            },
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_ifthenelse_empty() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);
        let condition = Arc::new(VariableStorage::new(false));

        // if (condition) {}
        let mut action = IfThenElse::new(condition.clone());
        assert_eq!(action.init(), ExecutionState::Ready);

        // false condition
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        // true condition
        let _ = condition.assign(true);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_ifthenelse_then() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);
        let condition: Arc<VariableStorage<bool>> = Arc::new(VariableStorage::new(true));

        // if (condition) { sleep(2*period) }
        let mut action = IfThenElse::new(condition.clone()).with_then(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);

        // true condition -> Sleep in Then branch
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        // false condition -> Empty in Else branch
        let _ = condition.assign(false);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_ifthenelse_else() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);
        let condition: Arc<VariableStorage<bool>> = Arc::new(VariableStorage::new(true));

        // if (condition) {} else { sleep(2*period) }
        let mut action = IfThenElse::new(condition.clone()).with_else(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);

        // true condition -> empty then branch
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        // false condition -> Sleep in else branch
        let _ = condition.assign(false);

        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_ifthenelse_then_else() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);
        let condition: Arc<VariableStorage<bool>> = Arc::new(VariableStorage::new(true));

        // if (condition) { sleep(2*period) } else { sleep(3*period) }
        let mut action = IfThenElse::new(condition.clone())
            .with_then(Sleep::new(2 * period))
            .with_else(Sleep::new(3 * period));

        assert_eq!(action.init(), ExecutionState::Ready);

        // then branch condition
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );

        // switch condition must not change branch here
        let _ = condition.assign(false);

        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        // else branch (condition already switched)
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        // switch condition must not change branch here
        let _ = condition.assign(true);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_ifthenelse_with_predicate() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // state of the predicate
        let state = Arc::new(AtomicBool::new(true));

        // create predicate that checks state
        let state_clone = state.clone();
        let predicate = Arc::new(Predicate::new(move || -> Result<bool, EvalError> {
            Ok(state_clone.load(Ordering::Relaxed))
        }));

        // if (predicate(|| state)) { sleep(2*period) } else { sleep(3*period) }
        let mut action = IfThenElse::new(predicate)
            .with_then(Sleep::new(period))
            .with_else(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);

        // then branch condition
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);

        // update state
        state.store(false, Ordering::Relaxed);

        // else branch (condition already switched)
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    //
    // Exception action tests
    //
    // TODO: FIXME: runs sometimes more than 60 s
    // #[test]
    // fn test_try_catch_stm() {
    //     // try branch
    //     test_state_machine(
    //         || {
    //             TryCatch::new(ExecutionExceptionFilter::for_all())
    //                 .with_try(Nop::new())
    //                 .with_catch(Nop::new())
    //         },
    //         (Duration::ZERO, UpdateResult::Complete),
    //         (Duration::ZERO, UpdateResult::Complete),
    //     );
    //
    //     // catch branch
    //     test_state_machine(
    //         || {
    //             TryCatch::new(ExecutionExceptionFilter::for_all())
    //                 .with_try(Throw::new(ExecutionException::timeout()))
    //                 .with_catch(Nop::new())
    //         },
    //         (Duration::ZERO, UpdateResult::Complete),
    //         (Duration::ZERO, UpdateResult::Complete),
    //     );
    // }

    #[test]
    fn test_try_catch_empty_bodies_no_throw() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try {} catch (_) {}
        let mut action = TryCatch::new(ExecutionExceptionFilter::for_all());

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (Duration::ZERO, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_try_catch_empty_catch_no_throw() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try { sleep(10ms) } catch (_) {}
        let mut action =
            TryCatch::new(ExecutionExceptionFilter::for_all()).with_try(Sleep::new(period));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_try_catch_no_throw() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try { sleep(10ms) } catch (_) { sleep(20ms) }
        let mut action = TryCatch::catch_all()
            .with_try(Sleep::new(period))
            .with_catch(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_try_catch_throw_and_catch() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try { throw(Timeout) } catch (_) { sleep(20ms) }
        let mut action = TryCatch::catch_all()
            .with_try(Throw::timeout(Duration::ZERO))
            .with_catch(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_try_catch_throw_no_catch() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try { throw timeout } catch user(1) { sleep(2*period) }
        let mut action = TryCatch::new(ExecutionExceptionFilter::for_exception(
            ExecutionException::user(1),
        ))
        .with_try(Throw::new(ExecutionException::timeout()))
        .with_catch(Sleep::new(2 * period));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        // throw an immediate timeout exception that is not captured by catch that catches only user(1)
        assert_eq!(
            action.update(&period, &mut ctx),
            (
                Duration::ZERO,
                UpdateResult::Interruption(ExecutionInterruption::Exception(
                    ExecutionException::timeout()
                ))
            )
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_try_catch_rethrow() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        // try { throw timeout } catch (Timeout) { throw user(1) }
        let mut action = TryCatch::new(ExecutionExceptionFilter::for_timeout())
            .with_try(Throw::new(ExecutionException::timeout()))
            .with_catch(Throw::new(ExecutionException::user(1)));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);
        // throw an immediate timeout exception that is catched but catch does a rethrow of user(1)
        assert_eq!(
            action.update(&period, &mut ctx),
            (
                Duration::ZERO,
                UpdateResult::Interruption(ExecutionInterruption::Exception(
                    ExecutionException::user(1)
                ))
            )
        );
        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    //
    // Loop action tests
    //
    #[test]
    fn test_loop_stm() {
        // loop branch
        test_state_machine(
            || Loop::new().with_body(Break::new()),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_for_stm() {
        // for branch
        test_state_machine(
            || {
                For::new(
                    Arc::new(VariableStorage::new(0i64)),
                    Arc::new(Constant::new(0)),
                    Arc::new(Constant::new(10)),
                )
                .with_body(Nop::new())
            },
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    //
    // Flow action tests
    //
    #[test]
    fn test_sequence_stm() {
        test_state_machine(
            || Sequence::new().with_step(Nop::new()).with_step(Nop::new()),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_sequence() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);

        let my_variable = Arc::new(VariableStorage::new(0i64));
        let my_condition = Arc::new(VariableStorage::new(false));

        let period = Duration::from_millis(10);

        let mut seq = Sequence::new()
            .with_step(Sleep::new(Duration::from_millis(100)))
            .with_step(Nop::new())
            .with_step(
                For::new(
                    my_variable.clone(),
                    Arc::new(Constant::new(0)),
                    Arc::new(Constant::new(10)),
                )
                .with_body(Sleep::new(Duration::from_millis(10))),
            )
            .with_step(
                IfThenElse::new(Arc::new(Not::<bool, VariableStorage<bool>>::new(
                    my_condition.clone(),
                )))
                .with_then(Nop::new()),
            );

        assert_eq!(seq.init(), ExecutionState::Ready);
        assert_eq!(seq.start(&mut ctx), ExecutionState::Running);

        // run 10 updates (for first sleep(100ms))
        for _ in 0..10 {
            assert_eq!(seq.update(&period, &mut ctx), (period, UpdateResult::Busy));
        }

        // run 9 updates (for For 0..10 { sleep(10ms) })
        for _ in 0..9 {
            assert_eq!(seq.update(&period, &mut ctx), (period, UpdateResult::Busy));
        }

        // now sequence must end after final update
        assert_eq!(
            seq.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );
        assert_eq!(seq.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(seq.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_concurrency_stm() {
        // concurrency branch
        test_state_machine(
            || {
                Concurrency::new()
                    .with_branch(Nop::new())
                    .with_branch(Nop::new())
            },
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_concurrency() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        let mut action = Concurrency::new()
            .with_branch(Sleep::new(Duration::from_millis(100)))
            .with_branch(Sleep::new(Duration::from_millis(110)));

        assert_eq!(action.init(), ExecutionState::Ready);
        assert_eq!(action.start(&mut ctx), ExecutionState::Running);

        // run 10 updates (for sleep)
        for _ in 0..9 {
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Busy)
            );
        }

        // now concurrency must end branch 1 but be still busy with branch 2
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Busy)
        );

        // now concurrency must end
        assert_eq!(
            action.update(&period, &mut ctx),
            (period, UpdateResult::Complete)
        );

        assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
        assert_eq!(action.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_computation_stm() {
        test_state_machine(
            || {
                Computation::new()
                    .with_branch(Nop::new())
                    .with_branch(Nop::new())
            },
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );
    }

    #[test]
    fn test_computation() {
        let engine = ready_test_engine();
        let mut ctx = ExecutionContext::from_engine(&engine);
        let period = Duration::from_millis(10);

        {
            // computation { sleep(30ms), throw Timeout after 20ms }
            let mut action = Computation::new()
                .with_branch(Sleep::new(Duration::from_millis(30)))
                .with_branch(Throw::timeout(Duration::from_millis(20)));

            assert_eq!(action.init(), ExecutionState::Ready);

            assert_eq!(action.start(&mut ctx), ExecutionState::Running);

            // first update keeps computation busy
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Busy)
            );

            // second update throws Timeout
            assert_eq!(
                action.update(&period, &mut ctx),
                (
                    period,
                    UpdateResult::Interruption(ExecutionInterruption::Exception(
                        ExecutionException::timeout()
                    ))
                )
            );

            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }

        {
            // computation { sleep(20ms), throw Timeout after 30ms }
            let mut action = Computation::new()
                .with_branch(Sleep::new(Duration::from_millis(20)))
                .with_branch(Throw::timeout(Duration::from_millis(30)));

            assert_eq!(action.init(), ExecutionState::Ready);

            assert_eq!(action.start(&mut ctx), ExecutionState::Running);

            // first update keeps computation busy
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Busy)
            );

            // second update completes computation
            assert_eq!(
                action.update(&period, &mut ctx),
                (period, UpdateResult::Complete)
            );

            assert_eq!(action.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(action.terminate(), ExecutionState::Terminated);
        }
    }
}
