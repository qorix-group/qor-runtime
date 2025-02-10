// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0

use qor_core::prelude::*;

/// Memory module result type
pub type RtoResult<T> = std::result::Result<T, Error>;

pub type StateType = i64;
pub type ConditionType = bool;

use std::fmt::{Debug, Display};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

//
// Expressions
//

/// Result of the evaluation of an RValue
pub enum EvalError {
    /// Division by zero error
    DivisionByZero,

    /// Arithmetic underflow error
    Underflow,

    /// Arithmetic overflow error
    Overflow,
}

/// An RValue delivers a value of type T upon request.
/// The type T must comply to the Qore Type System.
pub trait RValue<T: TypeTag>: Debug + Send + Sync {
    /// Evaluate the expression
    fn eval(&self) -> Result<T, EvalError>;
}

/// An LValue is a data element that can be assigned a value.
/// The type T must comply to the Qore Type System.
pub trait LValue<T: TypeTag + Debug + Send + Sync>: RValue<T> {
    /// Assign a value to the expression
    /// Hint: This assumes the assign does not require a mutable self
    /// (which would cause problems when used with Arc).
    /// We assume lock free atomic operations behind this.
    fn assign(&self, value: T) -> Result<T, EvalError>;
}

/// A Variable is a trait that represents a scalar variable storage with instant updates
/// The type T must comply to the Qore Type System.
pub trait Variable<T: TypeTag + Debug + Send + Sync>: LValue<T> + RValue<T> {}

/// A State is a trait that represents a state storage with explicit timepoints for updates
/// The type T must comply to the Qore Type System.
pub trait State<T: TypeTag + Debug + Send + Sync>: Variable<T> {
    fn update(&mut self, dt: &Duration) -> Result<T, EvalError>;
}

//
// Events
//

/// The `Notify` trait is implemented by Notifiers
pub trait Notify: Debug + Send {
    /// Execute a notification to the connected `Event`
    ///
    fn notify(&self);
}

/// A `Notifier` is a handle to an event to trigger notifications
///
// We do wrap notify into a generic `Notifier` so inference can work from actual type to `EventAdapter::Notifier`
pub struct Notifier<Adapter: EventAdapter> {
    inner: Arc<Adapter::NotifierInner>,
}

unsafe impl<Adapter: EventAdapter> Send for Notifier<Adapter> {}
unsafe impl<Adapter: EventAdapter> Sync for Notifier<Adapter> {}

impl<Adapter: EventAdapter> Debug for Notifier<Adapter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Notifier<{:?}>", self.inner)
    }
}

impl<Adapter: EventAdapter> Clone for Notifier<Adapter> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Adapter: EventAdapter> Notifier<Adapter> {
    /// Create a new notifier
    #[inline(always)]
    fn new(notifier: Adapter::NotifierInner) -> Self {
        Self {
            inner: Arc::new(notifier),
        }
    }
}

impl<Adapter: EventAdapter> Notify for Notifier<Adapter> {
    #[inline(always)]
    fn notify(&self) {
        self.inner.notify();
    }
}

pub enum TimeoutStatus {
    TimedOut,
    NotTimedOut,
}

/// Listen trait for event listeners.
///
pub trait Listen: Debug + Send {
    /// Check if the subscribed event has received a notification since the last check.
    ///
    /// Just checks on the event but leaves the status untouched.
    fn check(&self) -> bool;

    /// Check if the subscribed event has received a notification since the last check and reset it.
    ///
    /// Reset the event notification state after checking.
    fn check_and_reset(&self) -> bool;

    /// Wait for the event to trigger.
    ///
    /// This locks the current thread until the event triggers. The event is reset when this function returns.
    /// If the event has already triggered (check would return `true`), the function returns immediately.
    fn wait(&self) -> RtoResult<()>;

    /// Wait for the event to trigger with a timeout.
    ///
    /// This locks the current thread until the event triggers or the timeout is reached. The event is reset when this function returns.
    /// If the event has already triggered (check would return `true`), the function returns immediately.
    fn wait_timeout(&self, timeout: Duration) -> RtoResult<TimeoutStatus>;
}

