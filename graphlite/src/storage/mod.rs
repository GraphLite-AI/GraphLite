// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Graph storage implementation for in-memory graph data
//!
//! This module provides:
//! - Value type system for graph properties
//! - In-memory graph storage with adjacency lists
//! - Efficient indexing for nodes and edges by label
//! - Graph operations (add, get, find)
//! - Sample fraud data generation
//! - Pluggable storage backend trait for different KV stores

mod data_adapter;
pub mod graph_cache;
pub mod indexes;
pub mod multi_graph;
mod persistent;
pub mod storage_manager;
pub mod type_mapping;
pub mod types;
pub mod value;

pub use graph_cache::GraphCache;
pub use types::{Edge, Node, StorageError};
pub use value::{TimeWindow, Value};
// Only expose StorageType for configuration
pub use persistent::StorageType;
// Public exports for examples and tests
pub use persistent::{StorageDriver, StorageTree};
// Public interface - only StorageManager should be used externally
pub use storage_manager::{StorageManager, StorageMethod};
// Index system (stub)
// TTL management
// pub use ttl_manager::{TTLManager, TTLCleanupStats};  // TODO: Not yet extracted

// Re-export common types for convenience
