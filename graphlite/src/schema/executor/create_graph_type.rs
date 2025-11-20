// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// CREATE GRAPH TYPE executor implementation

use chrono::Utc;
use serde_json;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::schema::parser::ast::CreateGraphTypeStatement;
use crate::schema::types::{GraphTypeDefinition, GraphTypeVersion};
use crate::storage::StorageManager;
use crate::txn::state::OperationType;

/// Executor for CREATE GRAPH TYPE statements
pub struct CreateGraphTypeExecutor {
    statement: CreateGraphTypeStatement,
}

impl CreateGraphTypeExecutor {
    /// Create a new CREATE GRAPH TYPE executor
    #[allow(dead_code)] // ROADMAP v0.4.0 - Constructor for CREATE GRAPH TYPE executor (module disabled in mod.rs)
    pub fn new(statement: CreateGraphTypeStatement) -> Self {
        Self { statement }
    }

    /// Convert the statement into a GraphTypeDefinition
    fn build_graph_type_definition(
        &self,
        context: &ExecutionContext,
    ) -> Result<GraphTypeDefinition, ExecutionError> {
        // Get current user from context
        let created_by = context
            .current_user
            .clone()
            .unwrap_or_else(|| "system".to_string());

        // Use provided version or default to 1.0.0
        let version = self
            .statement
            .version
            .clone()
            .unwrap_or_else(|| GraphTypeVersion::new(1, 0, 0));

        Ok(GraphTypeDefinition {
            name: self.statement.name.clone(),
            version,
            previous_version: None,
            node_types: self.statement.node_types.clone(),
            edge_types: self.statement.edge_types.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by,
            description: None,
            metadata: Default::default(),
        })
    }

