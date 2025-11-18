// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::QueryResult;

/// Base trait for transaction statement executors
pub trait TransactionStatementExecutor: StatementExecutor {
    /// Execute the transaction statement
    /// Returns a QueryResult with status information
    fn execute_transaction_operation(
        &self,
        context: &ExecutionContext,
    ) -> Result<QueryResult, ExecutionError>;
    
    /// Transaction statements typically don't require write permissions on graphs
    /// They manage transaction state instead
    #[allow(dead_code)] // ROADMAP v0.6.0 - Permission-based transaction control
    fn requires_write_permission(&self) -> bool {
        false // Transaction statements manage transaction state, not graph data
    }
}
