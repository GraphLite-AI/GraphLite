// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! GraphLite - A lightweight ISO GQL Graph Database
//!
//! GraphLite is a standalone graph database that implements the ISO GQL standard.
//!
//! # Features
//!
//! - **ISO GQL Compliance**: Full implementation of the ISO GQL standard
//! - **Pattern Matching**: Powerful graph pattern matching with MATCH clauses
//! - **ACID Transactions**: Full transaction support with isolation levels
//! - **Embedded Database**: Uses Sled for embedded, serverless storage
//! - **Type System**: Strong type system with validation and inference
//! - **Query Optimization**: Cost-based query optimization
//!
//! # Usage
//!
//! GraphLite is primarily used as a standalone database via the CLI:
//!
//! ```bash
//! # Install database
//! graphlite install --path ./mydb --admin-user admin
//!
//! # Start interactive console
//! graphlite gql --path ./mydb -u admin
//!
//! # Execute queries
//! graphlite query --path ./mydb -u admin "MATCH (n:Person) RETURN n"
//! ```
//!
//! See the documentation for more details:
//! - [Getting Started Guide](../docs/tutorials/Getting-started.md)
//! - [System Procedures](../docs/reference/System-procedures.md)

// Public modules - exposed to external users
pub mod coordinator;

// Internal modules - only visible within graphlite crate
pub(crate) mod ast;
pub(crate) mod cache;
pub(crate) mod catalog;
pub(crate) mod exec;
pub(crate) mod functions;
pub(crate) mod plan;
pub(crate) mod schema;
pub(crate) mod session;
pub(crate) mod storage;
pub(crate) mod txn;
pub(crate) mod types;

// Re-export the public API - QueryCoordinator is the only entry point
pub use coordinator::{QueryCoordinator, QueryInfo, QueryPlan, QueryResult, QueryType, Row};

// Re-export Value type (needed for inspecting query results in Row.values)
pub use storage::Value;

/// GraphLite version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// GraphLite crate name
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
