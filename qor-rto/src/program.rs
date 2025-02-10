// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_core::prelude::*;

use crate::executor::{Engine, EngineInner};
use crate::{actions::*, base::*, rto_errors, Task, TaskHandle};

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::{sync::Arc, time::Duration};

pub mod builder;

/// A fiber is a lightweight cooperative thread running in a program.
///
/// It encapsulates the local state of the thread
pub struct Fiber {
    state: ExecutionState,

    action: Option<Box<dyn Action>>,
    uncaught: Option<ExecutionInterruption>,
}

impl Fiber {
    /// Create a new fiber with the given action
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            state: ExecutionState::new(),
            action: None,
            uncaught: None,
        }
    }

    /// Attach an action to the fiber
    pub fn with_action(mut self, action: Box<dyn Action>) -> Self {
        if self.state == ExecutionState::Uninitialized {
            self.action = Some(action);
        } else {
            panic!("Cannot attach action to fiber in state {}", self.state);
        }
        self
    }

    /// Get the state of the fiber#
    #[inline(always)]
    pub fn state(&self) -> ExecutionState {
        self.state
    }

    /// Get an uncaught interruption of the fiber
    #[inline(always)]
    pub fn uncaught(&self) -> Option<ExecutionInterruption> {
        self.uncaught
    }

    /// Initialize the fiber
    fn init(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Init) {
            ExecutionState::Ready => {
                if let Some(action) = &mut self.action {
                    self.state.transition_to(action.init())
                } else {
                    self.state.transition_to(ExecutionState::Ready)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Init)),
        }
    }

    /// Start the fiber
    fn start(&mut self, ctx: &mut ExecutionContext) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Start) {
            ExecutionState::Running => {
                if let Some(action) = &mut self.action {
                    self.state.transition_to(action.start(ctx))
                } else {
                    self.state.transition_to(ExecutionState::Running)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Start)),
        }
    }

    /// Update the fiber: Here we capture unhandled interruptions
    fn update(&mut self, delta: &Duration, ctx: &mut ExecutionContext) -> (Duration, UpdateResult) {
        if let Some(action) = &mut self.action {
            match self.state {
                ExecutionState::Running => {
                    // Handle action update
                    match action.update(delta, ctx) {
                        (elapsed, UpdateResult::Ready | UpdateResult::Complete) => {
                            // Ready and Complete are treated the same: We are done
                            (elapsed, UpdateResult::Complete)
                        }

                        (elapsed, UpdateResult::Busy) => {
                            // We need more time
                            (elapsed, UpdateResult::Busy)
                        }

                        (elapsed, UpdateResult::Interruption(inter)) => {
                            // Unhandled interruption. This turns into an error in the fiber
                            self.uncaught = Some(inter);

                            // and we finalize the action
                            action.finalize(ctx);

                            // but we enter error state as the interruption was uncaught
                            self.state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Update));

                            // and we are done
                            (elapsed, UpdateResult::Err)
                        }

                        (elapsed, UpdateResult::Err) => {
                            self.state
                                .transition_to(ExecutionState::Err(ExecutionTransition::Update));
                            (elapsed, UpdateResult::Err)
                        }
                    }
                }
                _ => (Duration::ZERO, UpdateResult::Err),
            }
        } else {
            // No action set: We are done
            (Duration::ZERO, UpdateResult::Complete)
        }
    }

    /// Finalize the fiber
    fn finalize(&mut self, ctx: &mut ExecutionContext) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Finalize) {
            ExecutionState::Completed => {
                if let Some(action) = &mut self.action {
                    self.state.transition_to(action.finalize(ctx))
                } else {
                    self.state.transition_to(ExecutionState::Completed)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Finalize)),
        }
    }

    /// Terminate the fiber
    fn terminate(&mut self) -> ExecutionState {
        match self.state.peek_state(ExecutionTransition::Terminate) {
            ExecutionState::Terminated => {
                if let Some(action) = &mut self.action {
                    self.state.transition_to(action.terminate())
                } else {
                    self.state.transition_to(ExecutionState::Terminated)
                }
            }
            _ => self
                .state
                .transition_to(ExecutionState::Err(ExecutionTransition::Terminate)),
        }
    }
}

impl Default for Fiber {
    fn default() -> Self {
        Self::new()
    }
}

/// A program is the central control element for running actions in fibers as tasks on an executor.
///
/// Programs run multiple fibers in parallel. The first fiber added is the main fiber. Termination of the main fiber terminates the program.
/// All other fibers are considered daemon fibers and are terminated when the main fiber terminates.
///
pub struct Program {
    inner: Arc<ProgramInner>,
}