    /// Validate the graph type definition
    fn validate_graph_type(&self) -> Result<(), ExecutionError> {
        // Validate node type names are unique
        let mut node_labels = std::collections::HashSet::new();
        for node_type in &self.statement.node_types {
            if !node_labels.insert(&node_type.label) {
                return Err(ExecutionError::ValidationError(format!(
                    "Duplicate node type label: {}",
                    node_type.label
                )));
            }
        }

        // Validate edge type names are unique
        let mut edge_types = std::collections::HashSet::new();
        for edge_type in &self.statement.edge_types {
            if !edge_types.insert(&edge_type.type_name) {
                return Err(ExecutionError::ValidationError(format!(
                    "Duplicate edge type: {}",
                    edge_type.type_name
                )));
            }
        }

        // Validate edge types reference valid node types
        for edge_type in &self.statement.edge_types {
            for from_type in &edge_type.from_node_types {
                if !node_labels.contains(from_type) {
                    return Err(ExecutionError::ValidationError(format!(
                        "Edge type '{}' references unknown node type '{}' in FROM clause",
                        edge_type.type_name, from_type
                    )));
                }
            }
            for to_type in &edge_type.to_node_types {
                if !node_labels.contains(to_type) {
                    return Err(ExecutionError::ValidationError(format!(
                        "Edge type '{}' references unknown node type '{}' in TO clause",
                        edge_type.type_name, to_type
                    )));
                }
            }
        }

        // Validate property names within node types are unique
        for node_type in &self.statement.node_types {
            let mut prop_names = std::collections::HashSet::new();
            for prop in &node_type.properties {
                if !prop_names.insert(&prop.name) {
                    return Err(ExecutionError::ValidationError(format!(
                        "Duplicate property '{}' in node type '{}'",
                        prop.name, node_type.label
                    )));
                }
            }
        }

        // Validate property names within edge types are unique
        for edge_type in &self.statement.edge_types {
            let mut prop_names = std::collections::HashSet::new();
            for prop in &edge_type.properties {
                if !prop_names.insert(&prop.name) {
                    return Err(ExecutionError::ValidationError(format!(
                        "Duplicate property '{}' in edge type '{}'",
                        prop.name, edge_type.type_name
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if the graph type already exists
    fn check_graph_type_exists(
        &self,
        catalog_manager: &CatalogManager,
    ) -> Result<bool, ExecutionError> {
        // Query the graph_type catalog to check existence
        let query_result = catalog_manager.query_read_only(
            "graph_type",
            crate::catalog::operations::QueryType::Exists,
            serde_json::json!({ "name": self.statement.name }),
        );

        match query_result {
            Ok(CatalogResponse::Success { data: Some(data) }) => Ok(data
                .get("exists")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)),
            Err(_) => Ok(false), // Catalog might not be registered yet
            _ => Ok(false),
        }
    }
}

impl StatementExecutor for CreateGraphTypeExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateTable // TODO: Add CreateGraphType to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("CREATE GRAPH TYPE {}", self.statement.name)
    }
}

impl DDLStatementExecutor for CreateGraphTypeExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        // Validate the graph type definition
        self.validate_graph_type()?;

        // Check if graph type already exists
        if !self.statement.if_not_exists {
            if self.check_graph_type_exists(catalog_manager)? {
                return Err(ExecutionError::ValidationError(format!(
                    "Graph type '{}' already exists",
                    self.statement.name
                )));
            }
        } else if self.check_graph_type_exists(catalog_manager)? {
            // IF NOT EXISTS specified and type exists - return success
            return Ok((
                format!(
                    "Graph type '{}' already exists, skipping creation",
                    self.statement.name
                ),
                0,
            ));
        }

        // Build the graph type definition
        let graph_type = self.build_graph_type_definition(context)?;

        // The graph type catalog should already be registered during system initialization
        if !catalog_manager.has_catalog("graph_type") {
            return Err(ExecutionError::RuntimeError(
                "Graph type catalog is not registered. Please ensure the system is properly initialized.".to_string()
            ));
        }

        // Store the graph type in the catalog
        let response = catalog_manager
            .execute(
                "graph_type",
                CatalogOperation::Create {
                    entity_type: EntityType::GraphType,
                    name: graph_type.name.clone(),
                    params: serde_json::to_value(&graph_type).map_err(|e| {
                        ExecutionError::RuntimeError(format!(
                            "Failed to serialize graph type: {}",
                            e
                        ))
                    })?,
                },
            )
            .map_err(|e| {
                ExecutionError::RuntimeError(format!("Failed to create graph type: {}", e))
            })?;

        match response {
            CatalogResponse::Success { data } => {
                let name = data
                    .as_ref()
                    .and_then(|d| d.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or(&self.statement.name);
                Ok((format!("Graph type '{}' created successfully", name), 1))
            }
            CatalogResponse::Error { message } => Err(ExecutionError::RuntimeError(message)),
            _ => Err(ExecutionError::RuntimeError(
                "Unexpected response from catalog".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::types::{DataType, NodeTypeDefinition, PropertyDefinition};

    #[test]
    fn test_validate_duplicate_node_types() {
        let statement = CreateGraphTypeStatement {
            name: "TestType".to_string(),
            if_not_exists: false,
            version: None,
            node_types: vec![
                NodeTypeDefinition {
                    label: "User".to_string(),
                    properties: vec![],
                    constraints: vec![],
                    description: None,
                    is_abstract: false,
                    extends: None,
                },
                NodeTypeDefinition {
                    label: "User".to_string(), // Duplicate
                    properties: vec![],
                    constraints: vec![],
                    description: None,
                    is_abstract: false,
                    extends: None,
                },
            ],
            edge_types: vec![],
        };

        let executor = CreateGraphTypeExecutor::new(statement);
        assert!(executor.validate_graph_type().is_err());
    }

    #[test]
    fn test_validate_duplicate_properties() {
        let statement = CreateGraphTypeStatement {
            name: "TestType".to_string(),
            if_not_exists: false,
            version: None,
            node_types: vec![NodeTypeDefinition {
                label: "User".to_string(),
                properties: vec![
                    PropertyDefinition {
                        name: "email".to_string(),
                        data_type: DataType::String,
                        required: true,
                        unique: true,
                        default_value: None,
                        description: None,
                        deprecated: false,
                        deprecation_message: None,
                        validation_pattern: None,
                        constraints: vec![],
                    },
                    PropertyDefinition {
                        name: "email".to_string(), // Duplicate
                        data_type: DataType::String,
                        required: false,
                        unique: false,
                        default_value: None,
                        description: None,
                        deprecated: false,
                        deprecation_message: None,
                        validation_pattern: None,
                        constraints: vec![],
                    },
                ],
                constraints: vec![],
                description: None,
                is_abstract: false,
                extends: None,
            }],
            edge_types: vec![],
        };

        let executor = CreateGraphTypeExecutor::new(statement);
        assert!(executor.validate_graph_type().is_err());
    }
}
