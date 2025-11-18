// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::{SetTransactionCharacteristicsStatement, IsolationLevel};
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor, TransactionStatementExecutor};
use crate::storage::value::Value;
use crate::txn::state::OperationType;
use std::collections::HashMap;

pub struct SetTransactionCharacteristicsExecutor {
    statement: SetTransactionCharacteristicsStatement,
}

impl SetTransactionCharacteristicsExecutor {
    pub fn new(statement: SetTransactionCharacteristicsStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for SetTransactionCharacteristicsExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Begin // Using Begin as a general transaction operation type
    }
    
    fn operation_description(&self, _context: &ExecutionContext) -> String {
        "SET TRANSACTION CHARACTERISTICS".to_string()
    }
    
    fn requires_write_permission(&self) -> bool {
        false // Transaction control doesn't require graph write permissions
    }
}

impl TransactionStatementExecutor for SetTransactionCharacteristicsExecutor {
    fn execute_transaction_operation(
        &self,
        _context: &ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        Self::execute_set_characteristics(&self.statement)
    }
}

impl SetTransactionCharacteristicsExecutor {
    pub fn execute_set_characteristics(statement: &SetTransactionCharacteristicsStatement) -> Result<QueryResult, ExecutionError> {
        // For now, just return a success message with the characteristics that would be set
        // In a full implementation, this would:
        // 1. Validate the characteristics
        // 2. Set them for the next transaction
        
        let mut message = "Transaction characteristics set:".to_string();
        
        if let Some(ref isolation_level) = statement.characteristics.isolation_level {
            message.push_str(&format!(" ISOLATION LEVEL {}", isolation_level.as_str()));
        }
        
        if let Some(ref access_mode) = statement.characteristics.access_mode {
            message.push_str(&format!(" {}", access_mode.as_str()));
        }
        
        // For now, we only support READ_COMMITTED
        if let Some(ref isolation_level) = statement.characteristics.isolation_level {
            match isolation_level {
                IsolationLevel::ReadCommitted => {
                    // This is supported
                },
                _ => {
                    return Err(ExecutionError::UnsupportedOperator(
                        format!("Isolation level {} not yet supported. Only READ COMMITTED is currently implemented.", 
                                isolation_level.as_str())
                    ));
                }
            }
        }
        
        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([
                ("status".to_string(), Value::String(message)),
            ]))],
            variables: vec!["status".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}