impl Program {
    /// Create a new empty program
    pub fn new() -> Self {
        Self {
            inner: ProgramInner::new(),
        }
    }

    /// Add a concurrent action to the program
    ///
    /// This implicitly creates a fiber that wraps the action passed.
    ///
    /// This only works while the program is in the uninitialized state.
    pub fn with_action(self, action: Box<dyn Action>) -> Self {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add action to program in state {}", self.state());
        }
        self.inner
            .add_fiber(Fiber::new().with_action(action))
            .unwrap();
        self
    }

    /// Add a fiber to the program
    ///
    /// This only works while the program is in the uninitialized state.
    pub fn with_fiber(self, fiber: Fiber) -> Self {
        if self.state() != ExecutionState::Uninitialized {
            panic!("Cannot add fiber to program in state {}", self.state());
        }
        self.inner.add_fiber(fiber).unwrap();
        self
    }

    /// Get the state of the program
    #[inline(always)]
    pub fn state(&self) -> ExecutionState {
        self.inner.state()
    }

    /// Spawn the program on the given execution engine.
    #[inline(always)]
    pub fn spawn(self, executor: &Engine) -> RtoResult<TaskHandle<RtoResult<()>>> {
        self.inner.spawn(executor.inner())
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

/// The inner program structure
struct ProgramInner {
    /// The state of the program
    state: AtomicU32,

    /// The top-level action of the program
    fibers: Mutex<Vec<Fiber>>,
}

impl ProgramInner {
    /// The main fiber index
    const MAIN_FIBER: usize = 0;

    /// Create a new empty program
    #[inline(always)]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: AtomicU32::new(ExecutionState::Uninitialized.into()),
            fibers: Mutex::new(Vec::new()),
        })
    }

    /// Add a fiber to the program
    fn add_fiber(self: &Arc<Self>, fiber: Fiber) -> RtoResult<()> {
        let mut fibers = self
            .fibers
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;
        fibers.push(fiber);
        Ok(())
    }

    /// Get the state of the program
    #[inline(always)]
    fn state(self: &Arc<Self>) -> ExecutionState {
        self.state.load(Ordering::Relaxed).into()
    }

    /// Spawn the program on the given execution engine.
    fn spawn(
        self: &Arc<Self>,
        executor: &Arc<EngineInner>,
    ) -> RtoResult<TaskHandle<RtoResult<()>>> {
        if self.state() != ExecutionState::Uninitialized {
            return Err(Error::new(
                rto_errors::INVALID_STATE,
                format!("Cannot spawn program in state {}", self.state()),
            ));
        }

        // Initialize all fibers
        let mut fibers = self
            .fibers
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;
        for fiber in fibers.iter_mut() {
            if fiber.init() != ExecutionState::Ready {
                return Err(Error::new(
                    rto_errors::PROGRAM_FIBER_FAILED,
                    "Cannot initialize fiber.".to_string(),
                ));
            }
        }

        // we are initialized
        self.state
            .store(ExecutionState::Ready.into_u32(), Ordering::Relaxed);

        // create the future
        let future = ProgramInner::run(self.clone(), executor.clone());

        // spawn the task
        executor.spawn(Task::from_future(future))
    }

    /// Run the program.
    ///
    /// This is not a public function as a program runs on an engine. This is the
    /// asynchronous execution main routine of the program.
    async fn run(self: Arc<Self>, executor: Arc<EngineInner>) -> RtoResult<()> {
        debug_assert!(self.state() == ExecutionState::Ready);

        // all fibers remain locked while we run
        let mut fibers = self
            .fibers
            .lock()
            .map_err(|_| Error::from_code(qor_core::core_errors::LOCK_ERROR))?;

        // create the execution context
        let ctx = &mut ExecutionContext::from_inner(executor);

        // start all fibers. For now, we ignore the result of the start operation because we check on the fiber state while updating
        for fiber in fibers.iter_mut() {
            let _ = fiber.start(ctx);
        }

        // get the current time
        let period = Duration::from_millis(10);
        let mut remaining_logical = period;

        // start the `physical` time taken from the clock
        let mut cycle_clock = ctx.now();
        let mut until_clock = cycle_clock;

        // the main loop
        loop {
            // update end of the physical clock cycle.
            until_clock += period;

            // update for 1 cycle with with logical `period`.
            // We define that one update cycle logically takes `period` time and adjust this later with the real time
            for fiber in fibers.iter_mut() {
                // ignore completed and terminated fibers
                if fiber.state() == ExecutionState::Completed
                    || fiber.state() == ExecutionState::Terminated
                {
                    continue;
                }

                // update the fiber and elapsed time
                let (elapsed, result) = fiber.update(&period, ctx);
                debug_assert!(
                    elapsed <= period,
                    "Fiber update accumulated more elapsed time than was given"
                );

                // handle result
                match result {
                    UpdateResult::Ready | UpdateResult::Complete => {
                        // fiber is done
                        fiber.finalize(ctx);
                    }

                    UpdateResult::Busy => {
                        // nothing to do: fiber needs more time
                    }

                    UpdateResult::Interruption(interruption) => {
                        // fiber was interrupted: finalize
                        debug_assert!(false, "Unhandled interruption: {:?}", interruption); // a fiber should handle all interruptions itself. This is a bug
                        fiber.finalize(ctx);
                    }

                    UpdateResult::Err => {
                        // fiber failed: terminate
                        fiber.terminate();
                    }
                }
            }
            // here logically `period` time has passed

            // logically, this was one period: adjust the remaining logical time
            remaining_logical -= period;

            // check for completion: only main fibre matters
            if fibers[Self::MAIN_FIBER].state() == ExecutionState::Completed
                || fibers[Self::MAIN_FIBER].state() == ExecutionState::Terminated
            {
                // main fiber is done: loop done
                break;
            }

            // measure real time of the current cycle, reset cycle timepoint
            let now_clock = ctx.now(); // <-- time measurement linearization point
            let elapsed_clock = now_clock.saturating_duration_since(cycle_clock);
            cycle_clock = now_clock;

            // this is the logical time we have to catch up to in the next rounds
            while remaining_logical <= elapsed_clock {
                remaining_logical += period;
            }

            // sleep until the next cycle, clock handles overruns
            ctx.sleep_until(until_clock);
        }
        // here the main fiber is completed (or terminated)
        debug_assert!(
            fibers[Self::MAIN_FIBER].state() == ExecutionState::Completed
                || fibers[Self::MAIN_FIBER].state() == ExecutionState::Terminated
        );

        // finalize all remaining `Running` fibers
        for fiber in fibers.iter_mut().skip(1) {
            if fiber.state() == ExecutionState::Running {
                let _ = fiber.finalize(ctx);
                debug_assert!(fiber.state() == ExecutionState::Completed);
            }
        }

        // and terminate all fibers
        for fiber in fibers.iter_mut() {
            if fiber.state() != ExecutionState::Terminated {
                let _ = fiber.terminate();
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::executor::EngineBuilder;
    use std::time::{Duration, Instant};

    fn test_state_machine_fiber(
        fab: impl Fn() -> Fiber,
        expected_zero: (Duration, UpdateResult),
        expected_ten: (Duration, UpdateResult),
    ) {
        let engine = EngineBuilder::new().build().unwrap();
        let mut ctx = ExecutionContext::from_engine(&engine);

        // good: normal execution
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.terminate(), ExecutionState::Terminated);
        }

        // good: terminate without start
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.terminate(), ExecutionState::Terminated);
        }

        // good: finalize without update
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.terminate(), ExecutionState::Terminated);
        }

        // good: (start-update-finalize) twice
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                sut.update(&Duration::from_millis(10), &mut ctx),
                expected_ten
            );
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.terminate(), ExecutionState::Terminated);
        }

        // wrong: init after start
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.init(), ExecutionState::Err(ExecutionTransition::Init));
        }

        // wrong: init after finalize
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.init(), ExecutionState::Err(ExecutionTransition::Init));
        }

        // wrong: start without init
        {
            let mut sut = fab();
            assert_eq!(
                sut.start(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Start)
            );
        }

        // wrong: start after start
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(
                sut.start(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Start)
            );
        }

        // wrong: update without init
        {
            let mut sut = fab();
            assert_eq!(
                sut.update(&Duration::ZERO, &mut ctx),
                (Duration::ZERO, UpdateResult::Err)
            );
        }

        // wrong: update without start
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(
                sut.update(&Duration::ZERO, &mut ctx),
                (Duration::ZERO, UpdateResult::Err)
            );
        }

        // wrong: finalize without init
        {
            let mut sut = fab();
            assert_eq!(
                sut.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: finalize without start
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(
                sut.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: double finalize
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(
                sut.finalize(&mut ctx),
                ExecutionState::Err(ExecutionTransition::Finalize)
            );
        }

        // wrong: terminate without init
        {
            let mut sut = fab();
            assert_eq!(
                sut.terminate(),
                ExecutionState::Err(ExecutionTransition::Terminate)
            );
        }

        // wrong: terminate on terminate
        {
            let mut sut = fab();
            assert_eq!(sut.init(), ExecutionState::Ready);
            assert_eq!(sut.start(&mut ctx), ExecutionState::Running);
            assert_eq!(sut.update(&Duration::ZERO, &mut ctx), expected_zero);
            assert_eq!(sut.finalize(&mut ctx), ExecutionState::Completed);
            assert_eq!(sut.terminate(), ExecutionState::Terminated);
            assert_eq!(
                sut.terminate(),
                ExecutionState::Err(ExecutionTransition::Terminate)
            );
        }
    }

    #[test]
    fn test_fiber_stm() {
        // with a Nop, so simple
        test_state_machine_fiber(
            || Fiber::new().with_action(Nop::new()),
            (Duration::ZERO, UpdateResult::Complete),
            (Duration::ZERO, UpdateResult::Complete),
        );

        // With Sleep
        test_state_machine_fiber(
            || Fiber::new().with_action(Sleep::new(Duration::from_millis(10))),
            (Duration::ZERO, UpdateResult::Busy),
            (Duration::from_millis(10), UpdateResult::Complete),
        );

        let engine = EngineBuilder::new()
            .with_tag(Tag::new(*b"$Fiber__"))
            .with_threads(2)
            .with_tasks(8)
            .build()
            .unwrap();
        let ctx = &mut ExecutionContext::from_engine(&engine);

        // With Uncaught Interruption
        let mut fiber = Fiber::new().with_action(Break::new());
        assert_eq!(fiber.init(), ExecutionState::Ready);
        assert_eq!(fiber.start(ctx), ExecutionState::Running);
        assert_eq!(
            fiber.update(&Duration::ZERO, ctx),
            (Duration::ZERO, UpdateResult::Err)
        );
        assert_eq!(fiber.uncaught(), Some(ExecutionInterruption::Break));
        assert_eq!(
            fiber.finalize(ctx),
            ExecutionState::Err(ExecutionTransition::Finalize)
        );
        assert_eq!(fiber.terminate(), ExecutionState::Terminated);
    }

    #[test]
    fn test_program() {
        let executor = EngineBuilder::new()
            .with_tag(Tag::new(*b"Program_"))
            .with_threads(2)
            .build()
            .unwrap();
        executor.start().unwrap();

        let program = Program::new().with_action(Sleep::new(Duration::from_secs(5)));
        assert_eq!(program.state(), ExecutionState::Uninitialized);

        let handle = program.spawn(&executor).unwrap();
        assert_eq!(handle.is_finished(), false);

        let result = handle.join().unwrap();
        assert_eq!(result, Ok(()));

        executor.shutdown().unwrap();
    }

    struct MeasureState {
        now: Instant,
        turns: u32,
        samples: Vec<Duration>,
    }

    impl MeasureState {
        fn new(turns: u32) -> Self {
            Self {
                now: Instant::now(),
                turns,
                samples: Vec::new(),
            }
        }

        fn update(&mut self, delta: &Duration) -> (Duration, UpdateResult) {
            let now = Instant::now();
            let elapsed = now.saturating_duration_since(self.now);
            self.now = now;

            self.samples.push(elapsed);

            if self.turns > 0 {
                self.turns -= 1;
                (elapsed.min(*delta), UpdateResult::Busy)
            } else {
                (elapsed.min(*delta), UpdateResult::Complete)
            }
        }
    }

    impl Drop for MeasureState {
        fn drop(&mut self) {
            for (idx, sample) in self.samples.iter().enumerate() {
                //dbg!(idx, sample);
                print!("Sample {:04}: {:?}, ", idx, sample);
                if idx % 4 == 3 {
                    println!();
                }
            }

            let sum: Duration = self.samples.iter().sum();
            let avg = sum / self.samples.len() as u32;
            println!("Average: {:?}", avg);
        }
    }

    #[test]
    fn test_program_timebox() {
        let executor = EngineBuilder::new()
            .with_tag(Tag::new(*b"Program_"))
            .with_threads(2)
            .build()
            .unwrap();
        executor.start().unwrap();

        let mut measure_state = MeasureState::new(500);

        let program =
            Program::new().with_action(Invoke::new(move |delta| measure_state.update(delta)));
        assert_eq!(program.state(), ExecutionState::Uninitialized);

        let handle = program.spawn(&executor).unwrap();
        assert_eq!(handle.is_finished(), false);

        let result = handle.join().unwrap();
        assert_eq!(result, Ok(()));

        executor.shutdown().unwrap();
    }
}
