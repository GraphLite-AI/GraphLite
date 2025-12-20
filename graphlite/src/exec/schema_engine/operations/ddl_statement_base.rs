// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::catalog::manager::CatalogManager;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;

/// Base trait for all DDL statement executors
pub trait DDLStatementExecutor: StatementExecutor {
    /// Execute the DDL operation
    /// Returns a description message and the number of affected entities
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError>;

    /// Main execution method - handles the complete DDL operation flow
    fn execute(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Pre-execution: check permissions, log to WAL
        self.pre_execute(context)?;

        // Execute the DDL operation
        let (message, affected) = self.execute_ddl_operation(context, catalog_manager, storage)?;

        // Post-execution: any cleanup
        self.post_execute(context, affected)?;

        Ok((message, affected))
    }
}

/// Enum for different DDL statement types
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // ROADMAP v0.5.0 - DDL statement classification for schema management and audit
pub enum DDLStatementType {
    CreateSchema,
    DropSchema,
    CreateGraph,
    DropGraph,
    CreateGraphType,
    DropGraphType,
    TruncateGraph,
    ClearGraph,
    CreateUser,
    DropUser,
    CreateRole,
    DropRole,
    // Index DDL types
    CreateIndex,
    DropIndex,
    AlterIndex,
    OptimizeIndex,
}
