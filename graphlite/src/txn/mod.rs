// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Transaction management module for GQL database system
//! 
//! This module provides transaction control and ACID properties for the GQL database.
//! Currently implements basic transaction control commands with READ_COMMITTED isolation level.
//! 
//! # Features
//! - Transaction lifecycle management (BEGIN/START, COMMIT, ROLLBACK)
//! - Transaction isolation levels (READ_COMMITTED supported, others planned)
//! - Transaction access modes (READ ONLY, READ WRITE)
//! - Transaction state tracking and management
//! 
//! # Planned Features
//! - Full ACID properties implementation
//! - Multiple isolation levels (READ_UNCOMMITTED, REPEATABLE_READ, SERIALIZABLE)
//! - Deadlock detection and resolution
//! - Transaction logging and recovery
//! - Nested transaction support

pub mod manager;
pub mod state;
pub mod isolation;
pub mod log;
pub mod wal;
pub mod recovery;

pub use manager::TransactionManager;
pub use state::TransactionId;
pub use isolation::IsolationLevel;
pub use log::{TransactionLog, UndoOperation};
