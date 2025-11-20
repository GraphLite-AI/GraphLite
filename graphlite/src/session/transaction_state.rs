// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Session-scoped transaction state management
//!
//! This module provides transaction state management at the session level,
//! ensuring proper isolation and consistency for multi-statement transactions.

use crate::exec::ExecutionError;
use crate::txn::isolation::IsolationLevel;
use crate::txn::{TransactionId, TransactionLog, TransactionManager, UndoOperation};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Session-scoped transaction state
#[derive(Clone)]
pub struct SessionTransactionState {
    /// The transaction manager (shared across all sessions)
    manager: Arc<TransactionManager>,

    /// Current active transaction for this session (if any)
    current_transaction: Arc<RwLock<Option<TransactionId>>>,

    /// Transaction logs for rollback operations (per transaction)
    transaction_logs: Arc<RwLock<HashMap<TransactionId, TransactionLog>>>,

    /// Auto-commit mode for this session
    auto_commit: Arc<RwLock<bool>>,

    /// Transaction isolation level
    isolation_level: Arc<RwLock<IsolationLevel>>,
}

impl SessionTransactionState {
    /// Create a new session transaction state
    pub fn new(manager: Arc<TransactionManager>) -> Self {
        Self {
            manager,
            current_transaction: Arc::new(RwLock::new(None)),
            transaction_logs: Arc::new(RwLock::new(HashMap::new())),
            auto_commit: Arc::new(RwLock::new(true)),
            isolation_level: Arc::new(RwLock::new(IsolationLevel::ReadCommitted)),
        }
    }

    /// Get the transaction manager
    pub fn manager(&self) -> Arc<TransactionManager> {
        self.manager.clone()
    }

    /// Get the current transaction ID (if any)
    pub fn current_transaction_id(&self) -> Result<Option<TransactionId>, ExecutionError> {
        self.current_transaction
            .read()
            .map(|guard| *guard)
            .map_err(|_| {
                ExecutionError::RuntimeError("Failed to read transaction state".to_string())
            })
    }

    /// Check if there's an active transaction
    pub fn has_active_transaction(&self) -> Result<bool, ExecutionError> {
        Ok(self.current_transaction_id()?.is_some())
    }

    /// Check if auto-commit is enabled
    pub fn is_auto_commit(&self) -> Result<bool, ExecutionError> {
        self.auto_commit.read().map(|guard| *guard).map_err(|_| {
            ExecutionError::RuntimeError("Failed to read auto-commit state".to_string())
        })
    }

