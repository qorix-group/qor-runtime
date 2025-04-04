# iceoryx2 Building Blocks to Rust std Mapping

## Overview

This document presents an overview of how we can utilize iceoryx2 building blocks and other APIs as alternatives to the standard Rust API.

## Version

The mapping is performed based on the following crate versions:
- `std`: 1.85.0
- `iceorxy2`: 0.5.0

## Container (`iceoryx2_bb_container`)

### Vector

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|--------------------|--------------------|----------|---------------------------|---------|
| `Vec` | `Vec`[^1] | yes | no  | run-time fixed-size vector from default heap |
| `FixedSizeVec` | `array`[^2] or `Vec`[^1] | yes | yes | compile-time fixed-size vector |
| `RelocatableVec` | `Vec`[^1] | no | yes | run-time fixed-size vector from custom-allocated heap |

### Queue

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|--------------------|--------------------|----------|---------------------------|---------|
| `Queue` | `VecDeque`[^1] | yes | no | run-time fixed-size double-ended queue from default heap |
| `FixedSizeQueue` | `array`[^2] or `VecDeque`[^1]  | yes | yes | compile-time fixed-size double-ended queue |
| `RelocatableQueue` | `VecDeque`[^1] | no | yes | run-time fixed-size double-ended queue from custom-allocated heap |

### SlotMap

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|--------------------|--------------------|----------|---------------------------|---------|
| `SlotMap` | `HashMap`[^3] | yes | no | run-time fixed-sized (integer) key-based value map from default heap |
| `FixedSizeSlotMap` | `HashMap`[^3] | yes | no | compile-time fixed-size (integer) key-based value map |
| `RelocatableSlotMap` | `HashMap`[^3] | no | yes | run-time fixed-size (integer) key-based from custom-allocated heap |

### String

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|--------------------|--------------------|----------|---------------------------|---------|
| `FixedSizeByteString` | `String`[^4] | yes | yes | compile-time fixed-size null-terminated string (non UTF-8) |
| `SemanticString` | `String`[^4] | yes | yes | `FixedSizeByteString` with content/char validator |

[^1]: `std` container types are growable at run-time
[^2]: `std` arrays are immutable
[^3]: `std::collections::HashMap` uses key's hash as key and is growable 
[^4]: `std::String` is UTF-8, growable and not null-terminated

## Thread-safe lock-free constructs (`iceoryx2_bb_lock_free`)

### Single producer single consumer (spsc)

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|---------------|------------------|----------|-------------------------------|---------|
| `spsc::Queue` | - | yes | yes | compile-time fixed-size spsc queue |
| `IndexQueue` | - | yes | no | run-time fixed-size spsc queue for integer values only |
| `FixedSizeIndexQueue` | - | yes | yes | compile-time fixed-size version of `IndexQueue` |
| `RelocatableIndexQueue` | - | no | yes | run-time fixed-size version of `IndexQueue` with custom-allocater |
| `SafelyOverflowingIndexQueue` | - | yes | no | Similar to `IndexQueue`, but when the queue is full the oldest element is returned to the producer and replaced with the newest |
| `FixedSizeSafelyOverflowingIndexQueue` | - | yes | yes | compile-time fixed-size version of `SafelyOverflowingIndexQueue` |
| `RelocatableSafelyOverflowingIndexQueue` | - | no | yes | run-time fixed-size version of `SafelyOverflowingIndexQueue` with custom allocator |

### Single producer multi consumer (spmc)

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|---------------|------------------|----------|-------------------------------|---------|
| `UnrestrictedAtomic` | `sync::Atomic` | yes[^6] | no[^6] | similar to `Atomic` but can hold arbitrary type |

[^6]: Assumed values, since not officially stated in the docs

### Multi producer multi consumer (mpmc)

| `iceoryx2` type | similar `std` type | movable? | shared-memory compatible? | Comment |
|---------------|------------------|----------|-------------------------------|---------|
| `BitSet` | - | yes | no | run-time fixed-sized bitset from default heap |
| `FixedSizeBitSet` | - | yes | no | compile-time fixed-size bitset |
| `RelocatableBitSet` | - | no | yes | run-time fixed-size bitset from custom-allocated heap |
| `Container` | `sync::channel`[^7] or `sync::sync_channel`[^8]  | yes | no | run-time fixed-sized mpmc FIFO queue from default heap |
| `FixedSizeContainer` | `sync::channel`[^7] or `sync::sync_channel`[^8]   | yes | no | compile-time fixed-size mpmc FIFO queue map |
| `RelocatableContainer` | `sync::channel`[^7] or `sync::sync_channel`[^8]   | no | yes | run-time fixed-size mpmc FIFO queue from custom-allocated heap |
| `UniqueIndexSet` | `sync::channel`[^7] or `sync::sync_channel`[^8]  | no | yes | run-time fixed-sized mpmc FIFO queue for integers from custom-allocated heap |
| `FixedSizeUniqueIndexSet` | `sync::channel`[^7] or `sync::sync_channel`[^8]   | yes | yes | compile-time version of `UniqueIndexSet` |

