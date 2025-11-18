// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::TransactionStatement;
use crate::exec::{ExecutionError, QueryResult, QueryExecutor};
use crate::exec::write_stmt::{ExecutionContext, TransactionStatementExecutor};

use super::{
    start::StartTransactionExecutor,
    commit::CommitExecutor,
    rollback::RollbackExecutor,
    set_characteristics::SetTransactionCharacteristicsExecutor,
};

pub struct TransactionCoordinator;

impl TransactionCoordinator {
    pub fn execute_transaction_statement(
        statement: &TransactionStatement,
        context: &ExecutionContext,
        _executor: &QueryExecutor,
    ) -> Result<QueryResult, ExecutionError> {
        // Pre-execute: WAL logging and permissions
        let stmt_executor: Box<dyn TransactionStatementExecutor> = match statement {
            TransactionStatement::StartTransaction(start_stmt) => {
                Box::new(StartTransactionExecutor::new(start_stmt.clone()))
            },
            TransactionStatement::Commit(commit_stmt) => {
                Box::new(CommitExecutor::new(commit_stmt.clone()))
            },
            TransactionStatement::Rollback(rollback_stmt) => {
                Box::new(RollbackExecutor::new(rollback_stmt.clone()))
            },
            TransactionStatement::SetTransactionCharacteristics(set_stmt) => {
                Box::new(SetTransactionCharacteristicsExecutor::new(set_stmt.clone()))
            },
        };
        stmt_executor.pre_execute(context)?;
        stmt_executor.execute_transaction_operation(context)
    }
}
