// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query execution engine
//!
//! This module provides the execution engine that takes physical query plans
//! and executes them against graph storage to produce query results.

pub mod error;
pub mod result;
pub mod context;
pub mod executor;
pub mod write_stmt;
pub mod with_clause_processor;
pub mod unwind_preprocessor;
pub mod lock_tracker;
pub mod row_iterator; // Phase 4: Week 6.5 - Memory Optimization
// Text search not supported in GraphLite
// pub mod text_search_iterator; // Phase 4: Week 6.5 - Lazy text search
pub mod streaming_topk; // Phase 4: Week 6.5 - Streaming top-K
pub mod memory_budget; // Phase 4: Week 6.5 - Memory limit enforcement

// Re-export the main types for convenience
pub use error::ExecutionError;
pub use result::{QueryResult, Row, SessionResult};
pub use context::ExecutionContext;
pub use executor::{QueryExecutor, ExecutionRequest};
// Text search not supported in GraphLite
// pub use text_search_iterator::TextSearchIterator;