[^7]: Currently still experimental in `1.85.0`. Unlike `std::sync::channel`, the `Container` is fixed-sized
[^8]: Currently still experimental in `1.85.0`. Unlike `std::sync::sync_channel`, the `Container` is not blocking

## POSIX abstraction (`iceoryx2_bb_posix`)

### Concurrency

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `AdaptiveWait` | - | wait with auto-increasing waiting time to reduce CPU consumption  |
| `Barrier` | `sync::Barrier` | |
| `MultiConditionVariable` and `ConditionVariable` | `sync::CondVar` | block thread while waiting on multiple or single condition, respectively |
| `DeadlineQueue` | - | wait on multiple periodic deadlines and allow monitoring the missed ones |
| `MessageQueue` | - | async/sync channel with fixed-size queue size |
| `Mutex` | `sync::Mutex` | mutual exclusion primitive |
| `ReadWriteMutex` | `sync::RwLock` | allow multiple read-locks or one write-lock on shared resource |
| `SharedMemory` | - | create, open and remove shared memory object |
| `Signal` | - | POSIX signal handling |

### Filesystem

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `Directory` and `DirectoryEntry` | `fs::DirEntry` | directory representation |
| `File` | `fs::File` | read, create, write or modify files  |
| `FileDescriptor` | `os::BorrowedFd` or `os::OwnedFd` | file descriptor representation |
| `FileDescriptorSet` | - | useful for waiting on multiple objects |
| `FileLock` | - | lock a file for exclusive writing or multiple reading |
| `Group` | - | POSIX system's group representation |
| `Metadata` | `fs::Metadata` | metadata info about a file (permissions, size, etc.) |
| `Ownership` | - | file ownership representation (user and group) |
| `Permission` | `fs::Permissions` | representation of various permissions on a file |

### IPC

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `SharedMemory` | - | create, open and remove shared memory object |
| `SocketAncillary` and `SocketCred` | - | send and receive message & creds via UNIX Datagram Socket |
| `UdpSocket` | `net::UdpSocket` | create UDP socket |
| `UnixDatagramSocket` | - | create UNIX domain sockets sender/receiver |

### Memory

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `Heap` | - | perform low-level heap allocations |
| `MemoryLock` | - | exclude a specific memory region from being moved into the swap space |

### Process & Threads

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `Thread` and `ThreadGuardedStack` | `Thread` | create thread with a custom sized with or without a guarded stack  |
| `Process` | - | representation of POSIX process (pid, priority, scheduler, etc.) |
| `ProcessState` | - | monitor the status of other processes |

### Utilities

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `Time` | `time::Instant` or `time:SystemTime` | time representation under `Monotonic` or `Realtime` clock type |
| `UniqueSystemId` | - | create system wide unique id |

## Elementary (`iceoryx2_bb_elementary`)

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `Alignment` | `ptr::Alignment`[^9] | alignment memory representation |
| `BumpAllocator` | - | simple bump allocator that allocates memory linearly from some start address of the heap and deallocates the whole region upon free |
| `LazySingleton` | - | create singleton objects that are not initialized upon creation |
| `OwningPointer` | - | normal non-null pointer representation with absolute address to distinguish from `RelocatablePointer`  |
| `PackageVersion` | - | crate version representation obtained from the internal env vars  |
| `RelocatablePointer` | - | pointer representation that stores only the pointee's location as offset to its starting position (useful for IPC with multiple shared memory objects) |
| `ScopeGuard` | - | a guard that runs a pre-defined closure as soon as it goes out of scope (useful for working with low level HW/OS resources) |
| `StaticAssert` | - | compile time assertions |
| `TypedUniqueId` and `UniqueId` | - | generate typed or non-typed global unique ids |

[^9]: Still experimental in ´1.85.0´

## Memory (`iceoryx2_bb_memory`)

| `iceoryx2` type | similar `std` type | Comment |
|-----------------|--------------------|---------|
| `BumpAllocator` | - | thread-safe lock-free version of `BumpAllocator` |
| `HeapAllocator` | - | similar to `BumpAllocator` but with grow and shrink capabilities |
| `OneChunkAllocator` | - | non-threadsafe allocator that allocates, grows, shrinks and deallocates from only one chunk of memory |
| `PoolAllocator` | - | thread-safe lock-free allocator that partitions memory into buckets of equal size with a given alignment |
