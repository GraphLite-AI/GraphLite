// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// ALTER GRAPH TYPE executor implementation (Phase 4)

use serde_json::json;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType, QueryType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::schema::parser::ast::AlterGraphTypeStatement;
use crate::schema::types::{GraphTypeDefinition, GraphTypeVersion};
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for ALTER GRAPH TYPE statements
pub struct AlterGraphTypeExecutor {
    statement: AlterGraphTypeStatement,
}

impl AlterGraphTypeExecutor {
    /// Create a new ALTER GRAPH TYPE executor
    pub fn new(statement: AlterGraphTypeStatement) -> Self {
        Self { statement }
    }

    /// Get the current graph type definition
    fn get_current_definition(
        &self,
        catalog_manager: &CatalogManager,
    ) -> Result<GraphTypeDefinition, ExecutionError> {
        let response = catalog_manager
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                json!({ "name": self.statement.name }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            serde_json::from_value(data.clone()).map_err(|e| {
                ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                self.statement.name
            )))
        }
    }

    /// Auto-increment version based on the type of changes
    fn auto_increment_version(
        &self,
        current: &GraphTypeVersion,
        has_breaking_changes: bool,
    ) -> GraphTypeVersion {
        if has_breaking_changes {
            // Major version bump for breaking changes
            GraphTypeVersion::new(current.major + 1, 0, 0)
        } else {
            // Minor version bump for new features
            GraphTypeVersion::new(current.major, current.minor + 1, 0)
        }
    }
}

impl StatementExecutor for AlterGraphTypeExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::AlterTable // Reusing AlterTable for now
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("ALTER GRAPH TYPE {}", self.statement.name)
    }
}

impl DDLStatementExecutor for AlterGraphTypeExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Get the current graph type definition
        let current_definition = self.get_current_definition(catalog_manager)?;

        // For now, return a simple implementation that creates a new version
        // Full implementation would process the ALTER operations from the statement
        let mut new_definition = current_definition.clone();

        // Auto-increment version
        let new_version = self.auto_increment_version(&current_definition.version, false);
        new_definition.version = new_version.clone();
        new_definition.updated_at = chrono::Utc::now();
        new_definition.previous_version = Some(current_definition.version.clone());

        // Note: Migration validation would go here in a full implementation
        // For now, we directly create the new version

        // Create the new version in the catalog
        let params = serde_json::to_value(&new_definition).map_err(|e| {
            ExecutionError::RuntimeError(format!("Failed to serialize graph type: {}", e))
        })?;

        catalog_manager
            .execute(
                "graph_type",
                CatalogOperation::Create {
                    entity_type: EntityType::GraphType,
                    name: self.statement.name.clone(),
                    params,
                },
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to create new version: {}", e))
            })?;

        Ok((
            format!(
                "Successfully altered graph type '{}' to version {}",
                self.statement.name,
                new_version.to_string()
            ),
            0,
        ))
    }
}
