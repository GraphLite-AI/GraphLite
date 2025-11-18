//! GraphLite SDK - High-level ergonomic Rust API for GraphLite
//!
//! This crate provides a high-level, developer-friendly SDK on top of GraphLite's core API.
//! It offers ergonomic patterns, type safety, connection management, query builders, and
//! transaction support - everything needed to build robust graph-based applications in Rust.
//!
//! # Quick Start
//!
//! ```no_run
//! use graphlite_sdk::{GraphLite, Error};
//!
//! # fn main() -> Result<(), Error> {
//! // Open database
//! let db = GraphLite::open("./mydb")?;
//!
//! // Create session and execute query
//! let session = db.create_session("admin")?;
//! let result = db.query("MATCH (p:Person) RETURN p.name", &session)?;
//!
//! // Process results
//! for row in result.rows() {
//!     println!("Name: {:?}", row.get("p.name"));
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - **Connection Management** - Simple connection API with automatic session handling
//! - **Query Builder** - Fluent API for building type-safe GQL queries
//! - **Transaction Support** - ACID transactions with automatic rollback
//! - **Typed Results** - Deserialize query results into Rust structs
//! - **Connection Pooling** - Efficient concurrent access (future)
//! - **Async Support** - Full tokio integration (future)
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │   Application Code (Your Rust App)     │
//! └─────────────────────────────────────────┘
//!                  │
//!                  ▼
//! ┌─────────────────────────────────────────┐
//! │  GraphLite SDK (this crate)             │
//! │  - GraphLite (main API)                 │
//! │  - Session (session management)         │
//! │  - QueryBuilder (fluent queries)        │
//! │  - Transaction (ACID support)           │
//! │  - TypedResult (deserialization)        │
//! └─────────────────────────────────────────┘
//!                  │
//!                  ▼
//! ┌─────────────────────────────────────────┐
//! │  GraphLite Core (graphlite crate)       │
//! │  - QueryCoordinator                     │
//! │  - Storage Engine                       │
//! │  - Catalog Manager                      │
//! └─────────────────────────────────────────┘
//! ```
//!
//! # Module Organization
//!
//! - [`connection`] - Database connection and session management
//! - [`query`] - Query builder and execution
//! - [`transaction`] - Transaction support
//! - [`result`] - Result handling and deserialization
//! - [`error`] - Error types and handling

// Re-export core types for convenience
pub use graphlite::{QueryResult, Row, Value, QueryInfo, QueryType, QueryPlan};

// SDK modules
pub mod error;
pub mod connection;
pub mod query;
pub mod transaction;
pub mod result;

// Re-export main types for convenience
pub use error::{Error, Result};
pub use connection::{GraphLite, Session};
pub use query::QueryBuilder;
pub use transaction::Transaction;
pub use result::TypedResult;
