// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::RollbackStatement;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor, TransactionStatementExecutor};
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::storage::value::Value;
use crate::storage::{GraphCache, StorageManager};
use crate::txn::log::{TransactionLog, UndoOperation};
use crate::txn::state::OperationType;
use std::collections::HashMap;
use std::sync::RwLock;

pub struct RollbackExecutor {
    // Statement data passed via execute_rollback parameter, not stored
}

impl RollbackExecutor {
    pub fn new(_statement: RollbackStatement) -> Self {
        Self {}
    }
}

impl StatementExecutor for RollbackExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Rollback
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        "ROLLBACK".to_string()
    }

    fn requires_write_permission(&self) -> bool {
        false // Transaction control doesn't require graph write permissions
    }
}

impl TransactionStatementExecutor for RollbackExecutor {
    fn execute_transaction_operation(
        &self,
        context: &ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        // Use the session transaction state to rollback the transaction
        let transaction_state = context.transaction_state().ok_or_else(|| {
            ExecutionError::RuntimeError("No transaction state available".to_string())
        })?;

        // Get the storage manager from context for applying undo operations
        let storage_manager = context.storage_manager.as_ref().ok_or_else(|| {
            ExecutionError::RuntimeError("No storage manager available".to_string())
        })?;

        transaction_state.rollback_transaction_with_storage(Some(storage_manager))?;

        let message = "Transaction rolled back successfully";

        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([(
                "status".to_string(),
                Value::String(message.to_string()),
            )]))],
            variables: vec!["status".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}