/// A `Listener` is a handle to an event to receive notifications
///
// We do wrap notify into a generic `Listener` so inference can work from actual type to `EventAdapter::Listener`
pub struct Listener<Adapter: EventAdapter> {
    inner: Arc<Adapter::ListenerInner>,
}

unsafe impl<Adapter: EventAdapter> Send for Listener<Adapter> {}
unsafe impl<Adapter: EventAdapter> Sync for Listener<Adapter> {}

impl<Adapter: EventAdapter> Debug for Listener<Adapter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Listener<{:?}>", self.inner)
    }
}

impl<Adapter: EventAdapter> Clone for Listener<Adapter> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Adapter: EventAdapter> Listener<Adapter> {
    /// Create a new listener
    #[inline(always)]
    fn new(listener: Adapter::ListenerInner) -> Self {
        Self {
            inner: Arc::new(listener),
        }
    }
}

impl<Adapter: EventAdapter> Listen for Listener<Adapter> {
    #[inline(always)]
    fn check(&self) -> bool {
        self.inner.check()
    }

    #[inline(always)]
    fn check_and_reset(&self) -> bool {
        self.inner.check_and_reset()
    }

    #[inline(always)]
    fn wait(&self) -> RtoResult<()> {
        self.inner.wait()
    }

    #[inline(always)]
    fn wait_timeout(&self, timeout: Duration) -> RtoResult<TimeoutStatus> {
        self.inner.wait_timeout(timeout)
    }
}

/// An `EventAdapter` links an event implementation with the orchestration `Event` abstraction.
pub trait EventAdapter: Sized + Debug + Send + Sync {
    /// The notifier type for the event
    type NotifierInner: Notify;

    /// The listener type for the event
    type ListenerInner: Listen;

    /// Create a notifier and register it to the event
    fn notifier(self: &Arc<Self>) -> RtoResult<Self::NotifierInner>;

    /// Create a listener and subscibe it to the event
    /// The caller can utilize the listener to check on the event for notifications
    fn listener(self: &Arc<Self>) -> RtoResult<Self::ListenerInner>;
}

/// An `Event` signals the occurrence of the change of state.
///
/// Events are triggered by notifiers and processed by listeners.
/// Both notifiers and listeners can be requested from the event.
/// Dropping a notifier will unregister it from the event.
/// Dropping a listener will unsubscribe it from the event.
///
pub struct Event<Adapter: EventAdapter> {
    inner: Arc<Adapter>,
}

unsafe impl<Adapter: EventAdapter> Send for Event<Adapter> {}
unsafe impl<Adapter: EventAdapter> Sync for Event<Adapter> {}

impl<Adapter: EventAdapter> Debug for Event<Adapter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Event<{:?}>", self.inner)
    }
}

impl<Adapter: EventAdapter> Clone for Event<Adapter> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<Adapter: EventAdapter> Event<Adapter> {
    /// Create a new event
    #[inline(always)]
    pub fn new(adapter: Adapter) -> Self {
        Self {
            inner: Arc::new(adapter),
        }
    }

    /// Open a notifier to the event
    #[inline(always)]
    pub fn notifier(&self) -> RtoResult<Notifier<Adapter>> {
        Ok(Notifier::new(self.inner.notifier()?))
    }

    /// Subscribe a listener to the event
    #[inline(always)]
    pub fn listener(&self) -> RtoResult<Listener<Adapter>> {
        Ok(Listener::new(self.inner.listener()?))
    }
}

//
// Time
//

/// A clock provides the current timepoint and can hold the execution sleep for a given duration.
///
/// We use clocks to abstract access to time. This way we can decouple the execution time from the real time.
///
/// In the runtime orchestration, a clock is always steady and monotonic.
pub trait Clock {
    /// Get the frequency of the clock, in Hz.
    fn frequency(&self) -> u64;

