// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// CREATE GRAPH TYPE executor implementation
use crate::ast::ast::CreateGraphTypeStatement;
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogOperation, EntityType};
use crate::storage::StorageManager;
use crate::txn::state::OperationType;
use crate::schema::types::{GraphTypeDefinition, GraphTypeVersion};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;

pub struct CreateGraphTypeExecutor {
    statement: CreateGraphTypeStatement,
}

impl CreateGraphTypeExecutor {
    pub fn new(statement: CreateGraphTypeStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for CreateGraphTypeExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::CreateTable // TODO: Add CreateGraphType to OperationType enum
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        let name = self.statement.graph_type_path.segments.join(".");
        format!("CREATE GRAPH TYPE {}", name)
    }
}

impl DDLStatementExecutor for CreateGraphTypeExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        _storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        let name = self.statement.graph_type_path.segments.join(".");

        // Check if graph type already exists
        if !self.statement.or_replace {
            let check_result = catalog_manager.query_read_only(
                "graph_type",
                crate::catalog::operations::QueryType::GetGraphType,
                json!({ "name": &name }),
            );

            if check_result.is_ok() {
                if self.statement.if_not_exists {
                    return Ok((format!("Graph type '{}' already exists", name), 0));
                } else {
                    return Err(ExecutionError::SchemaValidation(format!(
                        "Graph type '{}' already exists",
                        name
                    )));
                }
            }
        }

        // Parse GraphTypeSpec to extract node and edge type definitions
        let node_types = self.parse_vertex_types(&self.statement.graph_type_spec);
        let edge_types = self.parse_edge_types(&self.statement.graph_type_spec);

        let graph_type_def = GraphTypeDefinition {
            name: name.clone(),
            version: GraphTypeVersion::new(1, 0, 0),
            previous_version: None,
            node_types,
            edge_types,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: context.get_variable("user_id")
                .and_then(|v| v.as_string().map(|s| s.to_string()))
                .unwrap_or_else(|| "system".to_string()),
            description: Some(format!("Graph type {}", name)),
            metadata: HashMap::new(),
        };

        // Create the graph type in the catalog
        let operation = CatalogOperation::Create {
            entity_type: EntityType::GraphType,
            name: name.clone(),
            params: json!(graph_type_def),
        };

        catalog_manager.execute("graph_type", operation)
            .map_err(|e| ExecutionError::CatalogError(format!("Failed to create graph type: {}", e)))?;

        Ok((format!("Graph type '{}' created successfully", name), 1))
    }
}

impl CreateGraphTypeExecutor {
    /// Parse vertex types from GraphTypeSpec into NodeTypeDefinition
    fn parse_vertex_types(&self, spec: &crate::ast::ast::GraphTypeSpec) -> Vec<crate::schema::types::NodeTypeDefinition> {
        use crate::schema::types::{NodeTypeDefinition, PropertyDefinition, DataType};

        spec.vertex_types.iter().map(|vertex_spec| {
            // Get the label from identifier
            let label = vertex_spec.identifier.clone().unwrap_or_else(|| "UnnamedNode".to_string());

            // Parse properties
            let properties = if let Some(ref prop_list) = vertex_spec.properties {
                prop_list.properties.iter().map(|prop_decl| {
                    // Convert AST TypeSpec to schema DataType
                    let data_type = match &prop_decl.type_spec {
                        crate::ast::ast::TypeSpec::String { .. } => DataType::String,
                        crate::ast::ast::TypeSpec::Integer => DataType::Integer,
                        crate::ast::ast::TypeSpec::BigInt => DataType::BigInt,
                        crate::ast::ast::TypeSpec::Float { .. } => DataType::Float,
                        crate::ast::ast::TypeSpec::Double => DataType::Double,
                        crate::ast::ast::TypeSpec::Boolean => DataType::Boolean,
                        crate::ast::ast::TypeSpec::Date => DataType::Date,
                        crate::ast::ast::TypeSpec::LocalTime { .. } => DataType::Time,
                        crate::ast::ast::TypeSpec::LocalDateTime { .. } => DataType::DateTime,
                        _ => DataType::String, // Default to string for unsupported types
                    };

                    PropertyDefinition {
                        name: prop_decl.name.clone(),
                        data_type,
                        required: false, // TODO: Parse from constraints
                        unique: false,   // TODO: Parse from constraints
                        default_value: None,
                        description: None,
                        deprecated: false,
                        deprecation_message: None,
                        validation_pattern: None,
                        constraints: vec![],
                    }
                }).collect()
            } else {
                vec![]
            };

            NodeTypeDefinition {
                label,
                properties,
                constraints: vec![], // TODO: Parse constraints from property annotations
                description: None,
                is_abstract: false,
                extends: None,
            }
        }).collect()
    }

    /// Parse edge types from GraphTypeSpec into EdgeTypeDefinition
    fn parse_edge_types(&self, spec: &crate::ast::ast::GraphTypeSpec) -> Vec<crate::schema::types::EdgeTypeDefinition> {
        use crate::schema::types::{EdgeTypeDefinition, PropertyDefinition, DataType, EdgeCardinality};

        spec.edge_types.iter().map(|edge_spec| {
            // Get the type name from identifier
            let type_name = edge_spec.identifier.clone().unwrap_or_else(|| "UnnamedEdge".to_string());

            // Parse properties
            let properties = if let Some(ref prop_list) = edge_spec.properties {
                prop_list.properties.iter().map(|prop_decl| {
                    let data_type = match &prop_decl.type_spec {
                        crate::ast::ast::TypeSpec::String { .. } => DataType::String,
                        crate::ast::ast::TypeSpec::Integer => DataType::Integer,
                        crate::ast::ast::TypeSpec::BigInt => DataType::BigInt,
                        crate::ast::ast::TypeSpec::Float { .. } => DataType::Float,
                        crate::ast::ast::TypeSpec::Double => DataType::Double,
                        crate::ast::ast::TypeSpec::Boolean => DataType::Boolean,
                        crate::ast::ast::TypeSpec::Date => DataType::Date,
                        crate::ast::ast::TypeSpec::LocalTime { .. } => DataType::Time,
                        crate::ast::ast::TypeSpec::LocalDateTime { .. } => DataType::DateTime,
                        _ => DataType::String,
                    };

                    PropertyDefinition {
                        name: prop_decl.name.clone(),
                        data_type,
                        required: false,
                        unique: false,
                        default_value: None,
                        description: None,
                        deprecated: false,
                        deprecation_message: None,
                        validation_pattern: None,
                        constraints: vec![],
                    }
                }).collect()
            } else {
                vec![]
            };

            EdgeTypeDefinition {
                type_name,
                from_node_types: vec![], // TODO: Parse from SOURCE clause
                to_node_types: vec![],   // TODO: Parse from DESTINATION clause
                properties,
                constraints: vec![],
                description: None,
                cardinality: EdgeCardinality::default(), // Default (no constraints)
            }
        }).collect()
    }
}
