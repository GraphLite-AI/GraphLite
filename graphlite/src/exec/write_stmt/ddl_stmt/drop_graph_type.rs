// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// TODO: Implement DropGraphTypeExecutor
use crate::ast::ast::DropGraphTypeStatement;
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::catalog::manager::CatalogManager;
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
        format!("DROP GRAPH TYPE")
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
        Err(ExecutionError::RuntimeError("DropGraphTypeExecutor not yet implemented".to_string()))
    }
}
