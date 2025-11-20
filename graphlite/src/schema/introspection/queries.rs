// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Schema Introspection Queries
//
// Provides comprehensive schema metadata queries for discovering:
// - Graph types and their versions
// - Node type definitions and properties
// - Edge type definitions and constraints
// - Property constraints and data types
// - Schema usage statistics

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::QueryType;
use crate::exec::ExecutionError;
use crate::schema::types::{Constraint, GraphTypeDefinition};

/// Schema introspection interface
#[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type queries
pub struct SchemaIntrospection {
    catalog_manager: Arc<RwLock<CatalogManager>>,
}

/// Types of introspection queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntrospectionQuery {
    /// List all graph types
    ListGraphTypes,

    /// Get detailed information about a specific graph type
    DescribeGraphType {
        name: String,
        version: Option<String>,
    },

    /// List all node types in a graph type
    ListNodeTypes { graph_type: String },

    /// List all edge types in a graph type
    ListEdgeTypes { graph_type: String },

    /// Get properties for a specific node type
    DescribeNodeType {
        graph_type: String,
        node_label: String,
    },

    /// Get properties for a specific edge type
    DescribeEdgeType {
        graph_type: String,
        edge_label: String,
    },

    /// Get version history for a graph type
    GetVersionHistory { graph_type: String },

    /// Get schema statistics (usage, constraints, etc.)
    GetSchemaStatistics { graph_type: Option<String> },
}

/// Result of an introspection query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionResult {
    pub query_type: String,
    pub data: JsonValue,
    pub metadata: Option<JsonValue>,
}

impl SchemaIntrospection {
    /// Create a new schema introspection instance
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection constructor
    pub fn new(catalog_manager: Arc<RwLock<CatalogManager>>) -> Self {
        Self { catalog_manager }
    }

    /// Execute an introspection query
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    pub fn execute(
        &self,
        query: IntrospectionQuery,
    ) -> Result<IntrospectionResult, ExecutionError> {
        match query {
            IntrospectionQuery::ListGraphTypes => self.list_graph_types(),
            IntrospectionQuery::DescribeGraphType { name, version } => {
                self.describe_graph_type(&name, version.as_deref())
            }
            IntrospectionQuery::ListNodeTypes { graph_type } => self.list_node_types(&graph_type),
            IntrospectionQuery::ListEdgeTypes { graph_type } => self.list_edge_types(&graph_type),
            IntrospectionQuery::DescribeNodeType {
                graph_type,
                node_label,
            } => self.describe_node_type(&graph_type, &node_label),
            IntrospectionQuery::DescribeEdgeType {
                graph_type,
                edge_label,
            } => self.describe_edge_type(&graph_type, &edge_label),
            IntrospectionQuery::GetVersionHistory { graph_type } => {
                self.get_version_history(&graph_type)
            }
            IntrospectionQuery::GetSchemaStatistics { graph_type } => {
                self.get_schema_statistics(graph_type.as_deref())
            }
        }
    }