    /// Get the period of the clock, in nanoseconds.
    ///
    /// This is  by default the inverse of the frequency.
    fn period(&self) -> Duration {
        Duration::from_nanos(1_000_000_000 / self.frequency())
    }

    /// Get the minimum timepoint of the clock. This is the timepoint when the clock started.
    ///
    /// Do not assume this to be zero, as the clock may have started at any absolute point in time.
    fn min(&self) -> Instant;

    /// Get the current timepoint of the clock.
    ///
    fn now(&self) -> Instant;

    /// Get the age of the clock.
    ///
    /// The age is the duration since the clock started.
    fn age(&self) -> Duration {
        self.now().duration_since(self.min())
    }

    /// Sleep until the given timepoint.
    ///
    /// It is implementation specific for the clock what happens here.
    /// The clock may suspend the thread, or just busy wait until the given timepoint.
    /// If the timepoint is in the past, the function returns immediately.
    fn sleep_until(&self, time: Instant);

    /// Sleep for the given duration.
    ///
    /// It is implementation specific for the clock what happens here.
    /// The clock may suspend the thread, or just busy wait for the given duration.
    /// In simulation clocks, this may even do nothing and just advance the time.
    fn sleep(&self, duration: Duration) {
        self.sleep_until(self.now() + duration);
    }
}

//TODO: The AccurateClock need to be moved to qor_os.
/// The AccurateClock is a clock that uses a monotonous system counter with high resolution
/// to determine the current timepoint.
#[derive(Debug, Clone, Copy)]
pub struct AccurateClock {
    /// The start time of the clock
    start: Instant,

    /// The frequency of the performance counter
    frequency: u64,

    /// The sleep block time (this is the resolution to expect for a sleep() operation)
    block_duration: Duration,
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "C" {
    fn QueryPerformanceFrequency(lpFrequency: *mut i64) -> i32;
}

#[cfg(target_os = "windows")]
fn get_performance_counter_frequency() -> u64 {
    unsafe {
        let mut frequency: i64 = 0;
        let _ = QueryPerformanceFrequency(&mut frequency);
        frequency as u64
    }
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct Timespec {
    tv_sec: i64,  // seconds
    tv_nsec: i64, // nanoseconds
}

#[cfg(target_os = "linux")]
#[link(name = "c")]
extern "C" {
    fn clock_getres(clk_id: i32, res: *mut Timespec) -> i32;
}

#[cfg(target_os = "linux")]
fn get_high_resolution_clock_frequency() -> u64 {
    1_000_000_000
        / unsafe {
            let mut res = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            let _ = clock_getres(0, &mut res);
            res.tv_nsec
        } as u64
}

impl AccurateClock {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            frequency: get_high_resolution_clock_frequency(),
            block_duration: Self::measure_sleep(),
        }
    }

    fn measure_sleep() -> Duration {
        const ROUNDS: usize = 100;
        let mut sum = Duration::ZERO;

        let mut start = Instant::now();
        for _ in 0..ROUNDS {
            std::thread::sleep(Duration::from_millis(1));
            let now = Instant::now();
            sum += now.duration_since(start);
            start = now;
        }
        debug_assert!(sum.as_secs() < 100); // this is to guarantee that the subsec_nanos are valid in Duration average

        sum / ROUNDS as u32
    }
}

impl Default for AccurateClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for AccurateClock {
    fn frequency(&self) -> u64 {
        self.frequency
    }

    fn min(&self) -> Instant {
        self.start
    }

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, timepoint: Instant) {
        // snap the start time
        let start = Instant::now();

        // immediate return if the timepoint is in the past
        if start >= timepoint {
            return;
        }

        // locking portion of sleep
        if let Some(endpoint) = timepoint.checked_sub(self.block_duration) {
            // the time we have to bridge
            let duration = endpoint.duration_since(start);

            // calculate the number of blocks to sleep
            let blocks = (duration.as_nanos() / self.block_duration.subsec_nanos() as u128) as u32;

            // the slow wait with sleep (n-1 blocks)
            if blocks > 1 {
                std::thread::sleep(self.block_duration * (blocks - 1));
            }

            // the fast wait with yield (last block)
            while Instant::now() < endpoint {
                std::thread::yield_now();
            }
        }

        // the rest we do busy waiting as this is more accurate.
        while Instant::now() < timepoint {
            std::hint::spin_loop();
        }
    }
}

