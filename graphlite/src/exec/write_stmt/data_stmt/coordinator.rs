// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::{DataStatement, GraphExpression};
use crate::exec::write_stmt::data_stmt::{
    planned_insert::PlannedInsertExecutor, DataStatementExecutor, DeleteExecutor,
    MatchDeleteExecutor, MatchInsertExecutor, MatchRemoveExecutor, MatchSetExecutor,
    RemoveExecutor, SetExecutor,
};
use crate::exec::write_stmt::ExecutionContext;
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::session::UserSession;
use crate::storage::StorageManager;
use std::sync::Arc;

/// Coordinator for executing data statements using the modular approach
pub struct DataStatementCoordinator;

impl DataStatementCoordinator {
    /// Main entry point for executing data statements
    pub fn execute_data_statement(
        stmt: &DataStatement,
        _graph_expr: Option<&GraphExpression>,
        storage: Arc<StorageManager>,
        session: &Arc<std::sync::RwLock<UserSession>>,
        context: &mut ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        log::debug!(
            "DataStatementCoordinator::execute_data_statement called with statement: {:?}",
            std::mem::discriminant(stmt)
        );
        let start_time = std::time::Instant::now();

        // Use the provided execution context and update session ID if needed
        let session_read = session
            .read()
            .map_err(|e| ExecutionError::RuntimeError(format!("Failed to read session: {}", e)))?;
        let session_id = session_read.session_id.clone();
        drop(session_read); // Release the read lock

        // Update the context's session ID to ensure consistency
        if context.session_id != session_id {
            context.session_id = session_id;
        }
        let stmt_executor: Box<dyn DataStatementExecutor> = match stmt {
            DataStatement::Insert(insert_stmt) => {
                // Use planned execution for INSERT statements
                Box::new(PlannedInsertExecutor::new(insert_stmt.clone()))
            }
            DataStatement::MatchInsert(match_insert_stmt) => {
                Box::new(MatchInsertExecutor::new(match_insert_stmt.clone()))
            }
            DataStatement::Set(set_stmt) => Box::new(SetExecutor::new(set_stmt.clone())),
            DataStatement::MatchSet(match_set_stmt) => {
                log::debug!("DataStatementCoordinator: Creating MatchSetExecutor");
                Box::new(MatchSetExecutor::new(match_set_stmt.clone()))
            }
            DataStatement::Remove(remove_stmt) => {
                Box::new(RemoveExecutor::new(remove_stmt.clone()))
            }
            DataStatement::MatchRemove(match_remove_stmt) => {
                Box::new(MatchRemoveExecutor::new(match_remove_stmt.clone()))
            }
            DataStatement::Delete(delete_stmt) => {
                Box::new(DeleteExecutor::new(delete_stmt.clone()))
            }
            DataStatement::MatchDelete(match_delete_stmt) => {
                Box::new(MatchDeleteExecutor::new(match_delete_stmt.clone()))
            }
        };
        log::debug!("DataStatementCoordinator: Calling executor.execute()");
        let result = stmt_executor.execute(context, &storage);
        let execution_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok((message, rows_affected)) => {
                // Changes are already persisted via unified data modification flow
                log::debug!("Data modifications completed successfully");

                // Collect warnings from execution context
                let warnings = context.get_warnings().to_vec();

                let result = QueryResult {
                    rows_affected,
                    session_result: None,
                    rows: vec![Row {
                        values: std::collections::HashMap::from([(
                            "status".to_string(),
                            crate::storage::Value::String(message.clone()),
                        )]),
                        positional_values: vec![crate::storage::Value::String(message)],
                        source_entities: std::collections::HashMap::new(),
                        text_score: None,
                        highlight_snippet: None,
                    }],
                    variables: vec!["status".to_string()],
                    execution_time_ms: execution_time,
                    warnings: warnings.clone(),
                };

                // If there are warnings, add them to the result rows for visibility
                if !warnings.is_empty() {
                    log::info!("Query completed with {} warning(s)", warnings.len());
                    for warning in &warnings {
                        log::warn!("  {}", warning);
                    }
                }

                Ok(result)
            }
            Err(e) => Err(e),
        }
    }
}
