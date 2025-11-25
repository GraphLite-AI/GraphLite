// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// DropRoleExecutor - Implements DROP ROLE statement execution
use crate::ast::DropRoleStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

pub struct DropRoleExecutor {
    statement: DropRoleStatement,
}

impl DropRoleExecutor {
    pub fn new(statement: DropRoleStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for DropRoleExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropRole
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("DROP ROLE '{}'", self.statement.role_name)
    }
}

impl DDLStatementExecutor for DropRoleExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let role_name = &self.statement.role_name;

        // Prevent deletion of system roles
        if role_name == "admin" || role_name == "user" {
            return Err(ExecutionError::RuntimeError(format!(
                "Cannot drop system role '{}'. System roles (admin, user) cannot be deleted.",
                role_name
            )));
        }

        // Create the catalog operation for dropping the role
        let drop_op = CatalogOperation::Drop {
            entity_type: EntityType::Role,
            name: role_name.clone(),
            cascade: false, // Role dropping doesn't support cascade yet
        };

        // Execute the operation through the catalog manager
        let drop_result = catalog_manager.execute("security", drop_op);
        match drop_result {
            Ok(_response) => {
                // Persist the catalog changes transactionally
                let persist_result = catalog_manager.persist_catalog("security");
                if let Err(e) = persist_result {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Failed to persist role deletion '{}' to storage: {}",
                        role_name, e
                    )));
                }

                let message = if self.statement.if_exists {
                    format!("Role '{}' dropped successfully (if exists)", role_name)
                } else {
                    format!("Role '{}' dropped successfully", role_name)
                };
                Ok((message, 1))
            }
            Err(catalog_error) => {
                // Handle "not found" errors differently for IF EXISTS
                let error_msg = catalog_error.to_string();
                if error_msg.contains("not found") && self.statement.if_exists {
                    let message = format!("Role '{}' does not exist (if exists)", role_name);
                    Ok((message, 0))
                } else {
                    Err(ExecutionError::RuntimeError(format!(
                        "Failed to drop role '{}': {}",
                        role_name, catalog_error
                    )))
                }
            }
        }
    }
}
