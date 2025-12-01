//! # InMemoryTable
//!
//! A Rust library for managing typed records in POSIX shared memory with IPC semaphore synchronization.
//!
//! This crate provides a high-level interface for creating, accessing, and manipulating tables
//! stored in shared memory, enabling efficient inter-process communication (IPC) with type-safe
//! record handling.
//!
//! ## ⚠️ Development Status
//!
//! **WARNING: This library is currently under active development and is NOT production-ready.**
//!
//! - The API may change without notice
//! - Not all edge cases have been tested
//! - Performance optimizations are ongoing
//! - Use at your own risk in production environments
//!
//! ## Core Concepts
//!
//! ### Shared Memory
//!
//! The library uses POSIX shared memory (`/dev/shm` on Linux) to create memory segments
//! that can be accessed by multiple processes. Each table is identified by a unique name
//! and persists until explicitly destroyed or the system reboots.
//!
//! ### IPC Semaphores
//!
//! Synchronization is handled through System V IPC semaphores, ensuring that concurrent
//! access from multiple processes is safe. The library automatically manages locks during
//! read and write operations.
//!
//! ### Record Structure
//!
//! Each record type must implement the [`record::TableRecord`] trait, which defines:
//! - A primary key type (must be `Eq + Hash + Copy + Ord + Default`)
//! - A method to extract the key from a record
//!
//! Records must also implement `Serialize` and `Deserialize` from `serde` for storage.
//!

mod internal;

pub mod error;
pub mod index;
pub mod primitive;
pub mod record;
pub mod table;
