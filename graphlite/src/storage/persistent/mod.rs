// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Persistent storage backends
//!
//! This module provides trait-based abstractions for persistent key-value storage,
//! allowing different storage backends (Sled, Memory) to be used interchangeably.
//!
//! These drivers handle raw key-value operations for persistent disk-based storage.
//!
//! # Architecture
//!
//! ```text
//! DataAdapter (application data structures)
//!     ↓
//! StorageDriver (key-value abstraction)
//!     ↓
//! Concrete Implementations (Sled, Memory)
//! ```
//!
//! # Example Usage
//!
//! ```ignore
//! use crate::storage::persistent::{create_storage_driver, StorageType};
//!
//! // Create a driver
//! let driver = create_storage_driver(StorageType::Sled, "./data")?;
//!
//! // Open a tree (like a table or collection)
//! let tree = driver.open_tree("my_data")?;
//!
//! // Basic operations
//! tree.insert(b"key", b"value")?;
//! let value = tree.get(b"key")?;
//! tree.remove(b"key")?;
//! ```

// Core modules
pub mod factory;
pub mod traits;
pub mod types;

// Driver implementations
pub mod memory;
pub mod sled;

// Public API re-exports
pub use factory::create_storage_driver;
pub use traits::{StorageDriver, StorageTree};
pub use types::StorageType;
