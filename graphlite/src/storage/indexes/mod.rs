// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Graph indexing system for GraphLite
//!
//! This module provides indexing support for:
//! - Graph indexes (adjacency lists, paths, reachability)
//!
//! All indexes are designed to be partition-aware for future distribution.

pub mod traits;
pub mod types;
pub mod errors;
pub mod manager;
pub mod metrics;

// Re-export core types
pub use types::*;
pub use errors::*;
pub use manager::*;
