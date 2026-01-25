// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// DropUserExecutor - Implements DROP USER statement execution
use crate::ast::DropUserStatement;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::exec::schema_engine::operations::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

pub struct DropUserExecutor {
    statement: DropUserStatement,
}

impl DropUserExecutor {
    pub fn new(statement: DropUserStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for DropUserExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::DropUser
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("DROP USER '{}'", self.statement.username)
    }
}

impl DDLStatementExecutor for DropUserExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let username = &self.statement.username;

        // Create the catalog operation for dropping the user
        let drop_op = CatalogOperation::Drop {
            entity_type: EntityType::User,
            name: username.clone(),
            cascade: false, // User dropping doesn't support cascade yet
        };

        // Execute the operation through the catalog manager
        let drop_result = catalog_manager.execute("security", drop_op);
        match drop_result {
            Ok(_response) => {
                // Persist the catalog changes transactionally
                let persist_result = catalog_manager.persist_catalog("security");
                if let Err(e) = persist_result {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Failed to persist user deletion '{}' to storage: {}",
                        username, e
                    )));
                }

                let message = if self.statement.if_exists {
                    format!("User '{}' dropped successfully (if exists)", username)
                } else {
                    format!("User '{}' dropped successfully", username)
                };
                Ok((message, 1))
            }
            Err(catalog_error) => {
                // Handle "not found" errors differently for IF EXISTS
                let error_msg = catalog_error.to_string();
                if error_msg.contains("not found") && self.statement.if_exists {
                    let message = format!("User '{}' does not exist (if exists)", username);
                    Ok((message, 0))
                } else {
                    Err(ExecutionError::RuntimeError(format!(
                        "Failed to drop user '{}': {}",
                        username, catalog_error
                    )))
                }
            }
        }
    }
}
