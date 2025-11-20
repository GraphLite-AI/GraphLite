// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query execution engine
//!
//! This module provides the execution engine that takes physical query plans
//! and executes them against graph storage to produce query results.

pub mod context;
pub mod error;
pub mod executor;
pub mod lock_tracker;
pub mod result;
pub mod row_iterator;
pub mod unwind_preprocessor;
pub mod with_clause_processor;
pub mod write_stmt; // Phase 4: Week 6.5 - Memory Optimization
                    // Text search not supported in GraphLite
                    // pub mod text_search_iterator; // Phase 4: Week 6.5 - Lazy text search
pub mod memory_budget;
pub mod streaming_topk; // Phase 4: Week 6.5 - Streaming top-K // Phase 4: Week 6.5 - Memory limit enforcement

// Re-export the main types for convenience
pub use context::ExecutionContext;
pub use error::ExecutionError;
pub use executor::{ExecutionRequest, QueryExecutor};
pub use result::{QueryResult, Row, SessionResult};
// Text search not supported in GraphLite
// pub use text_search_iterator::TextSearchIterator;
