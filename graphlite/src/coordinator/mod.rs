// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query Coordinator - Central orchestration for query execution
//!
//! The QueryCoordinator provides a unified entry point for query execution,
//! properly coordinating all database components (session, storage, catalog, execution).

pub mod query_coordinator;

pub use query_coordinator::{QueryCoordinator, QueryInfo, QueryType, QueryPlan};

// Re-export types needed for the public API
pub use crate::exec::{QueryResult, Row};
