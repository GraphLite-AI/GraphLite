// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use crate::ast::CreateSchemaStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for CREATE SCHEMA statements
pub struct CreateSchemaExecutor {
    statement: CreateSchemaStatement,
}

impl CreateSchemaExecutor {
    pub fn new(statement: CreateSchemaStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for CreateSchemaExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateTable // TODO: Add CreateSchema to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        let schema_name = self.statement.schema_path.name().map_or("unknown", |v| v);
        if self.statement.if_not_exists {
            format!("CREATE SCHEMA IF NOT EXISTS {}", schema_name)
        } else {
            format!("CREATE SCHEMA {}", schema_name)
        }
    }
}

impl DDLStatementExecutor for CreateSchemaExecutor {
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

        // Validate schema name
        if schema_name.is_empty() || schema_name.trim().is_empty() {
            return Err(ExecutionError::RuntimeError(
                "Invalid schema name: schema name cannot be empty".to_string(),
            ));
        }

        if schema_name.contains(' ') {
            return Err(ExecutionError::RuntimeError(
                "Invalid schema name: schema name cannot contain spaces".to_string(),
            ));
        }

        if schema_name.starts_with(char::is_numeric) {
            return Err(ExecutionError::RuntimeError(
                "Invalid schema name: schema name cannot start with a number".to_string(),
            ));
        }

        let create_op = CatalogOperation::Create {
            entity_type: EntityType::Schema,
            name: schema_name.clone(),
            params: serde_json::json!({
                "schema_path": self.statement.schema_path.to_string(),
                "if_not_exists": self.statement.if_not_exists
            }),
        };

        let result = catalog_manager.execute("schema", create_op);
        match result {
            Ok(response) => match response {
                crate::catalog::operations::CatalogResponse::Success { data: _ } => {
                    let persist_result = catalog_manager.persist_catalog("schema");
                    if let Err(e) = persist_result {
                        log::error!("Failed to persist schema catalog: {}", e);
                    }

                    // Invalidate catalog cache - schema list has changed
                    if let Some(cache_mgr) = &context.cache_manager {
                        cache_mgr.invalidate_on_schema_change(
                            schema_name.clone(),
                            "schema_created".to_string(),
                        );
                        log::debug!("Invalidated catalog cache after CREATE SCHEMA '{}'", schema_name);
                    }

                    let message = if self.statement.if_not_exists {
                        format!("Schema '{}' created (if not exists)", schema_name)
                    } else {
                        format!("Schema '{}' created", schema_name)
                    };

                    Ok((message, 1))
                }
                crate::catalog::operations::CatalogResponse::Error { message }
                    if message == "Already exists" =>
                {
                    if self.statement.if_not_exists {
                        let message =
                            format!("Schema '{}' already exists (if not exists)", schema_name);
                        Ok((message, 0))
                    } else {
                        Err(ExecutionError::CatalogError(format!(
                            "Schema '{}' already exists",
                            schema_name
                        )))
                    }
                }
                crate::catalog::operations::CatalogResponse::Error { message } => {
                    Err(ExecutionError::CatalogError(format!(
                        "Failed to create schema '{}': {}",
                        schema_name, message
                    )))
                }
                _ => Err(ExecutionError::CatalogError(
                    "Unexpected response from schema_metadata catalog".to_string(),
                )),
            },
            Err(e) => Err(ExecutionError::CatalogError(format!(
                "Failed to create schema: {}",
                e
            ))),
        }
    }
}