    /// Set auto-commit mode
    pub fn set_auto_commit(&self, enabled: bool) -> Result<(), ExecutionError> {
        let mut auto_commit = self.auto_commit.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update auto-commit state".to_string())
        })?;
        *auto_commit = enabled;
        Ok(())
    }

    /// Get the isolation level
    pub fn isolation_level(&self) -> Result<IsolationLevel, ExecutionError> {
        self.isolation_level
            .read()
            .map(|guard| *guard)
            .map_err(|_| ExecutionError::RuntimeError("Failed to read isolation level".to_string()))
    }

    /// Set the isolation level
    pub fn set_isolation_level(&self, level: IsolationLevel) -> Result<(), ExecutionError> {
        let mut isolation = self.isolation_level.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update isolation level".to_string())
        })?;
        *isolation = level;
        Ok(())
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self) -> Result<TransactionId, ExecutionError> {
        // Check if there's already an active transaction
        if self.has_active_transaction()? {
            return Err(ExecutionError::RuntimeError(
                "Transaction already in progress".to_string(),
            ));
        }

        // Start a new transaction
        let isolation = self.isolation_level()?;
        let txn_id = self.manager.start_transaction(Some(isolation), None)?;

        // Set as current transaction
        let mut current = self.current_transaction.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction state".to_string())
        })?;
        *current = Some(txn_id);

        // Initialize transaction log
        let mut logs = self.transaction_logs.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction logs".to_string())
        })?;
        logs.insert(txn_id, TransactionLog::new(txn_id));

        log::info!("Session began transaction: {:?}", txn_id);
        Ok(txn_id)
    }

    /// Commit the current transaction
    pub fn commit_transaction(&self) -> Result<(), ExecutionError> {
        let txn_id = self.current_transaction_id()?.ok_or_else(|| {
            ExecutionError::RuntimeError("No active transaction to commit".to_string())
        })?;

        // Commit the transaction
        self.manager.commit_transaction(txn_id)?;

        // Clear current transaction
        let mut current = self.current_transaction.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction state".to_string())
        })?;
        *current = None;

        // Clear transaction log
        let mut logs = self.transaction_logs.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction logs".to_string())
        })?;
        logs.remove(&txn_id);

        log::info!("Session committed transaction: {:?}", txn_id);
        Ok(())
    }

    /// Rollback the current transaction
    pub fn rollback_transaction(&self) -> Result<(), ExecutionError> {
        self.rollback_transaction_with_storage(None)
    }

    /// Rollback the current transaction with optional storage manager for applying undo operations
    pub fn rollback_transaction_with_storage(
        &self,
        storage: Option<&Arc<crate::storage::StorageManager>>,
    ) -> Result<(), ExecutionError> {
        log::info!("ROLLBACK: Attempting rollback, checking for active transaction...");
        let current_txn = self.current_transaction_id()?;

        let txn_id = current_txn.ok_or_else(|| {
            ExecutionError::RuntimeError("No active transaction to rollback".to_string())
        })?;

        // Get the transaction log for rollback operations
        let logs = self.transaction_logs.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to read transaction logs".to_string())
        })?;

        let log = logs
            .get(&txn_id)
            .ok_or_else(|| ExecutionError::RuntimeError("Transaction log not found".to_string()))?;

        // Collect undo operations before dropping the read lock
        let undo_operations: Vec<crate::txn::UndoOperation> = log.undo_operations.clone();
        drop(logs);

        // Apply undo operations in reverse order if storage is provided
        if let Some(storage_manager) = storage {
            log::info!(
                "Applying {} undo operations for transaction {:?}",
                undo_operations.len(),
                txn_id
            );
            for operation in undo_operations.iter().rev() {
                if let Err(e) = self.apply_undo_operation(operation, storage_manager) {
                    log::error!(
                        "Failed to apply undo operation: {:?}. Error: {}",
                        operation,
                        e
                    );
                    // Continue with other undo operations even if one fails
                }
            }
        } else {
            log::warn!(
                "No storage manager provided for rollback - undo operations will not be applied!"
            );
        }

        // Execute rollback in transaction manager (for WAL logging)
        self.manager.rollback_transaction(txn_id)?;

        // Clear current transaction
        let mut current = self.current_transaction.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction state".to_string())
        })?;
        *current = None;

        // Clear transaction log
        let mut logs = self.transaction_logs.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to update transaction logs".to_string())
        })?;
        logs.remove(&txn_id);

        log::info!("Session rolled back transaction: {:?}", txn_id);
        Ok(())
    }

    /// Apply a single undo operation to rollback changes
    fn apply_undo_operation(
        &self,
        operation: &crate::txn::UndoOperation,
        storage: &Arc<crate::storage::StorageManager>,
    ) -> Result<(), ExecutionError> {
        use crate::txn::UndoOperation;

        match operation {
            UndoOperation::Batch { operations } => {
                // Handle batch operations - undo all operations in the batch atomically
                log::info!(
                    "ROLLBACK: Processing batch with {} operations",
                    operations.len()
                );
                for (i, op) in operations.iter().enumerate() {
                    log::debug!(
                        "ROLLBACK: Applying batch operation {}/{}",
                        i + 1,
                        operations.len()
                    );
                    self.apply_undo_operation(op, storage)?;
                }
                log::info!(
                    "ROLLBACK: Successfully processed batch of {} operations",
                    operations.len()
                );
            }
            UndoOperation::InsertNode {
                graph_path,
                node_id,
            } => {
                log::info!(
                    "ROLLBACK: Undoing InsertNode: graph={}, node={}",
                    graph_path,
                    node_id
                );

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                log::info!(
                    "ROLLBACK: Graph loaded, current node count: {}",
                    graph.node_count().unwrap_or(0)
                );

                // Remove the inserted node
                match graph.remove_node(node_id) {
                    Ok(_) => {
                        log::info!("ROLLBACK: Successfully removed node {} from graph", node_id);
                    }
                    Err(e) => {
                        log::error!(
                            "ROLLBACK: Failed to remove node {} during rollback: {}",
                            node_id,
                            e
                        );
                        return Err(ExecutionError::StorageError(format!(
                            "Failed to remove node during rollback: {}",
                            e
                        )));
                    }
                }

                log::info!(
                    "ROLLBACK: After removal, node count: {}",
                    graph.node_count().unwrap_or(0)
                );

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("ROLLBACK: Successfully rolled back InsertNode: {}", node_id);
            }
            UndoOperation::DeleteNode {
                graph_path,
                node_id: _,
                deleted_node,
            } => {
                log::debug!(
                    "Undoing DeleteNode: graph={}, node={}",
                    graph_path,
                    deleted_node.id
                );

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                // Restore the deleted node
                if let Err(e) = graph.add_node(deleted_node.clone()) {
                    log::warn!(
                        "Failed to restore node {} during rollback: {}",
                        deleted_node.id,
                        e
                    );
                }

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("Rolled back DeleteNode: {}", deleted_node.id);
            }
            UndoOperation::UpdateNode {
                graph_path,
                node_id,
                old_properties,
                old_labels,
            } => {
                log::debug!("Undoing UpdateNode: graph={}, node={}", graph_path, node_id);

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                // Find and update the node
                if let Some(node) = graph.get_node_mut(node_id) {
                    node.properties = old_properties.clone();
                    node.labels = old_labels.clone();
                }

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("Rolled back UpdateNode: {}", node_id);
            }
            UndoOperation::InsertEdge {
                graph_path,
                edge_id,
            } => {
                log::debug!("Undoing InsertEdge: graph={}, edge={}", graph_path, edge_id);

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                // Remove the inserted edge
                if let Err(e) = graph.remove_edge(edge_id) {
                    log::warn!("Failed to remove edge {} during rollback: {}", edge_id, e);
                }

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("Rolled back InsertEdge: {}", edge_id);
            }
            UndoOperation::DeleteEdge {
                graph_path,
                edge_id: _,
                deleted_edge,
            } => {
                log::debug!(
                    "Undoing DeleteEdge: graph={}, edge={}",
                    graph_path,
                    deleted_edge.id
                );

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                // Restore the deleted edge
                if let Err(e) = graph.add_edge(deleted_edge.clone()) {
                    log::warn!(
                        "Failed to restore edge {} during rollback: {}",
                        deleted_edge.id,
                        e
                    );
                }

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("Rolled back DeleteEdge: {}", deleted_edge.id);
            }
            UndoOperation::UpdateEdge {
                graph_path,
                edge_id,
                old_properties,
                old_label,
            } => {
                log::debug!("Undoing UpdateEdge: graph={}, edge={}", graph_path, edge_id);

                // Load the graph
                let mut graph = storage
                    .get_graph(graph_path)
                    .map_err(|e| {
                        ExecutionError::StorageError(format!(
                            "Failed to load graph for rollback: {}",
                            e
                        ))
                    })?
                    .ok_or_else(|| {
                        ExecutionError::StorageError(format!(
                            "Graph not found for rollback: {}",
                            graph_path
                        ))
                    })?;

                // Find and update the edge
                if let Some(edge) = graph.get_edge_mut(edge_id) {
                    edge.properties = old_properties.clone();
                    edge.label = old_label.clone();
                }

                // Save the updated graph
                storage.save_graph(graph_path, graph).map_err(|e| {
                    ExecutionError::StorageError(format!(
                        "Failed to save graph after rollback: {}",
                        e
                    ))
                })?;

                log::info!("Rolled back UpdateEdge: {}", edge_id);
            }
        }

        Ok(())
    }

    /// Log an operation to the WAL if there's an active transaction
    pub fn log_operation_to_wal(
        &self,
        operation_type: crate::txn::state::OperationType,
        description: String,
    ) -> Result<(), ExecutionError> {
        if let Some(txn_id) = self.current_transaction_id()? {
            // Log the operation to WAL through transaction manager
            self.manager
                .log_operation(txn_id, operation_type, description)?;
        }
        // If no active transaction, don't log to WAL (auto-commit mode)
        Ok(())
    }

    /// Log a transaction operation (undo operation) for rollback
    pub fn log_transaction_operation(
        &self,
        operation: UndoOperation,
    ) -> Result<(), ExecutionError> {
        if let Some(txn_id) = self.current_transaction_id()? {
            log::info!(
                "TRANSACTION: Logging undo operation for transaction: {:?}, operation: {:?}",
                txn_id,
                operation
            );

            let mut logs = self.transaction_logs.write().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction logs lock".to_string())
            })?;

            // Get or create log for this transaction
            let log = logs
                .entry(txn_id)
                .or_insert_with(|| TransactionLog::new(txn_id));

            // Add the operation to the log
            log.undo_operations.push(operation);

            log::info!(
                "TRANSACTION: Transaction {} now has {} undo operations",
                txn_id,
                log.undo_operations.len()
            );
        } else {
            log::debug!("TRANSACTION: No active transaction, undo operation not logged");
        }

        Ok(())
    }

    /// Execute a function with auto-commit wrapping if needed
    pub fn execute_with_auto_commit<F, R>(&self, f: F) -> Result<R, ExecutionError>
    where
        F: FnOnce() -> Result<R, ExecutionError>,
    {
        let needs_auto_commit = self.is_auto_commit()? && !self.has_active_transaction()?;

        if needs_auto_commit {
            // Start auto-commit transaction
            let txn_id = self.begin_transaction()?;

            // Execute the function
            match f() {
                Ok(result) => {
                    // Commit on success
                    self.commit_transaction()?;
                    Ok(result)
                }
                Err(e) => {
                    // Rollback on error
                    if let Err(rollback_err) = self.rollback_transaction() {
                        log::error!(
                            "Failed to rollback transaction {}: {}",
                            txn_id,
                            rollback_err
                        );
                    }
                    Err(e)
                }
            }
        } else {
            // No auto-commit needed, just execute
            f()
        }
    }
}
