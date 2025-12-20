// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// TODO: Implement DropSchemaExecutor following the same pattern as DropGraphExecutor
use crate::ast::DropSchemaStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::schema_engine::operations::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

pub struct DropSchemaExecutor {
    statement: DropSchemaStatement,
}

impl DropSchemaExecutor {
    pub fn new(statement: DropSchemaStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for DropSchemaExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropTable // TODO: Add DropSchema to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!(
            "DROP SCHEMA {}",
            self.statement.schema_path.name().map_or("unknown", |v| v)
        )
    }
}

impl DDLStatementExecutor for DropSchemaExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let schema_name = self
            .statement
            .schema_path
            .name()
            .map_or("unknown".to_string(), |v| v.clone());

        let drop_op = CatalogOperation::Drop {
            entity_type: EntityType::Schema,
            name: schema_name.clone(),
            cascade: self.statement.cascade,
        };

        // Check for dependent objects based on CASCADE vs RESTRICT
        log::info!(
            "DROP SCHEMA: cascade={}, schema_name={}",
            self.statement.cascade,
            schema_name
        );

        // Get all graphs to check for dependencies
        let list_all_graphs_op = CatalogOperation::List {
            entity_type: EntityType::Graph,
            filters: None, // Get all graphs
        };

        let graphs_result = catalog_manager.execute("graph_metadata", list_all_graphs_op);
        let dependent_graphs = match graphs_result {
            Ok(response) => {
                if let crate::catalog::operations::CatalogResponse::List { items } = response {
                    // Filter graphs that belong to this schema
                    items
                        .iter()
                        .filter_map(|graph_entry| {
                            let graph_schema = graph_entry
                                .get("id")
                                .and_then(|id_obj| id_obj.get("schema_name"))
                                .and_then(|v| v.as_str());

                            let schema_matches =
                                graph_schema.map(|s| s == schema_name).unwrap_or(false);

                            if schema_matches {
                                graph_entry
                                    .get("id")
                                    .and_then(|id| id.get("name"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            }
            Err(e) => {
                log::error!("Failed to query graph catalog for dependency check: {}", e);
                Vec::new()
            }
        };

        // Handle CASCADE vs RESTRICT
        if self.statement.cascade {
            // CASCADE: Drop all dependent graphs first
            log::info!(
                "CASCADE: Starting dependency cleanup for schema '{}'",
                schema_name
            );
            for graph_name in &dependent_graphs {
                log::info!(
                    "CASCADE: Dropping dependent graph '{}' in schema '{}'",
                    graph_name,
                    schema_name
                );

                let drop_graph_op = CatalogOperation::Drop {
                    entity_type: EntityType::Graph,
                    name: graph_name.clone(),
                    cascade: true,
                };

                let drop_result = catalog_manager.execute("graph_metadata", drop_graph_op);
                if let Err(e) = drop_result {
                    log::error!("Failed to drop dependent graph '{}': {}", graph_name, e);
                } else {
                    log::info!("Successfully dropped graph '{}' during CASCADE", graph_name);
                }
            }
        } else {
            // RESTRICT: Fail if there are dependent objects
            if !dependent_graphs.is_empty() {
                let graph_list = dependent_graphs.join(", ");
                return Err(ExecutionError::CatalogError(format!(
                    "Cannot drop schema '{}': contains objects (graphs: {})",
                    schema_name, graph_list
                )));
            }
        }

        // Dependency handling complete - proceed with schema drop

        let drop_result = catalog_manager.execute("schema", drop_op);
        match drop_result {
            Ok(response) => {
                match response {
                    crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                        // Reset sessions that were using this schema
                        if let Some(session_provider) = &context.session_provider {
                            let sessions = session_provider.list_sessions();

                            for session_id in sessions {
                                if let Some(session_arc) = session_provider.get_session(&session_id)
                                {
                                    if let Ok(mut session) = session_arc.write() {
                                        let mut reset_session = false;

                                        // Reset if current schema matches dropped schema
                                        if let Some(current_schema) = &session.current_schema {
                                            if current_schema == &schema_name
                                                || current_schema == &format!("/{}", schema_name)
                                            {
                                                log::info!("Resetting session {} schema '{}' due to schema drop", session_id, current_schema);
                                                reset_session = true;
                                            }
                                        }

                                        // Reset if current graph is in the dropped schema
                                        if let Some(current_graph) = &session.current_graph {
                                            let graph_pattern = format!("/{}/", schema_name);
                                            if current_graph.starts_with(&graph_pattern) {
                                                log::info!("Resetting session {} graph '{}' due to schema drop", session_id, current_graph);
                                                reset_session = true;
                                            }
                                        }

                                        if reset_session {
                                            session.current_schema = None;
                                            session.current_graph = None;
                                        }
                                    }
                                }
                            }
                        }

                        // Persist both schema and graph catalogs after successful drop
                        let persist_result = catalog_manager.persist_catalog("schema");
                        if let Err(e) = persist_result {
                            log::error!("Failed to persist schema catalog: {}", e);
                        }
                        if self.statement.cascade {
                            let graph_persist_result =
                                catalog_manager.persist_catalog("graph_metadata");
                            if let Err(e) = graph_persist_result {
                                log::error!("Failed to persist graph catalog after CASCADE: {}", e);
                            }
                        }

                        // Invalidate catalog cache - schema and possibly graphs dropped
                        if let Some(cache_mgr) = &context.cache_manager {
                            cache_mgr.invalidate_on_schema_change(
                                schema_name.clone(),
                                "schema_dropped".to_string(),
                            );
                            if self.statement.cascade {
                                // Also invalidate graph cache since CASCADE drops dependent graphs
                                cache_mgr.invalidate_on_data_change(None, 0);
                            }
                            log::debug!("Invalidated catalog cache after DROP SCHEMA '{}' cascade={}", schema_name, self.statement.cascade);
                        }

                        let message = if self.statement.if_exists {
                            format!("Schema '{}' dropped (if exists)", schema_name)
                        } else {
                            format!("Schema '{}' dropped", schema_name)
                        };

                        Ok((message, 1))
                    }
                    crate::catalog::operations::CatalogResponse::Error { message }
                        if message.contains("not found") =>
                    {
                        if self.statement.if_exists {
                            let message =
                                format!("Schema '{}' does not exist (if exists)", schema_name);
                            Ok((message, 0))
                        } else {
                            Err(ExecutionError::CatalogError(format!(
                                "Schema '{}' does not exist",
                                schema_name
                            )))
                        }
                    }
                    crate::catalog::operations::CatalogResponse::Error { message } => {
                        Err(ExecutionError::CatalogError(format!(
                            "Failed to drop schema '{}': {}",
                            schema_name, message
                        )))
                    }
                    _ => Err(ExecutionError::CatalogError(
                        "Unexpected response from schema catalog".to_string(),
                    )),
                }
            }
            Err(e) => {
                if e.to_string().contains("not found") && self.statement.if_exists {
                    Ok((
                        format!("Schema '{}' does not exist (if exists)", schema_name),
                        0,
                    ))
                } else {
                    Err(ExecutionError::CatalogError(format!(
                        "Failed to drop schema: {}",
                        e
                    )))
                }
            }
        }
    }
}
