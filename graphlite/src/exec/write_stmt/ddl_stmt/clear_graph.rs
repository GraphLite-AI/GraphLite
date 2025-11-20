// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::ClearGraphStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for CLEAR GRAPH statements
pub struct ClearGraphExecutor {
    statement: ClearGraphStatement,
}

impl ClearGraphExecutor {
    /// Create a new ClearGraphExecutor
    pub fn new(statement: ClearGraphStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for ClearGraphExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Other // TODO: Add ClearGraph to OperationType enum
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = if let Some(ref catalog_path) = self.statement.graph_path {
            catalog_path
                .segments
                .last()
                .map_or("unknown".to_string(), |v| v.clone())
        } else {
            context
                .get_graph_name()
                .unwrap_or_else(|_| "current_session".to_string())
        };
        format!("CLEAR GRAPH {}", graph_name)
    }
}

impl DDLStatementExecutor for ClearGraphExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let full_path = if let Some(ref catalog_path) = self.statement.graph_path {
            // Explicit graph path provided - must be in full path format
            if catalog_path.segments.len() >= 2 {
                format!("/{}", catalog_path.segments.join("/"))
            } else {
                return Err(ExecutionError::RuntimeError(
                    format!("Graph path '{}' is not in full path format. Use /<schema-name>/<graph-name> format.", 
                           catalog_path.segments.join("/"))
                ));
            }
        } else {
            // No graph path - use current session graph (already validated by context)
            context.get_graph_name()?
        };

        // Step 1: Clear all graph data from storage FIRST
        // CLEAR removes all nodes and edges but keeps the graph and its schema
        let empty_graph = crate::storage::GraphCache::new();
        storage.save_graph(&full_path, empty_graph).map_err(|e| {
            ExecutionError::StorageError(format!(
                "Failed to clear graph data for '{}': {}",
                full_path, e
            ))
        })?;

        // Step 2: Update catalog metadata if needed (CLEAR typically doesn't change metadata)
        // Use CatalogManager to handle clear operation
        let simple_graph_name = if let Some(ref catalog_path) = self.statement.graph_path {
            catalog_path
                .segments
                .last()
                .map_or("unknown".to_string(), |v| v.clone())
        } else {
            "session_graph".to_string()
        };
        let clear_op = CatalogOperation::Update {
            entity_type: EntityType::Graph,
            name: simple_graph_name.clone(),
            updates: serde_json::json!({"operation": "clear"}),
        };

        let clear_result = catalog_manager.execute("graph_metadata", clear_op);
        match clear_result {
            Ok(response) => {
                match response {
                    crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                        // Persist the catalog after successful clear
                        let persist_result = catalog_manager.persist_catalog("graph_metadata");
                        if let Err(e) = persist_result {
                            log::error!("Failed to persist graph_metadata catalog: {}", e);
                        }

                        let message = format!("Graph '{}' cleared", full_path);
                        Ok((message, 1))
                    }
                    crate::catalog::operations::CatalogResponse::Error { message } => {
                        Err(ExecutionError::CatalogError(format!(
                            "Failed to clear graph '{}': {}",
                            full_path, message
                        )))
                    }
                    _ => Err(ExecutionError::CatalogError(format!(
                        "Unexpected response from graph_metadata catalog"
                    ))),
                }
            }
            Err(e) => Err(ExecutionError::CatalogError(format!(
                "Failed to clear graph: {}",
                e
            ))),
        }
    }
}
