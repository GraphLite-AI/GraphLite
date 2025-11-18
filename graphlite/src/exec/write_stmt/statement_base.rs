// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::exec::ExecutionError;
use crate::exec::write_stmt::ExecutionContext;
use crate::txn::state::OperationType;
use async_trait::async_trait;

/// Base trait for all statement executors providing common infrastructure
#[async_trait]
pub trait StatementExecutor: Send + Sync {
    /// Get the operation type for this executor
    fn operation_type(&self) -> OperationType;
    
    /// Get a description of the operation
    fn operation_description(&self, context: &ExecutionContext) -> String;
    
    /// Check if this statement requires write permissions
    #[allow(dead_code)] // ROADMAP v0.6.0 - Permission-based statement authorization
    fn requires_write_permission(&self) -> bool {
        true // Default to requiring write permission for safety
    }
    
    /// Pre-execution: WAL logging (permissions are checked at higher level)
    fn pre_execute(&self, context: &ExecutionContext) -> Result<(), ExecutionError> {
        // Note: Permission checks are handled at higher level before executor is called
        
        // Log to WAL using context's method
        let description = self.operation_description(context);
        context.log_operation_to_wal(self.operation_type(), description)?;
        
        Ok(())
    }
    
    /// Post-execution: optional cleanup
    fn post_execute(&self, _context: &ExecutionContext, _rows_affected: usize) -> Result<(), ExecutionError> {
        // Default: no post-execution actions needed
        Ok(())
    }
}