//
// Execution Elements
//

/// The ExecutionTransition enum represents the possible transitions between execution states.
#[derive(Clone, Copy, PartialEq)]
pub enum ExecutionTransition {
    /// Init: Transition from ExectuionState::Uninitialized or ExecutionState::Terminated to ExecutionState::Ready
    Init,

    /// Start: Transition from ExecutionState::Ready or ExecutionState::Completed to ExecutionState::Running
    Start,

    /// Update: Transition from ExecutionState::Running to ExecutionState::Running
    Update,

    /// Finalize: Transition from ExecutionState::Running to ExecutionState::Completed
    Finalize,

    /// Terminate: Transition from ExecutionState::Ready or ExecutionState::Completed to ExecutionState::Terminated
    Terminate,

    /// Error: Transition from an error state
    Error,
}

impl Debug for ExecutionTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionTransition::Init => write!(f, ">Init"),
            ExecutionTransition::Start => write!(f, ">Start"),
            ExecutionTransition::Update => write!(f, ">Update"),
            ExecutionTransition::Finalize => write!(f, ">Finalize"),
            ExecutionTransition::Terminate => write!(f, ">Terminate"),
            ExecutionTransition::Error => write!(f, ">Error"),
        }
    }
}

/// The ExecutionState enum represents the runtime state of executing elements.
/// Executing elements are `Actions`, `Programs`, `Tasks` and `Executors`.
#[derive(Clone, Copy, PartialEq)]
#[repr(isize)]
pub enum ExecutionState {
    /// The execution element has not been initialized.
    Uninitialized = 0,

    /// The execution element is ready to start.
    Ready = 1,

    /// The execution element is running.
    Running = 2,

    /// The execution element has completed successfully.
    Completed = 3,

    /// The execution element has been terminated.
    Terminated = 4,

    /// The execution element state machine is in an invalid state for the operation.
    Err(ExecutionTransition),
}

impl From<u32> for ExecutionState {
    fn from(state: u32) -> Self {
        ExecutionState::from_u32(state)
    }
}

impl From<ExecutionState> for u32 {
    fn from(val: ExecutionState) -> Self {
        val.into_u32()
    }
}

impl Debug for ExecutionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionState::Uninitialized => write!(f, "Uninitialized"),
            ExecutionState::Ready => write!(f, "Ready"),
            ExecutionState::Running => write!(f, "Running"),
            ExecutionState::Completed => write!(f, "Completed"),
            ExecutionState::Terminated => write!(f, "Terminated"),
            ExecutionState::Err(transition) => write!(f, "Error({:?})", transition),
        }
    }
}

impl Display for ExecutionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ExecutionState {
    /// Create a new uninitialized execution state
    pub const fn new() -> Self {
        Self::Uninitialized
    }

    /// Unwrap the execution state, panicking on error and ignoring the value otherwise
    /// This is a convenience function for mimicking the Result<T, E>::unwrap() and to avoid compiler warnings
    /// when functions return ExecutionState are used without checking the result.
    pub fn unwrap(&self) {
        if let ExecutionState::Err(transition) = self {
            panic!("Execution State error in transition: {:?}", transition);
        }
    }

    /// convert a u32 value to an execution state
    /// This is to use the state together with atomic operations
    pub const fn from_u32(state: u32) -> Self {
        match state {
            0 => ExecutionState::Uninitialized,
            1 => ExecutionState::Ready,
            2 => ExecutionState::Running,
            3 => ExecutionState::Completed,
            4 => ExecutionState::Terminated,
            _ => ExecutionState::Err(ExecutionTransition::Error),
        }
    }

