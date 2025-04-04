// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

use iceoryx2_bb_container::queue::Queue;

use crate::containers::spmc_queue::LocalProducerConsumer;

type ArcInternal<T> = std::sync::Arc<T>;
type MutexInternal<T> = std::sync::Mutex<T>;
type MutexGuardInternal<'a, T> = std::sync::MutexGuard<'a, T>;

///
/// A struct wrapping the iceoryx2 Queue
///
/// It supports pushing from and popping to a SpmcStealQueue.
/// Pushing has to go through the LocalProducerConsumer.
///
pub struct MpmcQueue<T> {
    inner: ArcInternal<MutexInternal<Queue<T>>>,
}

impl<T: Send> MpmcQueue<T> {
    ///
    /// Initialize a MpmcQueue
    ///
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: ArcInternal::new(MutexInternal::new(Queue::new(capacity))),
        }
    }

    ///
    /// Pop an item from the queue.
    ///
    pub fn pop(&self) -> Option<T> {
        self.inner.lock().unwrap().pop()
    }

    ///
    /// Push an item into the queue.
    ///
    pub fn push(&self, item: T) -> bool {
        self.inner.lock().unwrap().push(item)
    }

    ///
    /// Pop multiple items into a slice. The queue lock is taken with try_lock. If the lock could
    /// not be aquired, the incoming `dst` slice stays unaltered and function returns.
    ///
    pub fn pop_slice(&self, dst: &mut [Option<T>]) {
        if let Ok(ref mut inner_queue) = self.inner.try_lock() {
            for i in 0..dst.len() {
                if let Some(item) = inner_queue.pop() {
                    dst[i] = Some(item);
                } else {
                    break;
                }
            }
        }
    }

    ///
    /// Take a look, if the queue is empty.
    ///
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }

    ///
    /// Returns a Lock to the inner queue
    pub(crate) fn lock(&self) -> MutexGuardInternal<'_, Queue<T>> {
        self.inner.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::spmc_queue::SpmcStealQueue;

    #[test]
    fn test_1_thread_pop_slice() {
        let src: MpmcQueue<u32> = MpmcQueue::new(20);
        for i in 0..20 {
            assert!(src.inner.lock().unwrap().push(i));
        }

        let d: SpmcStealQueue<u32> = SpmcStealQueue::new(32);
        let dst = d.get_local().unwrap();

        let mut dst = [None; 10];
        src.pop_slice(&mut dst);
        // check if all 10 items contain something
        assert_eq!(dst.iter().all(|&x| x.is_some()), true);

        for i in 0..10 {
            assert_eq!(dst[i], Some(i as u32));
        }

        for i in 10..20 {
            assert_eq!(src.inner.lock().unwrap().pop(), Some(i));
        }

        assert_eq!(src.inner.lock().unwrap().pop(), None);
    }
}
