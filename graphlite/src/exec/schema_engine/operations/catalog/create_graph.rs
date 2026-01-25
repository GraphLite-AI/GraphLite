// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::CreateGraphStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::exec::schema_engine::operations::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use log::{info, warn};

/// Executor for CREATE GRAPH statements
pub struct CreateGraphExecutor {
    statement: CreateGraphStatement,
}

impl CreateGraphExecutor {
    /// Create a new CreateGraphExecutor
    pub fn new(statement: CreateGraphStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for CreateGraphExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateGraph
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        if self.statement.or_replace {
            format!("CREATE OR REPLACE GRAPH {}", graph_name)
        } else if self.statement.if_not_exists {
            format!("CREATE GRAPH IF NOT EXISTS {}", graph_name)
        } else {
            format!("CREATE GRAPH {}", graph_name)
        }
    }
}

impl DDLStatementExecutor for CreateGraphExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Extract schema name and graph name from the AST CatalogPath
        let (schema_name, graph_name) = match self.statement.graph_path.segments.len() {
            2 => {
                // Full path provided: /schema_name/graph_name
                (
                    self.statement.graph_path.segments[0].clone(),
                    self.statement.graph_path.segments[1].clone(),
                )
            }
            1 => {
                // Relative path: graph_name only - use session schema
                let graph_name = self.statement.graph_path.segments[0].clone();
                match context.get_current_schema() {
                    Some(session_schema) => {
                        // Strip leading slash if present (session schemas are stored with leading slash)
                        let schema_name = session_schema
                            .strip_prefix('/')
                            .unwrap_or(&session_schema)
                            .to_string();
                        (schema_name, graph_name)
                    }
                    None => {
                        return Err(ExecutionError::RuntimeError(
                            "Cannot create graph with relative path: no current schema set. Use 'SESSION SET SCHEMA schema_name' or provide full path '/schema_name/graph_name'".to_string()
                        ));
                    }
                }
            }
            _ => {
                return Err(ExecutionError::RuntimeError(
                    "Invalid graph path: must specify either graph name (when schema is set) or full path /schema_name/graph_name".to_string()
                ));
            }
        };
        let full_graph_path = format!("/{}/{}", schema_name, graph_name);

        // Step 1: Validate that the schema exists
        let schema_query_op = CatalogOperation::Query {
            query_type: crate::catalog::operations::QueryType::Get,
            params: serde_json::json!({ "name": schema_name }),
        };

        let schema_result = catalog_manager.execute("schema", schema_query_op);
        match schema_result {
            Ok(crate::catalog::operations::CatalogResponse::Query { results }) => {
                if results.is_null() {
                    return Err(ExecutionError::CatalogError(format!(
                        "Schema '{}' not found",
                        schema_name
                    )));
                }
            }
            Ok(crate::catalog::operations::CatalogResponse::Error { message }) => {
                return Err(ExecutionError::CatalogError(format!(
                    "Failed to validate schema '{}': {}",
                    schema_name, message
                )));
            }
            Err(e) => {
                return Err(ExecutionError::CatalogError(format!(
                    "Failed to validate schema '{}': {}",
                    schema_name, e
                )));
            }
            _ => {
                return Err(ExecutionError::CatalogError(format!(
                    "Unexpected response when validating schema '{}'",
                    schema_name
                )));
            }
        }

        // Step 2: Validate graph type if specified
        let graph_type_name: Option<String> = if let Some(_spec) = &self.statement.graph_type_spec {
            // For now, we'll use a default name. In a full implementation,
            // this would be parsed from the spec or provided as a reference
            let type_name = "DefaultGraphType"; // This should come from spec

            // Validate that the graph type exists
            let graph_type_query = CatalogOperation::Query {
                query_type: QueryType::GetGraphType,
                params: serde_json::json!({ "name": type_name }),
            };

            match catalog_manager.execute("graph_type", graph_type_query) {
                Ok(CatalogResponse::Success { data: Some(_) }) => {
                    info!(
                        "Using graph type '{}' for new graph '{}'",
                        type_name, graph_name
                    );
                    Some(type_name.to_string())
                }
                Ok(_) | Err(_) => {
                    // Graph type not found
                    let enforcement_mode = context
                        .get_variable("schema_enforcement_mode")
                        .and_then(|v| v.as_string().map(|s| s.to_string()))
                        .unwrap_or_else(|| "advisory".to_string());

                    match enforcement_mode.as_str() {
                        "strict" => {
                            return Err(ExecutionError::SchemaValidation(format!(
                                "Graph type '{}' not found for CREATE GRAPH",
                                type_name
                            )));
                        }
                        "advisory" => {
                            warn!(
                                "Graph type '{}' not found, creating untyped graph",
                                type_name
                            );
                            None
                        }
                        _ => None,
                    }
                }
            }
        } else {
            None
        };

        // Step 3: Create catalog entry first (DDL operations typically create metadata first)
        let create_op = CatalogOperation::Create {
            entity_type: EntityType::Graph,
            name: graph_name.clone(),
            params: serde_json::json!({
                "graph_path": full_graph_path,
                "schema_name": schema_name,
                "graph_type": graph_type_name,
                "if_not_exists": self.statement.if_not_exists,
                "or_replace": self.statement.or_replace,
                "description": None::<String>
            }),
        };

        let create_result = catalog_manager.execute("graph_metadata", create_op);
        match create_result {
            Ok(response) => {
                match response {
                    crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                        // Step 2: Create the graph in storage after successful catalog creation
                        // Create empty graph in storage after successful catalog creation
                        let empty_graph = crate::storage::GraphCache::new();
                        if let Err(e) = storage.save_graph(&full_graph_path, empty_graph) {
                            log::error!("Failed to create graph in storage: {}", e);
                            // TODO: Rollback catalog operation if storage creation fails
                            return Err(ExecutionError::StorageError(format!(
                                "Failed to create graph '{}' in storage: {}",
                                full_graph_path, e
                            )));
                        }

                        // Step 3: Persist the catalog after successful creation
                        let persist_result = catalog_manager.persist_catalog("graph_metadata");
                        if let Err(e) = persist_result {
                            log::error!("Failed to persist graph_metadata catalog: {}", e);
                            // Don't fail the operation, just log the error
                        } else {
                            log::info!("Successfully persisted graph_metadata catalog after creating graph '{}'", graph_name);
                        }

                        // Invalidate catalog cache - graph list has changed
                        if let Some(cache_mgr) = &context.cache_manager {
                            cache_mgr.invalidate_on_data_change(Some(graph_name.clone()), 1);
                            log::debug!(
                                "Invalidated catalog cache after CREATE GRAPH '{}'",
                                graph_name
                            );
                        }

                        let message = if self.statement.if_not_exists {
                            format!("Graph '{}' created (if not exists)", graph_name)
                        } else if self.statement.or_replace {
                            format!("Graph '{}' created (or replace)", graph_name)
                        } else {
                            format!("Graph '{}' created", graph_name)
                        };

                        Ok((message, 1))
                    }
                    crate::catalog::operations::CatalogResponse::Error { message } => {
                        if message == "Already exists" && self.statement.if_not_exists {
                            // This is expected behavior for IF NOT EXISTS
                            let message =
                                format!("Graph '{}' already exists (if not exists)", graph_name);
                            Ok((message, 0))
                        } else if message == "Already exists" {
                            Err(ExecutionError::CatalogError(format!(
                                "Graph '{}' already exists",
                                graph_name
                            )))
                        } else {
                            Err(ExecutionError::CatalogError(format!(
                                "Failed to create graph '{}': {}",
                                graph_name, message
                            )))
                        }
                    }
                    _ => Err(ExecutionError::CatalogError(
                        "Unexpected response from graph_metadata catalog".to_string(),
                    )),
                }
            }
            Err(e) => Err(ExecutionError::CatalogError(format!(
                "Failed to create graph: {}",
                e
            ))),
        }
    }
}
