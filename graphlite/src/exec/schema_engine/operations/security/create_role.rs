// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// CreateRoleExecutor - Implements CREATE ROLE statement execution
use crate::ast::CreateRoleStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::schema_engine::operations::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use serde_json::Value;

pub struct CreateRoleExecutor {
    statement: CreateRoleStatement,
}

impl CreateRoleExecutor {
    pub fn new(statement: CreateRoleStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for CreateRoleExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateRole
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("CREATE ROLE '{}'", self.statement.role_name)
    }
}

impl DDLStatementExecutor for CreateRoleExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let role_name = &self.statement.role_name;

        // Create parameters for the role
        let mut params = serde_json::Map::new();
        params.insert("name".to_string(), Value::String(role_name.clone()));

        if let Some(description) = &self.statement.description {
            params.insert(
                "description".to_string(),
                Value::String(description.clone()),
            );
        }

        // Convert permissions to JSON format if any
        if !self.statement.permissions.is_empty() {
            let permissions: Vec<Value> = self
                .statement
                .permissions
                .iter()
                .map(|perm| {
                    let mut perm_obj = serde_json::Map::new();
                    perm_obj.insert(
                        "resource_type".to_string(),
                        Value::String(perm.resource_type.clone()),
                    );
                    if let Some(resource_name) = &perm.resource_name {
                        perm_obj.insert(
                            "resource_name".to_string(),
                            Value::String(resource_name.clone()),
                        );
                    }
                    Value::Object(perm_obj)
                })
                .collect();
            params.insert("permissions".to_string(), Value::Array(permissions));
        }

        params.insert(
            "if_not_exists".to_string(),
            Value::Bool(self.statement.if_not_exists),
        );

        // Create the catalog operation
        let operation = CatalogOperation::Create {
            entity_type: EntityType::Role,
            name: role_name.clone(),
            params: Value::Object(params),
        };

        // Execute the operation through the catalog manager
        let create_result = catalog_manager.execute("security", operation);
        match create_result {
            Ok(_response) => {
                // Persist the catalog changes transactionally
                let persist_result = catalog_manager.persist_catalog("security");
                if let Err(e) = persist_result {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Failed to persist role '{}' to storage: {}",
                        role_name, e
                    )));
                }

                let message = if self.statement.if_not_exists {
                    format!("Role '{}' created successfully (if not exists)", role_name)
                } else {
                    format!("Role '{}' created successfully", role_name)
                };
                Ok((message, 1))
            }
            Err(catalog_error) => Err(ExecutionError::RuntimeError(format!(
                "Failed to create role '{}': {}",
                role_name, catalog_error
            ))),
        }
    }
}
