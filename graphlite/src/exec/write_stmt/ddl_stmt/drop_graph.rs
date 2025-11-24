// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::ast::DropGraphStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for DROP GRAPH statements
pub struct DropGraphExecutor {
    statement: DropGraphStatement,
}

impl DropGraphExecutor {
    /// Create a new DropGraphExecutor
    pub fn new(statement: DropGraphStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for DropGraphExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropGraph
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        // Use the graph path from the statement rather than context
        let graph_path = if self.statement.graph_path.segments.len() >= 2 {
            format!("/{}", self.statement.graph_path.segments.join("/"))
        } else {
            self.statement
                .graph_path
                .segments
                .last()
                .map_or("unknown".to_string(), |s| s.clone())
        };

        if self.statement.cascade {
            format!("DROP GRAPH {} CASCADE", graph_path)
        } else {
            format!("DROP GRAPH {}", graph_path)
        }
    }
}

impl DDLStatementExecutor for DropGraphExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Extract schema name and graph name from the AST CatalogPath, supporting relative paths
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
                            "Cannot drop graph with relative path: no current schema set. Use 'SESSION SET SCHEMA schema_name' or provide full path '/schema_name/graph_name'".to_string()
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

        let full_path = format!("/{}/{}", schema_name, graph_name);

        // Step 1: Delete actual graph data from storage FIRST
        // This ensures transactional consistency - if storage deletion fails,
        // we don't remove metadata and pretend the graph was dropped
        storage.delete_graph(&full_path).map_err(|e| {
            ExecutionError::StorageError(format!(
                "Failed to delete graph data for '{}': {}",
                full_path, e
            ))
        })?;

        // Step 2: Remove from catalog metadata only after storage deletion succeeds
        // Use the qualified key for proper schema-aware deletion
        let qualified_graph_name = format!("{}/{}", schema_name, graph_name);
        let drop_op = CatalogOperation::Drop {
            entity_type: EntityType::Graph,
            name: qualified_graph_name.clone(),
            cascade: self.statement.cascade,
        };

        let drop_result = catalog_manager.execute("graph_metadata", drop_op);
        match drop_result {
            Ok(response) => {
                match response {
                    crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                        // Persist the catalog after successful drop
                        let persist_result = catalog_manager.persist_catalog("graph_metadata");
                        if let Err(e) = persist_result {
                            log::error!("Failed to persist graph_metadata catalog: {}", e);
                        }

                        // CRITICAL FIX: Invalidate sessions using the dropped graph
                        // This prevents stale data from being returned after graph deletion
                        if let Some(session_manager) = crate::session::get_session_manager() {
                            let sessions_invalidated =
                                session_manager.invalidate_sessions_for_graph(&full_path);
                            log::info!(
                                "Invalidated {} sessions using dropped graph '{}'",
                                sessions_invalidated,
                                full_path
                            );
                        } else {
                            log::warn!("No session manager available for session invalidation after dropping graph '{}'", full_path);
                        }

                        // TODO: Cache invalidation for query results, plans, and subqueries
                        // Currently there's no global CacheManager accessible here
                        // This needs architectural changes to provide cache manager access
                        // For now, the storage layer deletion and session invalidation
                        // should prevent most stale data issues
                        log::warn!("Cache invalidation for query/plan/subquery caches not yet implemented for DROP GRAPH");

                        let message = if self.statement.cascade {
                            format!("Graph '{}' dropped (cascade)", full_path)
                        } else {
                            format!("Graph '{}' dropped", full_path)
                        };

                        Ok((message, 1))
                    }
                    crate::catalog::operations::CatalogResponse::Error { message } => {
                        Err(ExecutionError::CatalogError(format!(
                            "Failed to drop graph '{}': {}",
                            full_path, message
                        )))
                    }
                    _ => Err(ExecutionError::CatalogError(
                        "Unexpected response from graph_metadata catalog".to_string(),
                    )),
                }
            }
            Err(e) => Err(ExecutionError::CatalogError(format!(
                "Failed to drop graph: {}",
                e
            ))),
        }
    }
}