    /// convert the execution state to a u32 value
    /// This is to use the state together with atomic operations
    pub const fn into_u32(self) -> u32 {
        match self {
            ExecutionState::Uninitialized => 0,
            ExecutionState::Ready => 1,
            ExecutionState::Running => 2,
            ExecutionState::Completed => 3,
            ExecutionState::Terminated => 4,
            ExecutionState::Err(_) => u32::MAX,
        }
    }

    /// Peek the successing state of the execution state machine after performing the given transition
    pub const fn peek_state(&self, transition: ExecutionTransition) -> ExecutionState {
        match transition {
            // Init is allowed from Uninitialized or Terminated to Ready
            ExecutionTransition::Init => match self {
                ExecutionState::Uninitialized | ExecutionState::Terminated => ExecutionState::Ready,
                _ => ExecutionState::Err(ExecutionTransition::Init),
            },

            // Start is allowed from Ready or Completed to Running
            ExecutionTransition::Start => match self {
                ExecutionState::Ready | ExecutionState::Completed => ExecutionState::Running,
                _ => ExecutionState::Err(ExecutionTransition::Start),
            },

            // Update is allowed from Running to Running
            ExecutionTransition::Update => match self {
                ExecutionState::Running => ExecutionState::Running,
                _ => ExecutionState::Err(ExecutionTransition::Update),
            },

            // Finalize is allowed from Running to Completed
            ExecutionTransition::Finalize => match self {
                ExecutionState::Running => ExecutionState::Completed,
                _ => ExecutionState::Err(ExecutionTransition::Finalize),
            },

            // Terminate is allowed from Ready or Completed to Terminated
            ExecutionTransition::Terminate => match self {
                ExecutionState::Ready | ExecutionState::Completed | ExecutionState::Err(_) => {
                    ExecutionState::Terminated
                }
                _ => ExecutionState::Err(ExecutionTransition::Terminate),
            },

            // Error is allowed from any state, and always results in an error state
            ExecutionTransition::Error => ExecutionState::Err(ExecutionTransition::Error),
        }
    }

    /// Transition into the given state
    pub fn transition_to(&mut self, state: ExecutionState) -> ExecutionState {
        *self = state;
        *self
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Id type for execution exceptions.
/// Id ranges from 0 to 63
pub type ExecutionExceptionId = u8;

/// Code type for execution exceptions
/// Code ranges from 0 to 2^63-1. `Code := 1 << Id`
pub type ExecutionExceptionCode = u64;

/// A filter for multiple exceptions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionExceptionFilter {
    mask: ExecutionExceptionCode,
}

impl Display for ExecutionExceptionFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08x}", self.mask)
    }
}

impl ExecutionExceptionFilter {
    /// The empty exception filter
    pub const CLEAR: ExecutionExceptionCode = 0;

    /// The all exception filter
    pub const ALL: ExecutionExceptionCode = !0;

    /// The system exception filter
    pub const SYSTEM: ExecutionExceptionCode = 0xFF00000000000000;

    /// The user exception filter
    pub const USER: ExecutionExceptionCode = !Self::SYSTEM;

    /// Create a new empty exception filter
    #[inline(always)]
    pub const fn new() -> Self {
        ExecutionExceptionFilter { mask: Self::CLEAR }
    }

    /// Create a new exception filter for the given exception
    #[inline(always)]
    pub const fn for_exception(ex: ExecutionException) -> Self {
        ExecutionExceptionFilter { mask: ex.code() }
    }

    /// Create a new exception filter for a timeout exception
    /// Convenience for `ExecutionExceptionFilter::for_exception(ExecutionException::timeout())`
    #[inline(always)]
    pub const fn for_timeout() -> Self {
        Self::for_exception(ExecutionException::timeout())
    }

