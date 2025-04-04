// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use foundation::prelude::CommonErrors;

use crate::AsyncTask;
use core::task::Poll;
use std::{cell::Cell, future::Future, sync::Arc};

pub struct JoinHandle<T: 'static> {
    for_task: Arc<AsyncTask<T>>, // To make it simple we keep real type instance, not TaskRef
    is_first_time: Cell<bool>,
}

impl<T> JoinHandle<T> {
    pub(crate) fn new(task: Arc<AsyncTask<T>>) -> Self {
        Self {
            for_task: task,
            is_first_time: Cell::new(true),
        }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        if self.is_first_time.get() {
            let waker = cx.waker();

            // First set a waker field
            self.for_task.set_waker(waker.clone());

            // Then set marker
            // Safety - belows forms AquRel so waker is really written before we do marking
            let was_set = self.for_task.header.state.set_waker();

            if !was_set {
                let ret = self.for_task.get_future_ret();

                return match ret {
                    Ok(v) => Poll::Ready(v),
                    Err(_) => todo!(), // TODO: decide what to do here, in theory we shall never be here.
                };
            }

            self.is_first_time.replace(false);
            Poll::Pending
        } else {
            // Safety belows forms AqrRel so waker is really written before we do marking

            let ret = self.for_task.get_future_ret();

            return match ret {
                Ok(v) => Poll::Ready(v),
                Err(CommonErrors::NoData) => Poll::Pending,
                Err(_) => todo!(), // TODO: decide what to do here, in theory we shall never be here.
            };
        }
    }
}
