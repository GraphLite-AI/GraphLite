// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// DROP GRAPH TYPE executor implementation

use serde_json;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::schema::parser::ast::DropGraphTypeStatement;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for DROP GRAPH TYPE statements
pub struct DropGraphTypeExecutor {
    statement: DropGraphTypeStatement,
}

impl DropGraphTypeExecutor {
    /// Create a new DROP GRAPH TYPE executor
    #[allow(dead_code)] // ROADMAP v0.4.0 - Constructor for DROP GRAPH TYPE executor (module disabled in mod.rs)
    pub fn new(statement: DropGraphTypeStatement) -> Self {
        Self { statement }
    }

    /// Check if any graphs are using this graph type
    fn check_graph_type_usage(
        &self,
        catalog_manager: &CatalogManager,
    ) -> Result<Vec<String>, ExecutionError> {
        // Query the graph catalog to find graphs using this type
        let query_result = catalog_manager.query_read_only(
            "graph",
            QueryType::List,
            serde_json::json!({}),
        );

        let mut using_graphs = Vec::new();

        if let Ok(CatalogResponse::List { items, .. }) = query_result {
            for item in items {
                if let Some(graph_type) = item.get("graph_type").and_then(|v| v.as_str()) {
                    if graph_type == self.statement.name {
                        if let Some(graph_name) = item.get("name").and_then(|v| v.as_str()) {
                            using_graphs.push(graph_name.to_string());
                        }
                    }
                }
            }
        }

        Ok(using_graphs)
    }

    /// Check if the graph type exists
    fn check_graph_type_exists(
        &self,
        catalog_manager: &CatalogManager,
    ) -> Result<bool, ExecutionError> {
        let query_result = catalog_manager.query_read_only(
            "graph_type",
            QueryType::Exists,
            serde_json::json!({ "name": self.statement.name }),
        );

        match query_result {
            Ok(CatalogResponse::Success { data: Some(data) }) => {
                Ok(data.get("exists")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false))
            }
            Err(_) => Ok(false),
            _ => Ok(false),
        }
    }
}

impl StatementExecutor for DropGraphTypeExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropTable // TODO: Add DropGraphType to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("DROP GRAPH TYPE {}", self.statement.name)
    }
}

impl DDLStatementExecutor for DropGraphTypeExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Check if the graph_type catalog exists
        if !catalog_manager.has_catalog("graph_type") {
            if self.statement.if_exists {
                return Ok((
                    format!("Graph type '{}' does not exist, skipping drop", self.statement.name),
                    0
                ));
            } else {
                return Err(ExecutionError::NotFound(
                    format!("Graph type '{}' does not exist", self.statement.name)
                ));
            }
        }

        // Check if graph type exists
        if !self.check_graph_type_exists(catalog_manager)? {
            if self.statement.if_exists {
                return Ok((
                    format!("Graph type '{}' does not exist, skipping drop", self.statement.name),
                    0
                ));
            } else {
                return Err(ExecutionError::NotFound(
                    format!("Graph type '{}' does not exist", self.statement.name)
                ));
            }
        }

        // Check if any graphs are using this graph type
        let using_graphs = self.check_graph_type_usage(catalog_manager)?;
        if !using_graphs.is_empty() && !self.statement.cascade {
            return Err(ExecutionError::ValidationError(
                format!(
                    "Cannot drop graph type '{}' because it is referenced by graphs: {}. \
                     Use CASCADE to drop the graph type and all dependent objects.",
                    self.statement.name,
                    using_graphs.join(", ")
                )
            ));
        }

        // If CASCADE is specified and there are dependent graphs, we should handle them
        if self.statement.cascade && !using_graphs.is_empty() {
            // TODO: Implement cascading deletion of graphs
            // For now, we'll just warn about it
            log::warn!(
                "CASCADE drop of graph type '{}' would affect graphs: {}",
                self.statement.name,
                using_graphs.join(", ")
            );
        }

        // Drop the graph type
        let response = catalog_manager.execute(
            "graph_type",
            CatalogOperation::Drop {
                entity_type: EntityType::GraphType,
                name: self.statement.name.clone(),
                cascade: self.statement.cascade,
            }
        ).map_err(|e| ExecutionError::RuntimeError(
            format!("Failed to drop graph type: {}", e)
        ))?;

        match response {
            CatalogResponse::Success { data } => {
                let name = data.as_ref()
                    .and_then(|d| d.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or(&self.statement.name);
                Ok((
                    format!("Graph type '{}' dropped successfully", name),
                    1
                ))
            }
            CatalogResponse::Error { message } => {
                Err(ExecutionError::RuntimeError(message))
            }
            _ => Err(ExecutionError::RuntimeError(
                "Unexpected response from catalog".to_string()
            ))
        }
    }
}