    /// List all graph types in the catalog
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn list_graph_types(&self) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only("graph_type", QueryType::List, json!({}))
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to list graph types: {}", e))
            })?;

        Ok(IntrospectionResult {
            query_type: "ListGraphTypes".to_string(),
            data: response.data().cloned().unwrap_or(json!([])),
            metadata: Some(json!({
                "count": response.data().and_then(|d| d.as_array()).map(|a| a.len()).unwrap_or(0)
            })),
        })
    }

    /// Describe a specific graph type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn describe_graph_type(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let query_params = if let Some(v) = version {
            json!({ "name": name, "version": v })
        } else {
            json!({ "name": name })
        };

        let response = catalog
            .query_read_only("graph_type", QueryType::GetGraphType, query_params)
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            // Parse the graph type definition
            let graph_type: GraphTypeDefinition =
                serde_json::from_value(data.clone()).map_err(|e| {
                    ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                })?;

            // Create a detailed description
            let description = json!({
                "name": graph_type.name,
                "version": format!("{}.{}.{}", graph_type.version.major, graph_type.version.minor, graph_type.version.patch),
                "description": graph_type.description,
                "node_types": graph_type.node_types.iter().map(|nt| {
                    json!({
                        "label": nt.label,
                        "description": nt.description,
                        "property_count": nt.properties.len(),
                        "constraint_count": nt.constraints.len(),
                        "is_abstract": nt.is_abstract,
                    })
                }).collect::<Vec<_>>(),
                "edge_types": graph_type.edge_types.iter().map(|et| {
                    json!({
                        "label": et.type_name,
                        "description": et.description,
                        "property_count": et.properties.len(),
                    })
                }).collect::<Vec<_>>(),
                "created_at": graph_type.created_at,
                "updated_at": graph_type.updated_at,
                "created_by": graph_type.created_by,
            });

            Ok(IntrospectionResult {
                query_type: "DescribeGraphType".to_string(),
                data: description,
                metadata: Some(json!({
                    "full_definition": data
                })),
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                name
            )))
        }
    }

    /// List all node types in a graph type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn list_node_types(&self, graph_type: &str) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                json!({ "name": graph_type }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            let graph_type_def: GraphTypeDefinition = serde_json::from_value(data.clone())
                .map_err(|e| {
                    ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                })?;

            let node_types: Vec<JsonValue> = graph_type_def.node_types.iter().map(|nt| {
                json!({
                    "label": nt.label,
                    "description": nt.description,
                    "properties": nt.properties.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                    "is_abstract": nt.is_abstract,
                })
            }).collect();

            Ok(IntrospectionResult {
                query_type: "ListNodeTypes".to_string(),
                data: json!(node_types),
                metadata: Some(json!({
                    "graph_type": graph_type,
                    "count": node_types.len()
                })),
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                graph_type
            )))
        }
    }

    /// List all edge types in a graph type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn list_edge_types(&self, graph_type: &str) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                json!({ "name": graph_type }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            let graph_type_def: GraphTypeDefinition = serde_json::from_value(data.clone())
                .map_err(|e| {
                    ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                })?;

            let edge_types: Vec<JsonValue> = graph_type_def.edge_types.iter().map(|et| {
                json!({
                    "label": et.type_name,
                    "description": et.description,
                    "properties": et.properties.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                    "source_node_types": et.from_node_types,
                    "target_node_types": et.to_node_types,
                })
            }).collect();

            Ok(IntrospectionResult {
                query_type: "ListEdgeTypes".to_string(),
                data: json!(edge_types),
                metadata: Some(json!({
                    "graph_type": graph_type,
                    "count": edge_types.len()
                })),
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                graph_type
            )))
        }
    }

    /// Describe a specific node type with all its properties
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn describe_node_type(
        &self,
        graph_type: &str,
        node_label: &str,
    ) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                json!({ "name": graph_type }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            let graph_type_def: GraphTypeDefinition = serde_json::from_value(data.clone())
                .map_err(|e| {
                    ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                })?;

            let node_type = graph_type_def
                .node_types
                .iter()
                .find(|nt| nt.label == node_label)
                .ok_or_else(|| {
                    ExecutionError::SchemaValidation(format!(
                        "Node type '{}' not found in graph type '{}'",
                        node_label, graph_type
                    ))
                })?;

            let properties: Vec<JsonValue> = node_type
                .properties
                .iter()
                .map(|p| {
                    json!({
                        "name": p.name,
                        "data_type": format!("{:?}", p.data_type),
                        "is_required": p.required,
                        "is_unique": p.unique,
                        "default_value": p.default_value,
                        "description": p.description,
                    })
                })
                .collect();

            let constraints: Vec<JsonValue> = node_type
                .constraints
                .iter()
                .map(|c| match c {
                    Constraint::Unique => json!({
                        "type": "UNIQUE",
                    }),
                    Constraint::NotNull => json!({
                        "type": "NOT_NULL",
                    }),
                    Constraint::PrimaryKey => json!({
                        "type": "PRIMARY_KEY",
                    }),
                    Constraint::ForeignKey {
                        references,
                        on_delete,
                    } => json!({
                        "type": "FOREIGN_KEY",
                        "references": references,
                        "on_delete": format!("{:?}", on_delete),
                    }),
                    Constraint::Check { expression } => json!({
                        "type": "CHECK",
                        "expression": expression,
                    }),
                    Constraint::MinLength(len) => json!({
                        "type": "MIN_LENGTH",
                        "value": len,
                    }),
                    Constraint::MaxLength(len) => json!({
                        "type": "MAX_LENGTH",
                        "value": len,
                    }),
                    Constraint::MinValue(val) => json!({
                        "type": "MIN_VALUE",
                        "value": val,
                    }),
                    Constraint::MaxValue(val) => json!({
                        "type": "MAX_VALUE",
                        "value": val,
                    }),
                    Constraint::Pattern(pattern) => json!({
                        "type": "PATTERN",
                        "value": pattern,
                    }),
                    Constraint::In(values) => json!({
                        "type": "IN",
                        "values": values,
                    }),
                })
                .collect();

            Ok(IntrospectionResult {
                query_type: "DescribeNodeType".to_string(),
                data: json!({
                    "label": node_type.label,
                    "description": node_type.description,
                    "properties": properties,
                    "constraints": constraints,
                    "is_abstract": node_type.is_abstract,
                    "extends": node_type.extends,
                }),
                metadata: Some(json!({
                    "graph_type": graph_type,
                    "property_count": properties.len(),
                    "constraint_count": constraints.len(),
                })),
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                graph_type
            )))
        }
    }

    /// Describe a specific edge type with all its properties
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn describe_edge_type(
        &self,
        graph_type: &str,
        edge_label: &str,
    ) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                json!({ "name": graph_type }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
            })?;

        if let Some(data) = response.data() {
            let graph_type_def: GraphTypeDefinition = serde_json::from_value(data.clone())
                .map_err(|e| {
                    ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                })?;

            let edge_type = graph_type_def
                .edge_types
                .iter()
                .find(|et| et.type_name == edge_label)
                .ok_or_else(|| {
                    ExecutionError::SchemaValidation(format!(
                        "Edge type '{}' not found in graph type '{}'",
                        edge_label, graph_type
                    ))
                })?;

            let properties: Vec<JsonValue> = edge_type
                .properties
                .iter()
                .map(|p| {
                    json!({
                        "name": p.name,
                        "data_type": format!("{:?}", p.data_type),
                        "is_required": p.required,
                        "is_unique": p.unique,
                        "default_value": p.default_value,
                        "description": p.description,
                    })
                })
                .collect();

            Ok(IntrospectionResult {
                query_type: "DescribeEdgeType".to_string(),
                data: json!({
                    "label": edge_type.type_name,
                    "description": edge_type.description,
                    "properties": properties,
                    "source_node_types": edge_type.from_node_types,
                    "target_node_types": edge_type.to_node_types,
                    "cardinality": edge_type.cardinality,
                }),
                metadata: Some(json!({
                    "graph_type": graph_type,
                    "property_count": properties.len(),
                })),
            })
        } else {
            Err(ExecutionError::SchemaValidation(format!(
                "Graph type '{}' not found",
                graph_type
            )))
        }
    }

    /// Get version history for a graph type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn get_version_history(&self, graph_type: &str) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        let response = catalog
            .query_read_only(
                "graph_type",
                QueryType::ListVersions,
                json!({ "name": graph_type }),
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get version history: {}", e))
            })?;

        Ok(IntrospectionResult {
            query_type: "GetVersionHistory".to_string(),
            data: response.data().cloned().unwrap_or(json!([])),
            metadata: Some(json!({
                "graph_type": graph_type,
                "version_count": response.data().and_then(|d| d.as_array()).map(|a| a.len()).unwrap_or(0)
            })),
        })
    }

    /// Get schema statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema introspection for graph type DDL (see ROADMAP.md §4)
    fn get_schema_statistics(
        &self,
        graph_type: Option<&str>,
    ) -> Result<IntrospectionResult, ExecutionError> {
        let catalog = self.catalog_manager.read();

        if let Some(gt_name) = graph_type {
            // Statistics for a specific graph type
            let response = catalog
                .query_read_only(
                    "graph_type",
                    QueryType::GetGraphType,
                    json!({ "name": gt_name }),
                )
                .map_err(|e| {
                    ExecutionError::CatalogError(format!("Failed to get graph type: {}", e))
                })?;

            if let Some(data) = response.data() {
                let graph_type_def: GraphTypeDefinition = serde_json::from_value(data.clone())
                    .map_err(|e| {
                        ExecutionError::RuntimeError(format!("Failed to parse graph type: {}", e))
                    })?;

                let stats = json!({
                    "graph_type": gt_name,
                    "node_type_count": graph_type_def.node_types.len(),
                    "edge_type_count": graph_type_def.edge_types.len(),
                    "total_node_properties": graph_type_def.node_types.iter()
                        .map(|nt| nt.properties.len())
                        .sum::<usize>(),
                    "total_edge_properties": graph_type_def.edge_types.iter()
                        .map(|et| et.properties.len())
                        .sum::<usize>(),
                    "total_constraints": graph_type_def.node_types.iter()
                        .map(|nt| nt.constraints.len())
                        .sum::<usize>(),
                    "created_at": graph_type_def.created_at,
                    "last_modified": graph_type_def.updated_at,
                });

                Ok(IntrospectionResult {
                    query_type: "GetSchemaStatistics".to_string(),
                    data: stats,
                    metadata: None,
                })
            } else {
                Err(ExecutionError::SchemaValidation(format!(
                    "Graph type '{}' not found",
                    gt_name
                )))
            }
        } else {
            // Global statistics - all graph types
            let response = catalog
                .query_read_only("graph_type", QueryType::List, json!({}))
                .map_err(|e| {
                    ExecutionError::CatalogError(format!("Failed to list graph types: {}", e))
                })?;

            let stats = json!({
                "total_graph_types": response.data().and_then(|d| d.as_array()).map(|a| a.len()).unwrap_or(0),
                "timestamp": chrono::Utc::now(),
            });

            Ok(IntrospectionResult {
                query_type: "GetSchemaStatistics".to_string(),
                data: stats,
                metadata: None,
            })
        }
    }
}