    /// Create a new exception filter for system execptions
    #[inline(always)]
    pub const fn for_system() -> Self {
        ExecutionExceptionFilter { mask: Self::SYSTEM }
    }

    /// Create a new exception filter for user execptions
    #[inline(always)]
    pub const fn for_user() -> Self {
        ExecutionExceptionFilter { mask: Self::USER }
    }

    /// Create a new exception filter for all exceptions
    #[inline(always)]
    pub const fn for_all() -> Self {
        ExecutionExceptionFilter { mask: Self::ALL }
    }

    /// Add an exception to the mask
    #[inline(always)]
    pub fn and_exception(mut self, exception: ExecutionException) -> Self {
        self.mask |= exception.code();
        self
    }

    /// Check if a given exception matches the mask
    #[inline(always)]
    pub const fn matches(&self, exception: &ExecutionException) -> bool {
        exception.code() & self.mask != 0
    }
}

impl Default for ExecutionExceptionFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl From<ExecutionExceptionCode> for ExecutionExceptionFilter {
    fn from(mask: ExecutionExceptionCode) -> Self {
        ExecutionExceptionFilter { mask }
    }
}

/// An ExecutionException represents a single expection thrown during execution
/// There are 64 exception codes available for exceptions, thereof 56 are user defined exceptions.
#[derive(Clone, Copy, PartialEq)]
pub struct ExecutionException {
    code: ExecutionExceptionCode,
}

impl Debug for ExecutionException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Exception({:08x})", self.code)
    }
}

impl ExecutionException {
    /// The invalid exception code
    const INVALID: ExecutionExceptionCode = 0;

    /// The maximum exception id
    pub const MAX: ExecutionExceptionId = 0x3F;

    /// The timeout exception id
    const TIMEOUT: ExecutionExceptionId = 0x38;

    /// The user defined exception start id
    pub const USER: ExecutionExceptionId = 0x00;

    /// The maximum user defined exception id
    pub const USER_MAX: ExecutionExceptionId = 0x37;

    /// Create a new user defined exception with the given code
    #[inline(always)]
    pub const fn new(exception: ExecutionExceptionId) -> Self {
        if exception > ExecutionException::MAX {
            Self {
                code: Self::INVALID,
            }
        } else {
            Self {
                code: 1 << exception,
            }
        }
    }

    /// Create a new invalid exception
    #[inline(always)]
    pub const fn invalid() -> Self {
        Self {
            code: Self::INVALID,
        }
    }

    /// Create a new timeout exception
    /// Convenience for ExecutionException::new(ExecutionException::TIMEOUT)
    #[inline(always)]
    pub const fn timeout() -> Self {
        Self::new(Self::TIMEOUT)
    }

    /// Create a new user defined exception
    /// Convenience for ExecutionException::new(ExecutionException::USER + exception)
    pub const fn user(exception: u8) -> Self {
        if exception > ExecutionException::USER_MAX {
            Self {
                code: Self::INVALID,
            }
        } else {
            ExecutionException::new(ExecutionException::USER + exception)
        }
    }

    /// Check if the exception is valid
    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.code != Self::INVALID
    }

    /// Get the exception code
    #[inline(always)]
    pub const fn code(&self) -> ExecutionExceptionCode {
        self.code
    }
}

/// Predefined exceptions for actions
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExecutionInterruption {
    /// An internal break action has occurred during execution
    Break,

    /// User expection with user defined code
    Exception(ExecutionException),

    /// A runtime failure has occurred
    Failure(ErrorCode),
}

/// The ActionResult enum represents the result of the update function of an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UpdateResult {
    /// The action is busy and requires more time to complete.
    Busy,

    /// The action is ready and can update again, but does not need to
    /// The duration is the consumption of the elapsed time passed on update
    Ready,

    /// The action has completed successfully and cannot update again
    /// The duration is the consumption of the elapsed time passed on update
    Complete,

    /// The action has encountered an interruption and cannot update again
    Interruption(ExecutionInterruption),

    /// The action is in a state where update is illegal
    Err,
}
