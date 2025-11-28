// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// GrantRoleExecutor - Implements GRANT ROLE statement execution
use crate::ast::GrantRoleStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use serde_json::json;

pub struct GrantRoleExecutor {
    statement: GrantRoleStatement,
}

impl GrantRoleExecutor {
    pub fn new(statement: GrantRoleStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for GrantRoleExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::GrantRole
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!(
            "GRANT ROLE '{}' TO '{}'",
            self.statement.role_name, self.statement.username
        )
    }
}

impl DDLStatementExecutor for GrantRoleExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let role_name = &self.statement.role_name;
        let username = &self.statement.username;

        // First, verify the role exists
        let role_query_result = catalog_manager.execute(
            "security",
            CatalogOperation::Query {
                query_type: QueryType::GetRole,
                params: json!({ "name": role_name }),
            },
        );
        let _role_response = match role_query_result {
            Ok(response) => response,
            Err(_) => {
                return Err(ExecutionError::RuntimeError(format!(
                    "Role '{}' does not exist",
                    role_name
                )));
            }
        };

        // Get the current user to check if they already have this role
        let user_query_result = catalog_manager.execute(
            "security",
            CatalogOperation::Query {
                query_type: QueryType::GetUser,
                params: json!({ "name": username }),
            },
        );
        let user_response = match user_query_result {
            Ok(response) => response,
            Err(_) => {
                return Err(ExecutionError::RuntimeError(format!(
                    "User '{}' does not exist",
                    username
                )));
            }
        };

        // Extract user data from response
        let current_user = match user_response {
            CatalogResponse::Query { results } => results,
            _ => {
                return Err(ExecutionError::RuntimeError(format!(
                    "Unexpected response when getting user '{}'",
                    username
                )));
            }
        };

        // Extract current roles from user
        let current_roles: Vec<String> = if let Some(roles_value) = current_user.get("roles") {
            if let Some(roles_array) = roles_value.as_array() {
                roles_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        // Check if user already has this role
        if current_roles.contains(role_name) {
            return Ok((
                format!("User '{}' already has role '{}'", username, role_name),
                0,
            ));
        }

        // Add the new role using the add_roles format
        let update_params = json!({
            "add_roles": [role_name]
        });

        let update_op = CatalogOperation::Update {
            entity_type: EntityType::User,
            name: username.clone(),
            updates: update_params,
        };

        // Execute the update operation
        let update_result = catalog_manager.execute("security", update_op);
        match update_result {
            Ok(_response) => {
                // Persist the catalog changes transactionally
                let persist_result = catalog_manager.persist_catalog("security");
                if let Err(e) = persist_result {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Failed to persist role grant for user '{}': {}",
                        username, e
                    )));
                }

                let message = format!(
                    "Role '{}' granted to user '{}' successfully",
                    role_name, username
                );
                Ok((message, 1))
            }
            Err(catalog_error) => Err(ExecutionError::RuntimeError(format!(
                "Failed to grant role '{}' to user '{}': {}",
                role_name, username, catalog_error
            ))),
        }
    }
}
