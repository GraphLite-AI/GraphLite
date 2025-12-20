// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// TODO: Implement DropGraphTypeExecutor
use crate::ast::DropGraphTypeStatement;
use crate::catalog::manager::CatalogManager;
use crate::exec::schema_engine::operations::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

pub struct DropGraphTypeExecutor;

impl DropGraphTypeExecutor {
    pub fn new(_statement: DropGraphTypeStatement) -> Self {
        Self
    }
}

impl StatementExecutor for DropGraphTypeExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropTable // TODO: Add DropGraphType to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        "DROP GRAPH TYPE".to_string()
    }
}

impl DDLStatementExecutor for DropGraphTypeExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        _catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // TODO: Implement drop graph type logic
        Err(ExecutionError::RuntimeError(
            "DropGraphTypeExecutor not yet implemented".to_string(),
        ))
    }
}
