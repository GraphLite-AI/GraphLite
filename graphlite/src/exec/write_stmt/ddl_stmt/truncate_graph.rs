// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::CatalogPath;
use crate::ast::ast::TruncateGraphStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for TRUNCATE GRAPH statements
pub struct TruncateGraphExecutor {
    statement: TruncateGraphStatement,
}

impl TruncateGraphExecutor {
    /// Create a new TruncateGraphExecutor
    pub fn new(statement: TruncateGraphStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for TruncateGraphExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Other // TODO: Add TruncateGraph to OperationType enum
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("TRUNCATE GRAPH {}", graph_name)
    }
}

impl DDLStatementExecutor for TruncateGraphExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let catalog_path = CatalogPath::new(
            self.statement.graph_path.segments.clone(),
            crate::ast::ast::Location {
                line: 0,
                column: 0,
                offset: 0,
            },
        );
        // Use full path format for storage operations (must be /<schema>/<graph>)
        let full_path = if self.statement.graph_path.segments.len() >= 2 {
            format!("/{}", self.statement.graph_path.segments.join("/"))
        } else {
            return Err(ExecutionError::RuntimeError(
                format!("Graph path '{}' is not in full path format. Use /<schema-name>/<graph-name> format.", 
                       self.statement.graph_path.segments.join("/"))
            ));
        };

        // Step 1: Clear all graph data from storage FIRST
        // TRUNCATE removes all nodes and edges but keeps the graph schema
        let empty_graph = crate::storage::GraphCache::new();
        storage.save_graph(&full_path, empty_graph).map_err(|e| {
            ExecutionError::StorageError(format!(
                "Failed to truncate graph data for '{}': {}",
                full_path, e
            ))
        })?;

        // Step 2: Update catalog metadata if needed (TRUNCATE typically doesn't change metadata)
        // Use CatalogManager to handle truncate operation
        let simple_graph_name = catalog_path
            .name()
            .map_or("unknown".to_string(), |v| v.clone());
        let truncate_op = CatalogOperation::Update {
            entity_type: EntityType::Graph,
            name: simple_graph_name.clone(),
            updates: serde_json::json!({"operation": "truncate"}),
        };

        let truncate_result = catalog_manager.execute("graph_metadata", truncate_op);
        match truncate_result {
            Ok(response) => {
                match response {
                    crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                        // Persist the catalog after successful truncate
                        let persist_result = catalog_manager.persist_catalog("graph_metadata");
                        if let Err(e) = persist_result {
                            log::error!("Failed to persist graph_metadata catalog: {}", e);
                        }

                        let message = format!("Graph '{}' truncated", full_path);
                        Ok((message, 1))
                    }
                    crate::catalog::operations::CatalogResponse::Error { message } => {
                        Err(ExecutionError::CatalogError(format!(
                            "Failed to truncate graph '{}': {}",
                            full_path, message
                        )))
                    }
                    _ => Err(ExecutionError::CatalogError(format!(
                        "Unexpected response from graph_metadata catalog"
                    ))),
                }
            }
            Err(e) => Err(ExecutionError::CatalogError(format!(
                "Failed to truncate graph: {}",
                e
            ))),
        }
    }
}
