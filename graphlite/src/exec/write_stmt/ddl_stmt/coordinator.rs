// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::CatalogStatement;
use crate::catalog::manager::CatalogManager;
use crate::exec::write_stmt::ddl_stmt::*;
use crate::exec::write_stmt::ExecutionContext;
use crate::exec::{ExecutionError, QueryResult, Row};
use crate::schema::executor::alter_graph_type::AlterGraphTypeExecutor;
use crate::session::UserSession;
use crate::storage::StorageManager;
use std::sync::Arc;

/// Coordinator for DDL statement execution
pub struct DDLStatementCoordinator;

impl DDLStatementCoordinator {
    /// Execute a DDL statement using the unified executor pattern
    pub fn execute_ddl_statement(
        stmt: &CatalogStatement,
        storage: Arc<StorageManager>,
        catalog_manager: &mut CatalogManager,
        session: Option<&Arc<std::sync::RwLock<UserSession>>>,
        context: &mut ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        let start_time = std::time::Instant::now();

        // Use the provided execution context and update session ID if needed
        let session_id = if let Some(session) = session {
            let session_read = session.read().map_err(|e| {
                ExecutionError::RuntimeError(format!("Failed to read session: {}", e))
            })?;
            let session_id = session_read.session_id.clone();
            drop(session_read); // Release the read lock
            session_id
        } else {
            "default_session".to_string() // Default session ID for unit tests and non-session contexts
        };

        // Update the context's session ID to ensure consistency
        if context.session_id != session_id {
            context.session_id = session_id;
        }

        // Create the appropriate executor and execute with transaction support
        let result = match stmt {
            CatalogStatement::CreateSchema(create_schema) => {
                let stmt_executor = CreateSchemaExecutor::new(create_schema.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::DropSchema(drop_schema) => {
                let stmt_executor = DropSchemaExecutor::new(drop_schema.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::CreateGraph(create_graph) => {
                let stmt_executor = CreateGraphExecutor::new(create_graph.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::DropGraph(drop_graph) => {
                let stmt_executor = DropGraphExecutor::new(drop_graph.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::CreateGraphType(create_graph_type) => {
                let stmt_executor = CreateGraphTypeExecutor::new(create_graph_type.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::DropGraphType(drop_graph_type) => {
                let stmt_executor = DropGraphTypeExecutor::new(drop_graph_type.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::AlterGraphType(alter_graph_type) => {
                // Convert from ast::ast::AlterGraphTypeStatement to schema::parser::ast::AlterGraphTypeStatement
                let schema_stmt = crate::schema::parser::ast::AlterGraphTypeStatement {
                    name: alter_graph_type.name.clone(),
                    version: None,
                    changes: vec![],
                };
                let stmt_executor = AlterGraphTypeExecutor::new(schema_stmt);
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::TruncateGraph(truncate_graph) => {
                let stmt_executor = TruncateGraphExecutor::new(truncate_graph.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::ClearGraph(clear_graph) => {
                let stmt_executor = ClearGraphExecutor::new(clear_graph.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::CreateUser(create_user) => {
                let stmt_executor = CreateUserExecutor::new(create_user.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::DropUser(drop_user) => {
                let stmt_executor = DropUserExecutor::new(drop_user.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::CreateRole(create_role) => {
                let stmt_executor = CreateRoleExecutor::new(create_role.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::DropRole(drop_role) => {
                let stmt_executor = DropRoleExecutor::new(drop_role.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::GrantRole(grant_role) => {
                let stmt_executor = GrantRoleExecutor::new(grant_role.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::RevokeRole(revoke_role) => {
                let stmt_executor = RevokeRoleExecutor::new(revoke_role.clone());
                stmt_executor.execute(context, catalog_manager, &storage)
            }
            CatalogStatement::CreateProcedure(create_procedure) => {
                // Store procedure in catalog
                DDLStatementCoordinator::execute_create_procedure(
                    create_procedure,
                    context,
                    catalog_manager,
                    &storage,
                )
            }
            CatalogStatement::DropProcedure(drop_procedure) => {
                // Remove procedure from catalog
                DDLStatementCoordinator::execute_drop_procedure(
                    drop_procedure,
                    context,
                    catalog_manager,
                    &storage,
                )
            }
        };

        let execution_time = start_time.elapsed().as_millis() as u64;

        match result {
            Ok((message, rows_affected)) => {
                // Persist catalog changes to disk (SQLite-style pattern requires immediate persistence)
                // Only persist if not in a test environment (test fixtures use temporary directories)
                // Note: We can detect CLI usage vs TestFixture by checking if there's a way to distinguish
                // For now, always persist but with better error handling
                match stmt {
                    CatalogStatement::CreateSchema(_) | CatalogStatement::DropSchema(_) => {
                        match catalog_manager.persist_catalog("schema") {
                            Ok(_) => log::debug!("Schema catalog persisted successfully"),
                            Err(e) => {
                                log::warn!("Failed to persist schema catalog (non-fatal): {}", e)
                            }
                        }
                    }
                    CatalogStatement::CreateGraph(_)
                    | CatalogStatement::DropGraph(_)
                    | CatalogStatement::CreateGraphType(_)
                    | CatalogStatement::DropGraphType(_)
                    | CatalogStatement::AlterGraphType(_) => {
                        match catalog_manager.persist_catalog("graph_metadata") {
                            Ok(_) => log::debug!("Graph metadata catalog persisted successfully"),
                            Err(e) => log::warn!(
                                "Failed to persist graph_metadata catalog (non-fatal): {}",
                                e
                            ),
                        }
                    }
                    CatalogStatement::CreateUser(_)
                    | CatalogStatement::DropUser(_)
                    | CatalogStatement::CreateRole(_)
                    | CatalogStatement::DropRole(_)
                    | CatalogStatement::GrantRole(_)
                    | CatalogStatement::RevokeRole(_) => {
                        match catalog_manager.persist_catalog("security") {
                            Ok(_) => log::debug!("Security catalog persisted successfully"),
                            Err(e) => {
                                log::warn!("Failed to persist security catalog (non-fatal): {}", e)
                            }
                        }
                    }
                    _ => {
                        // Other statements don't require catalog persistence
                    }
                }
                log::debug!("DDL operation completed successfully: {}", message);

                Ok(QueryResult {
                    rows_affected,
                    session_result: None,
                    warnings: Vec::new(),
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
                })
            }
            Err(e) => {
                // Don't log as error if it's a duplicate entry error for IF NOT EXISTS statements
                // These are expected and handled gracefully
                let error_str = format!("{}", e);
                if !error_str.contains("Duplicate entry") && !error_str.contains("already exists") {
                    log::error!("DDL operation failed: {}", e);
                }
                Err(e)
            }
        }
    }

    /// Execute CREATE PROCEDURE statement
    fn execute_create_procedure(
        create_procedure: &crate::ast::CreateProcedureStatement,
        _context: &crate::exec::ExecutionContext,
        catalog_manager: &mut crate::catalog::manager::CatalogManager,
        _storage: &std::sync::Arc<crate::storage::StorageManager>,
    ) -> Result<(String, usize), crate::exec::error::ExecutionError> {
        use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType};
        use serde_json::json;

        // Protect gql.* namespace - reserved for vendor system procedures
        if create_procedure.procedure_name.starts_with("gql.") {
            return Err(crate::exec::error::ExecutionError::RuntimeError(
                format!("Cannot create procedure '{}': The 'gql.*' namespace is reserved for system procedures. Use a schema-based namespace instead (e.g., 'myschema.my_procedure')",
                    create_procedure.procedure_name)
            ));
        }

        // Serialize procedure body and parameters
        let procedure_json = json!({
            "parameters": create_procedure.parameters,
            "procedure_body": create_procedure.procedure_body,
            "or_replace": create_procedure.or_replace,
            "if_not_exists": create_procedure.if_not_exists,
        });

        let response = catalog_manager
            .execute(
                "procedure",
                CatalogOperation::Create {
                    entity_type: EntityType::Procedure,
                    name: create_procedure.procedure_name.clone(),
                    params: procedure_json,
                },
            )
            .map_err(|e| {
                crate::exec::error::ExecutionError::CatalogError(format!(
                    "Failed to create procedure: {}",
                    e
                ))
            })?;

        match response {
            CatalogResponse::Success { data: _ } => Ok((
                format!(
                    "Procedure '{}' created successfully",
                    create_procedure.procedure_name
                ),
                0,
            )),
            CatalogResponse::Error { message } => {
                Err(crate::exec::error::ExecutionError::CatalogError(format!(
                    "Failed to create procedure: {}",
                    message
                )))
            }
            _ => Err(crate::exec::error::ExecutionError::CatalogError(
                "Unexpected response from catalog".to_string(),
            )),
        }
    }

    /// Execute DROP PROCEDURE statement
    fn execute_drop_procedure(
        drop_procedure: &crate::ast::DropProcedureStatement,
        _context: &crate::exec::ExecutionContext,
        catalog_manager: &mut crate::catalog::manager::CatalogManager,
        _storage: &std::sync::Arc<crate::storage::StorageManager>,
    ) -> Result<(String, usize), crate::exec::error::ExecutionError> {
        use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType};

        // Protect gql.* namespace - reserved for vendor system procedures
        if drop_procedure.procedure_name.starts_with("gql.") {
            return Err(crate::exec::error::ExecutionError::RuntimeError(
                format!("Cannot drop procedure '{}': The 'gql.*' namespace is reserved for system procedures and cannot be modified",
                    drop_procedure.procedure_name)
            ));
        }

        let response = catalog_manager
            .execute(
                "procedure",
                CatalogOperation::Drop {
                    entity_type: EntityType::Procedure,
                    name: drop_procedure.procedure_name.clone(),
                    cascade: false,
                },
            )
            .map_err(|e| {
                crate::exec::error::ExecutionError::CatalogError(format!(
                    "Failed to drop procedure: {}",
                    e
                ))
            })?;

        match response {
            CatalogResponse::Success { data: _ } => Ok((
                format!(
                    "Procedure '{}' dropped successfully",
                    drop_procedure.procedure_name
                ),
                0,
            )),
            CatalogResponse::Error { message } => {
                if drop_procedure.if_exists && message.contains("not found") {
                    Ok((
                        format!(
                            "Procedure '{}' does not exist (IF EXISTS)",
                            drop_procedure.procedure_name
                        ),
                        0,
                    ))
                } else {
                    Err(crate::exec::error::ExecutionError::CatalogError(format!(
                        "Failed to drop procedure: {}",
                        message
                    )))
                }
            }
            _ => Err(crate::exec::error::ExecutionError::CatalogError(
                "Unexpected response from catalog".to_string(),
            )),
        }
    }
}
