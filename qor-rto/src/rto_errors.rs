// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
use qor_core::core_errors::RTO;
use qor_core::ErrorCode;

/// An invalid action was given
pub const INVALID_ACTION: ErrorCode = RTO + 1;

/// An invalid state was given
pub const INVALID_STATE: ErrorCode = RTO + 2;

/// An invalid operation was given
pub const INVALID_OPERATION: ErrorCode = RTO + 3;

/// The evaluation of an expression failed
pub const EVALUATION_FAILURE: ErrorCode = RTO + 4;

/// The event is already subscribed
pub const EVENT_SUBSCRIBER_NOT_FOUND: ErrorCode = RTO + 5;

/// The event already has the given subscriber
pub const EVENT_SUBSCRIBER_ALREADY_EXISTS: ErrorCode = RTO + 6;

/// No more subscribers can be added to the event
pub const EVENT_SUBSCRIBERS_FULL: ErrorCode = RTO + 7;

/// Placeholder for the engine error
pub const ENGINE_ERROR: ErrorCode = RTO + 100;

/// Engine thread failed to spawn
pub const THREAD_SPWAN_ERROR: ErrorCode = RTO + 200;

/// An engine thread faulted on joining operation
pub const THREAD_JOIN_ERROR: ErrorCode = RTO + 201;

/// Task completed but did not reveal a result.
pub const TASK_EMPTY_RESULT: ErrorCode = RTO + 300;

/// (Scheduled) task received a cancel signal but did not cancel
pub const TASK_CANCEL_ERROR: ErrorCode = RTO + 301;

/// Program misses binding to execution engine
pub const PROGRAM_ENGINE_BINDING_MISSING: ErrorCode = RTO + 400;

/// Program misses binding to execution engine
pub const PROGRAM_FIBER_FAILED: ErrorCode = RTO + 401;

/// Program build failed
pub const PROGRAM_BUILD_FAILED: ErrorCode = RTO + 402;
