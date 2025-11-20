// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// RevokeRoleExecutor - Implements REVOKE ROLE statement execution
use crate::ast::ast::RevokeRoleStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use serde_json::json;

pub struct RevokeRoleExecutor {
    statement: RevokeRoleStatement,
}

impl RevokeRoleExecutor {
    pub fn new(statement: RevokeRoleStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for RevokeRoleExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::RevokeRole
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!(
            "REVOKE ROLE '{}' FROM '{}'",
            self.statement.role_name, self.statement.username
        )
    }
}

impl DDLStatementExecutor for RevokeRoleExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let role_name = &self.statement.role_name;
        let username = &self.statement.username;

        // System role protection rules
        // 1. The "user" role cannot be removed from any user
        if role_name == "user" {
            return Err(ExecutionError::RuntimeError(format!(
                "Cannot revoke system role 'user' from any user. The 'user' role is required for all users."
            )));
        }

        // 2. The "admin" role cannot be removed from the "admin" user
        if role_name == "admin" && username == "admin" {
            return Err(ExecutionError::RuntimeError(format!(
                "Cannot revoke 'admin' role from 'admin' user. The admin user must retain admin privileges."
            )));
        }

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

        // Get the current user to check their roles
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

        // Check if user has this role
        if !current_roles.contains(role_name) {
            return Ok((
                format!("User '{}' does not have role '{}'", username, role_name),
                0,
            ));
        }

        // Remove the role using the remove_roles format
        let update_params = json!({
            "remove_roles": [role_name]
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
                        "Failed to persist role revocation for user '{}': {}",
                        username, e
                    )));
                }

                let message = format!(
                    "Role '{}' revoked from user '{}' successfully",
                    role_name, username
                );
                Ok((message, 1))
            }
            Err(catalog_error) => Err(ExecutionError::RuntimeError(format!(
                "Failed to revoke role '{}' from user '{}': {}",
                role_name, username, catalog_error
            ))),
        }
    }
}
