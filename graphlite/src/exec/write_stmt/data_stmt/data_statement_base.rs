// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::storage::GraphCache;
use crate::txn::UndoOperation;

/// Base trait for all data statement executors
pub trait DataStatementExecutor: StatementExecutor {
    /// Execute the data modification on the graph
    /// Returns an undo operation for transaction rollback and the number of affected rows
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError>;
    
    /// Main execution method - handles the complete data modification flow
    fn execute(&self, context: &mut ExecutionContext, storage: &crate::storage::StorageManager) -> Result<(String, usize), ExecutionError> {
        // Pre-execution: check permissions, log to WAL, get graph
        self.pre_execute(context)?;
        
        // Get the graph name
        let graph_name = context.get_graph_name()?;
        
        // Execute the unified data modification flow
        let rows_affected = self.execute_unified_flow(context, &graph_name, storage)?;
        
        // Post-execution: any cleanup
        self.post_execute(context, rows_affected)?;
        
        let message = self.operation_description(context);
        Ok((message, rows_affected))
    }
    
    
    /// Execute using the unified data modification flow
    fn execute_unified_flow(&self, context: &mut ExecutionContext, graph_name: &str, storage: &crate::storage::StorageManager) -> Result<usize, ExecutionError> {
        use std::sync::{Arc, Mutex};

        log::debug!("UNIFIED_FLOW: Starting for graph '{}'", graph_name);

        let rows_affected = Arc::new(Mutex::new(0usize));
        let rows_affected_clone = rows_affected.clone();

        // Step 1: Log operation to WAL FIRST (Write-Ahead Logging principle)
        let description = self.operation_description(context);
        log::debug!("WAL: Logged {} operation for graph '{}'", description, graph_name);

        // Step 2: Get the current graph for modification
        log::debug!("UNIFIED_FLOW: Getting graph '{}' from storage", graph_name);
        let mut graph = storage.get_graph(graph_name)
            .map_err(|e| {
                log::error!("UNIFIED_FLOW: Failed to get graph: {}", e);
                ExecutionError::StorageError(format!("Failed to get graph: {}", e))
            })?
            .ok_or_else(|| {
                log::error!("UNIFIED_FLOW: Graph '{}' not found", graph_name);
                ExecutionError::StorageError(format!("Graph not found: {}", graph_name))
            })?;

        log::debug!("UNIFIED_FLOW: Got graph with {} nodes", graph.node_count().unwrap_or(0));


        // Step 3: Execute the modification and get undo operation
        let (undo_op, affected) = self.execute_modification(&mut graph, context)?;
        *rows_affected_clone.lock().unwrap() = affected;
        log::debug!("Executed modification for graph '{}'", graph_name);

        // Step 4: Log undo operation for transaction rollback
        context.log_transaction_operation(undo_op)?;
        
        // Step 5: Update the graph in unified storage (this now automatically handles persistence)
        // StorageManager will save to persistent storage and update in-memory
        // Ensure we use the same graph name format as used by QueryExecutor for retrieval
        let normalized_graph_name = graph_name.to_string();
        
        
        
        storage.save_graph(&normalized_graph_name, graph)
            .map_err(|e| ExecutionError::StorageError(format!("Failed to update in-memory graph: {}", e)))?;
        
        log::debug!("MEMORY: Updated in-memory graph '{}' after persistence", graph_name);
        
        let affected = *rows_affected.lock().unwrap();
        Ok(affected)
    }
}

/// Enum for different data statement types
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Statement type classification for optimization and monitoring
pub enum DataStatementType {
    Insert,
    Set,
    Delete,
    Remove,
    MatchInsert,
}
