// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// CreateUserExecutor - Implements CREATE USER statement execution
use crate::ast::CreateUserStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use serde_json::json;

pub struct CreateUserExecutor {
    statement: CreateUserStatement,
}

impl CreateUserExecutor {
    pub fn new(statement: CreateUserStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for CreateUserExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateUser
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("CREATE USER '{}'", self.statement.username)
    }
}

impl DDLStatementExecutor for CreateUserExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let username = &self.statement.username;

        // Prepare user creation parameters
        let mut user_params = json!({
            "username": username,
        });

        // Add password if provided
        if let Some(password) = &self.statement.password {
            user_params["password"] = json!(password);
        }

        // Always include default "user" role, plus any additional roles specified
        let mut user_roles = vec!["user".to_string()]; // Default role for all users

        // Add any additional roles specified in the CREATE USER statement
        for role in &self.statement.roles {
            if role != "user" {
                // Avoid duplicates
                user_roles.push(role.clone());
            }
        }

        user_params["roles"] = json!(user_roles);

        // Create the catalog operation for creating the user
        let create_op = CatalogOperation::Create {
            entity_type: EntityType::User,
            name: username.clone(),
            params: user_params,
        };

        // Execute the operation through the catalog manager
        let create_result = catalog_manager.execute("security", create_op);
        match create_result {
            Ok(_response) => {
                // Persist the catalog changes transactionally
                let persist_result = catalog_manager.persist_catalog("security");
                if let Err(e) = persist_result {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Failed to persist user creation '{}' to storage: {}",
                        username, e
                    )));
                }

                let message = if self.statement.if_not_exists {
                    format!("User '{}' created successfully (if not exists)", username)
                } else {
                    format!("User '{}' created successfully", username)
                };
                Ok((message, 1))
            }
            Err(catalog_error) => {
                // Handle "already exists" errors differently for IF NOT EXISTS
                let error_msg = catalog_error.to_string();
                if error_msg.contains("already exists") && self.statement.if_not_exists {
                    let message = format!("User '{}' already exists (if not exists)", username);
                    Ok((message, 0))
                } else {
                    Err(ExecutionError::RuntimeError(format!(
                        "Failed to create user '{}': {}",
                        username, catalog_error
                    )))
                }
            }
        }
    }
}
