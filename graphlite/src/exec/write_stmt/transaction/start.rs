// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::{StartTransactionStatement, IsolationLevel, AccessMode};
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor, TransactionStatementExecutor};
use crate::storage::value::Value;
use crate::txn::{log::TransactionLog, state::OperationType};
use std::sync::RwLock;
use std::collections::HashMap;

pub struct StartTransactionExecutor {
    // Statement data passed via execute_start_transaction parameter, not stored
}

impl StartTransactionExecutor {
    pub fn new(_statement: StartTransactionStatement) -> Self {
        Self { }
    }
}

impl StatementExecutor for StartTransactionExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Begin
    }
    
    fn operation_description(&self, _context: &ExecutionContext) -> String {
        "START TRANSACTION".to_string()
    }
    
    fn requires_write_permission(&self) -> bool {
        false // Transaction control doesn't require graph write permissions
    }
}

impl TransactionStatementExecutor for StartTransactionExecutor {
    fn execute_transaction_operation(
        &self,
        context: &ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        // Use the session transaction state to begin a transaction
        let transaction_state = context.transaction_state()
            .ok_or_else(|| ExecutionError::RuntimeError("No transaction state available".to_string()))?;
        let txn_id = transaction_state.begin_transaction()?;
        
        let message = "Transaction started successfully";
        
        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([
                ("status".to_string(), Value::String(message.to_string())),
                ("transaction_id".to_string(), Value::String(format!("{:?}", txn_id))),
            ]))],
            variables: vec!["status".to_string(), "transaction_id".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}

impl StartTransactionExecutor {
    #[allow(dead_code)] // ROADMAP v0.5.0 - Explicit transaction start for multi-statement transactions
    pub fn execute_start_transaction(
        statement: &StartTransactionStatement,
        transaction_manager: &crate::txn::TransactionManager,
        current_transaction: &RwLock<Option<crate::txn::TransactionId>>,
        transaction_logs: &RwLock<HashMap<crate::txn::TransactionId, TransactionLog>>,
    ) -> Result<QueryResult, ExecutionError> {
        // Check if there's already an active transaction
        let current_txn = current_transaction.read()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string()))?;
        
        if current_txn.is_some() {
            return Err(ExecutionError::RuntimeError("Transaction already in progress".to_string()));
        }
        drop(current_txn);
        
        // Extract isolation level and access mode from statement
        let (isolation_level, access_mode) = if let Some(ref characteristics) = statement.characteristics {
            (characteristics.isolation_level.clone(), characteristics.access_mode.clone())
        } else {
            (None, None)
        };
        
        // Start transaction using transaction manager
        let txn_id = transaction_manager.start_transaction(
            isolation_level.map(|il| match il {
                IsolationLevel::ReadUncommitted => crate::txn::isolation::IsolationLevel::ReadUncommitted,
                IsolationLevel::ReadCommitted => crate::txn::isolation::IsolationLevel::ReadCommitted,
                IsolationLevel::RepeatableRead => crate::txn::isolation::IsolationLevel::RepeatableRead,
                IsolationLevel::Serializable => crate::txn::isolation::IsolationLevel::Serializable,
            }),
            access_mode.map(|am| match am {
                AccessMode::ReadOnly => crate::txn::state::AccessMode::ReadOnly,
                AccessMode::ReadWrite => crate::txn::state::AccessMode::ReadWrite,
            }),
        )?;
        
        // Initialize transaction log for this transaction
        {
            let mut logs = transaction_logs.write()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction logs lock".to_string()))?;
            logs.insert(txn_id, TransactionLog::new(txn_id));
        }
        
        // Set current transaction
        {
            let mut current_txn = current_transaction.write()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string()))?;
            *current_txn = Some(txn_id);
        }
        
        let message = "Transaction started successfully";
        
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
