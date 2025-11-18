// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::CommitStatement;
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor, TransactionStatementExecutor};
use crate::storage::value::Value;
use crate::txn::{log::TransactionLog, state::OperationType};
use std::sync::RwLock;
use std::collections::HashMap;

pub struct CommitExecutor {
    // Statement data passed via execute_commit parameter, not stored
}

impl CommitExecutor {
    pub fn new(_statement: CommitStatement) -> Self {
        Self { }
    }
}

impl StatementExecutor for CommitExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Commit
    }
    
    fn operation_description(&self, _context: &ExecutionContext) -> String {
        "COMMIT".to_string()
    }
    
    fn requires_write_permission(&self) -> bool {
        false // Transaction control doesn't require graph write permissions
    }
}

impl TransactionStatementExecutor for CommitExecutor {
    fn execute_transaction_operation(
        &self,
        context: &ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        // Use the session transaction state to commit the transaction
        let transaction_state = context.transaction_state()
            .ok_or_else(|| ExecutionError::RuntimeError("No transaction state available".to_string()))?;
        transaction_state.commit_transaction()?;
        
        let message = "Transaction committed successfully";
        
        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([
                ("status".to_string(), Value::String(message.to_string())),
            ]))],
            variables: vec!["status".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}

impl CommitExecutor {
    #[allow(dead_code)] // ROADMAP v0.5.0 - Explicit transaction commit for ACID guarantees
    pub fn execute_commit(
        _statement: &CommitStatement,
        transaction_manager: &crate::txn::TransactionManager,
        current_transaction: &RwLock<Option<crate::txn::TransactionId>>,
        transaction_logs: &RwLock<HashMap<crate::txn::TransactionId, TransactionLog>>,
    ) -> Result<QueryResult, ExecutionError> {
        // Check if there's an active transaction
        let current_txn = current_transaction.read()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string()))?;
        
        let txn_id = match *current_txn {
            Some(id) => id,
            None => {
                drop(current_txn);
                return Err(ExecutionError::RuntimeError("No transaction in progress".to_string()));
            }
        };
        drop(current_txn);
        
        // Commit the transaction in the transaction manager
        transaction_manager.commit_transaction(txn_id)?;
        
        // Remove the transaction log since we're committing
        {
            let mut logs = transaction_logs.write()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction logs lock".to_string()))?;
            logs.remove(&txn_id);
        }
        
        // Clear transaction state
        {
            let mut current_txn = current_transaction.write()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string()))?;
            *current_txn = None;
        }
        
        // Persistence is now handled automatically by the unified data modification flow
        
        let message = "Transaction committed successfully";
        
        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([
                ("status".to_string(), Value::String(message.to_string())),
            ]))],
            variables: vec!["status".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}