impl RollbackExecutor {
    #[allow(dead_code)] // ROADMAP v0.5.0 - Transaction rollback for error recovery and atomicity
    pub fn execute_rollback(
        _statement: &RollbackStatement,
        transaction_manager: &crate::txn::TransactionManager,
        current_transaction: &RwLock<Option<crate::txn::TransactionId>>,
        transaction_logs: &RwLock<HashMap<crate::txn::TransactionId, TransactionLog>>,
        storage: &StorageManager,
    ) -> Result<QueryResult, ExecutionError> {
        // Check if there's an active transaction
        let current_txn = current_transaction.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
        })?;

        let txn_id = match *current_txn {
            Some(id) => id,
            None => {
                drop(current_txn);
                return Err(ExecutionError::RuntimeError(
                    "No transaction in progress".to_string(),
                ));
            }
        };
        drop(current_txn);

        // Apply undo operations from the transaction log
        {
            let mut logs = transaction_logs.write().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction logs lock".to_string())
            })?;

            if let Some(log) = logs.get(&txn_id) {
                log::info!(
                    "ROLLBACK: Found transaction log with {} operations",
                    log.operation_count
                );

                // Apply undo operations in reverse order - each operation knows its own graph_path
                for (i, undo_op) in log.get_rollback_operations().enumerate() {
                    log::info!("ROLLBACK: Applying undo operation {}: {:?}", i, undo_op);
                    Self::apply_undo_operation(undo_op, storage)?;
                }

                log::info!("ROLLBACK: Applied {} undo operations", log.operation_count);
            } else {
                drop(logs);
                return Err(ExecutionError::RuntimeError(
                    "No transaction log available".to_string(),
                ));
            }

            // Remove the transaction log
            logs.remove(&txn_id);
        }

        // End the transaction in the transaction manager
        transaction_manager.rollback_transaction(txn_id)?;

        // Clear transaction state
        {
            let mut current_txn = current_transaction.write().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;
            *current_txn = None;
        }

        let message = "Transaction rolled back successfully";

        Ok(QueryResult {
            rows: vec![Row::from_values(HashMap::from([(
                "status".to_string(),
                Value::String(message.to_string()),
            )]))],
            variables: vec!["status".to_string()],
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        })
    }

    /// Apply an undo operation using the unified data modification flow
    #[allow(dead_code)] // ROADMAP v0.5.0 - Undo operation application for rollback
    fn apply_undo_operation(
        undo_op: &UndoOperation,
        unified_storage: &StorageManager,
    ) -> Result<(), ExecutionError> {
        // Handle batch operations - undo all operations in the batch atomically
        if let UndoOperation::Batch { operations } = undo_op {
            for op in operations {
                Self::apply_undo_operation(op, unified_storage)?;
            }
            return Ok(());
        }

        // Get the graph name from the undo operation
        let graph_name = match undo_op {
            UndoOperation::InsertNode { graph_path, .. }
            | UndoOperation::UpdateNode { graph_path, .. }
            | UndoOperation::DeleteNode { graph_path, .. }
            | UndoOperation::InsertEdge { graph_path, .. }
            | UndoOperation::UpdateEdge { graph_path, .. }
            | UndoOperation::DeleteEdge { graph_path, .. } => graph_path,
            UndoOperation::Batch { .. } => unreachable!("Batch handled above"),
        };

        // Get the current graph for modification
        let mut graph = unified_storage
            .get_graph(graph_name)
            .map_err(|e| {
                ExecutionError::StorageError(format!("Failed to get graph during rollback: {}", e))
            })?
            .ok_or_else(|| {
                ExecutionError::StorageError(format!(
                    "Graph not found during rollback: {}",
                    graph_name
                ))
            })?;

        // Apply the undo operation to the graph
        Self::apply_undo_to_graph(&mut graph, undo_op)?;

        // Save the modified graph back to unified storage
        unified_storage.save_graph(graph_name, graph).map_err(|e| {
            ExecutionError::StorageError(format!(
                "Failed to save rollback changes to storage: {}",
                e
            ))
        })?;

        log::debug!(
            "ROLLBACK: Applied undo operation for graph '{}'",
            graph_name
        );

        Ok(())
    }

    /// Apply an undo operation to a specific graph
    #[allow(dead_code)] // ROADMAP v0.5.0 - Graph-specific undo for transaction rollback
    fn apply_undo_to_graph(
        graph: &mut GraphCache,
        undo_op: &UndoOperation,
    ) -> Result<(), ExecutionError> {
        // Handle batch operations recursively
        if let UndoOperation::Batch { operations } = undo_op {
            for op in operations {
                Self::apply_undo_to_graph(graph, op)?;
            }
            return Ok(());
        }

        match undo_op {
            UndoOperation::InsertNode { node_id, .. } => {
                // Undo: remove the node that was inserted
                graph.remove_node(node_id).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to remove node during rollback: {}",
                        e
                    ))
                })?;
                log::debug!("ROLLBACK: Removed node {} (undo insert)", node_id);
            }
            UndoOperation::InsertEdge { edge_id, .. } => {
                // Undo: remove the edge that was inserted
                graph.remove_edge(edge_id).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to remove edge during rollback: {}",
                        e
                    ))
                })?;
                log::debug!("ROLLBACK: Removed edge {} (undo insert)", edge_id);
            }
            UndoOperation::UpdateNode {
                node_id,
                old_properties,
                old_labels,
                ..
            } => {
                // Undo: restore the old properties and labels
                if let Some(node) = graph.get_node_mut(node_id) {
                    node.properties = old_properties.clone();
                    node.labels = old_labels.clone();
                    log::debug!(
                        "ROLLBACK: Restored node {} properties and labels (undo update)",
                        node_id
                    );
                } else {
                    log::warn!(
                        "ROLLBACK: Node {} not found for property restoration",
                        node_id
                    );
                }
            }
            UndoOperation::UpdateEdge {
                edge_id,
                old_properties,
                old_label,
                ..
            } => {
                // Undo: restore the old properties and label
                if let Some(edge) = graph.get_edge_mut(edge_id) {
                    edge.properties = old_properties.clone();
                    edge.label = old_label.clone();
                    log::debug!(
                        "ROLLBACK: Restored edge {} properties and label (undo update)",
                        edge_id
                    );
                } else {
                    log::warn!(
                        "ROLLBACK: Edge {} not found for property restoration",
                        edge_id
                    );
                }
            }
            UndoOperation::DeleteNode {
                node_id,
                deleted_node,
                ..
            } => {
                // Undo: restore the deleted node
                graph.add_node(deleted_node.clone()).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to restore node during rollback: {}",
                        e
                    ))
                })?;
                log::debug!("ROLLBACK: Restored node {} (undo delete)", node_id);
            }
            UndoOperation::DeleteEdge {
                edge_id,
                deleted_edge,
                ..
            } => {
                // Undo: restore the deleted edge
                graph.add_edge(deleted_edge.clone()).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to restore edge during rollback: {}",
                        e
                    ))
                })?;
                log::debug!("ROLLBACK: Restored edge {} (undo delete)", edge_id);
            }
            UndoOperation::Batch { .. } => {
                // Batch operations are handled above with early return, this should never be reached
                unreachable!("Batch operations should be handled before this match statement");
            }
        }

        Ok(())
    }
}
