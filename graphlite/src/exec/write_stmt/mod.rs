// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! DEPRECATED: This module is maintained for backward compatibility.
//! New code should import from:
//! - write_engine::operations for data operations
//! - schema_engine::operations for DDL operations
//! - transaction_engine::operations for transaction operations

pub mod statement_base;

// Re-export from new locations for backward compatibility
pub mod data_stmt {
    pub use crate::exec::write_engine::operations::*;
}

pub mod ddl_stmt {
    pub use crate::exec::schema_engine::operations::*;
}

pub mod transaction {
    pub use crate::exec::transaction_engine::operations::*;
}

pub use crate::exec::context::ExecutionContext;
pub use statement_base::StatementExecutor;
pub use transaction::{TransactionCoordinator, TransactionStatementExecutor};
