// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Schema validator implementation

use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::{CatalogResponse, QueryType};
use crate::schema::types::{Constraint, DataType, GraphTypeDefinition, NodeTypeDefinition};

/// Schema validation errors
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Unknown label: {0}")]
    UnknownLabel(String),

    #[error("Unknown property '{property}' for label '{label}'")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation error for undefined properties
    UnknownProperty { label: String, property: String },

    #[error("Missing required property '{property}' for label '{label}'")]
    MissingRequiredProperty { label: String, property: String },

    #[error("Invalid property type for '{property}': expected {expected}, got {got}")]
    InvalidPropertyType {
        property: String,
        expected: String,
        got: String,
    },

    #[error("Constraint violation: {constraint} - {message}")]
    ConstraintViolation { constraint: String, message: String },

    #[error("No graph type associated with graph '{0}'")]
    NoGraphType(String),

    #[error("Catalog error: {0}")]
    CatalogError(String),

    #[error("Invalid value: {0}")]
    InvalidValue(String),
}

/// Schema validator for validating nodes, edges, and indexes against graph type definitions
#[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation system for graph type enforcement
pub struct SchemaValidator {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Catalog integration for schema definitions
    catalog_manager: Arc<RwLock<CatalogManager>>,
    #[allow(dead_code)] // ROADMAP v0.4.0 - Flexible validation for schema evolution
    allow_unknown_properties: bool,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new(catalog_manager: Arc<RwLock<CatalogManager>>) -> Self {
        Self {
            catalog_manager,
            allow_unknown_properties: false, // Default to strict validation
        }
    }

    /// Create a new schema validator with custom configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Configurable validation for different use cases
    pub fn with_config(
        catalog_manager: Arc<RwLock<CatalogManager>>,
        allow_unknown_properties: bool,
    ) -> Self {
        Self {
            catalog_manager,
            allow_unknown_properties,
        }
    }

    /// Validate that a node matches the graph type schema (with graph name)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Node validation for graph type DDL enforcement at INSERT time
    pub fn validate_node(
        &self,
        graph_name: &str,
        label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ValidationError> {
        // Get graph type for graph
        let graph_type = self.get_graph_type_for_graph(graph_name)?;
        self.validate_node_with_type(&graph_type, label, properties)
    }

    /// Validate that a node matches the graph type schema (with graph type directly)
    pub fn validate_node_with_type(
        &self,
        graph_type: &GraphTypeDefinition,
        label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ValidationError> {
        // Find node type definition
        let node_type = graph_type
            .node_types
            .iter()
            .find(|nt| nt.label == label)
            .ok_or_else(|| ValidationError::UnknownLabel(label.to_string()))?;

        // Validate required properties
        for prop_def in &node_type.properties {
            if prop_def.required && !properties.contains_key(&prop_def.name) {
                return Err(ValidationError::MissingRequiredProperty {
                    label: label.to_string(),
                    property: prop_def.name.clone(),
                });
            }
        }

        // Validate property types
        for (prop_name, prop_value) in properties {
            if let Some(prop_def) = node_type.properties.iter().find(|p| p.name == *prop_name) {
                self.validate_property_type(prop_value, &prop_def.data_type)?;
            }
        }

        // Validate constraints
        self.validate_node_constraints(node_type)?;

        Ok(())
    }

    /// Validate partial node update (only validates provided properties)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Partial node validation for UPDATE/SET operations with graph type enforcement
    pub fn validate_partial_node(
        &self,
        graph_type: &GraphTypeDefinition,
        label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ValidationError> {
        // Find node type definition
        let node_type = graph_type
            .node_types
            .iter()
            .find(|nt| nt.label == label)
            .ok_or_else(|| ValidationError::UnknownLabel(label.to_string()))?;

        // Only validate the properties that are being updated
        for (prop_name, prop_value) in properties {
            if let Some(prop_def) = node_type.properties.iter().find(|p| p.name == *prop_name) {
                self.validate_property_type(prop_value, &prop_def.data_type)?;

                // Validate individual property constraints
                for constraint in &prop_def.constraints {
                    self.validate_property_constraint(prop_value, constraint)?;
                }
            } else if !self.allow_unknown_properties {
                return Err(ValidationError::UnknownProperty {
                    label: label.to_string(),
                    property: prop_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate that an edge matches the graph type schema
    #[allow(dead_code)] // ROADMAP v0.4.0 - Edge validation for graph type DDL enforcement (endpoint constraints, edge properties)
    pub fn validate_edge(
        &self,
        graph_type: &GraphTypeDefinition,
        edge_type: &str,
        from_label: &str,
        to_label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ValidationError> {
        // Find edge type definition
        let edge_def = graph_type
            .edge_types
            .iter()
            .find(|et| et.type_name == edge_type)
            .ok_or_else(|| ValidationError::UnknownLabel(edge_type.to_string()))?;

        // Validate from/to node types
        if !edge_def.from_node_types.contains(&from_label.to_string()) {
            return Err(ValidationError::InvalidValue(format!(
                "Edge type '{}' cannot originate from node type '{}'",
                edge_type, from_label
            )));
        }

        if !edge_def.to_node_types.contains(&to_label.to_string()) {
            return Err(ValidationError::InvalidValue(format!(
                "Edge type '{}' cannot terminate at node type '{}'",
                edge_type, to_label
            )));
        }

        // Validate required properties
        for prop_def in &edge_def.properties {
            if prop_def.required && !properties.contains_key(&prop_def.name) {
                return Err(ValidationError::MissingRequiredProperty {
                    label: edge_type.to_string(),
                    property: prop_def.name.clone(),
                });
            }
        }

        // Validate property types
        for (prop_name, prop_value) in properties {
            if let Some(prop_def) = edge_def.properties.iter().find(|p| p.name == *prop_name) {
                self.validate_property_type(prop_value, &prop_def.data_type)?;
            }
        }

        Ok(())
    }

    /// Validate that an index references valid schema elements
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index schema validation for CREATE INDEX on graph type properties
    pub fn validate_index_schema(
        &self,
        graph_name: &str,
        label: &str,
        properties: &[String],
    ) -> Result<(), ValidationError> {
        let graph_type = self.get_graph_type_for_graph(graph_name)?;

        // Check label exists
        let node_type = graph_type
            .node_types
            .iter()
            .find(|nt| nt.label == label)
            .ok_or_else(|| ValidationError::UnknownLabel(label.to_string()))?;

        // Check properties exist
        for property in properties {
            if !node_type.properties.iter().any(|p| p.name == *property) {
                return Err(ValidationError::UnknownProperty {
                    label: label.to_string(),
                    property: property.clone(),
                });
            }
        }

        Ok(())
    }

    /// Get the graph type definition for a graph
    #[allow(dead_code)] // ROADMAP v0.4.0 - Helper to resolve graph type from graph name for validation
    fn get_graph_type_for_graph(
        &self,
        graph_name: &str,
    ) -> Result<GraphTypeDefinition, ValidationError> {
        // Query catalog to get graph's associated graph type
        let catalog_manager = self.catalog_manager.read();

        // Get graph metadata to find the associated graph type name
        let graph_metadata = catalog_manager
            .query_read_only(
                "graph",
                QueryType::Get,
                serde_json::json!({ "name": graph_name }),
            )
            .map_err(|e| ValidationError::CatalogError(e.to_string()))?;

        // Extract graph type name from metadata
        let graph_type_name = match graph_metadata {
            CatalogResponse::Success { data: Some(data) } => data
                .get("graph_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| ValidationError::NoGraphType(graph_name.to_string()))?,
            CatalogResponse::Query { results } => results
                .get("graph_type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| ValidationError::NoGraphType(graph_name.to_string()))?,
            _ => {
                return Err(ValidationError::NoGraphType(graph_name.to_string()));
            }
        };

        // Get graph type definition
        let graph_type_response = catalog_manager
            .query_read_only(
                "graph_type",
                QueryType::GetGraphType,
                serde_json::json!({ "name": graph_type_name }),
            )
            .map_err(|e| ValidationError::CatalogError(e.to_string()))?;

        match graph_type_response {
            CatalogResponse::Success { data: Some(data) } => serde_json::from_value(data)
                .map_err(|e| ValidationError::CatalogError(e.to_string())),
            CatalogResponse::Query { results } => serde_json::from_value(results)
                .map_err(|e| ValidationError::CatalogError(e.to_string())),
            _ => Err(ValidationError::NoGraphType(graph_name.to_string())),
        }
    }

    /// Validate property type
    fn validate_property_type(
        &self,
        value: &Value,
        expected_type: &DataType,
    ) -> Result<(), ValidationError> {
        let valid = match (expected_type, value) {
            (DataType::String | DataType::Text, Value::String(_)) => true,
            (DataType::Integer, Value::Number(n)) => n.is_i64(),
            (DataType::BigInt, Value::Number(n)) => n.is_i64() || n.is_u64(),
            (DataType::Float | DataType::Double, Value::Number(n)) => n.is_f64() || n.is_i64(),
            (DataType::Boolean, Value::Bool(_)) => true,
            (DataType::Json, _) => true, // JSON can hold any value
            (DataType::Array(_), Value::Array(_)) => true, // TODO: Validate element types
            (DataType::UUID, Value::String(s)) => {
                // Basic UUID format validation
                s.len() == 36 && s.chars().filter(|c| *c == '-').count() == 4
            }
            _ => false,
        };

        if !valid {
            return Err(ValidationError::InvalidPropertyType {
                property: "".to_string(), // TODO: Pass property name through
                expected: format!("{:?}", expected_type),
                got: format!("{}", value),
            });
        }

        Ok(())
    }

    /// Validate node constraints
    #[allow(dead_code)] // ROADMAP v0.4.0 - Property constraint validation (UNIQUE, CHECK, RANGE) for graph types
    fn validate_property_constraint(
        &self,
        value: &Value,
        constraint: &Constraint,
    ) -> Result<(), ValidationError> {
        match constraint {
            Constraint::MinLength(min) => {
                if let Value::String(s) = value {
                    if s.len() < *min {
                        return Err(ValidationError::ConstraintViolation {
                            constraint: "MinLength".to_string(),
                            message: format!(
                                "String length {} is less than minimum {}",
                                s.len(),
                                min
                            ),
                        });
                    }
                }
            }
            Constraint::MaxLength(max) => {
                if let Value::String(s) = value {
                    if s.len() > *max {
                        return Err(ValidationError::ConstraintViolation {
                            constraint: "MaxLength".to_string(),
                            message: format!("String length {} exceeds maximum {}", s.len(), max),
                        });
                    }
                }
            }
            Constraint::Pattern(pattern) => {
                if let Value::String(s) = value {
                    let regex = regex::Regex::new(pattern)
                        .map_err(|e| ValidationError::InvalidValue(e.to_string()))?;
                    if !regex.is_match(s) {
                        return Err(ValidationError::ConstraintViolation {
                            constraint: "Pattern".to_string(),
                            message: format!("Value '{}' does not match pattern '{}'", s, pattern),
                        });
                    }
                }
            }
            Constraint::MinValue(min) => {
                if let Value::Number(n) = value {
                    if let Some(num_val) = n.as_f64() {
                        if num_val < *min {
                            return Err(ValidationError::ConstraintViolation {
                                constraint: "MinValue".to_string(),
                                message: format!("Value {} is less than minimum {}", num_val, min),
                            });
                        }
                    }
                }
            }
            Constraint::MaxValue(max) => {
                if let Value::Number(n) = value {
                    if let Some(num_val) = n.as_f64() {
                        if num_val > *max {
                            return Err(ValidationError::ConstraintViolation {
                                constraint: "MaxValue".to_string(),
                                message: format!("Value {} exceeds maximum {}", num_val, max),
                            });
                        }
                    }
                }
            }
            Constraint::Unique => {
                // Unique constraint is handled at storage level
            }
            _ => {
                // Other constraints are handled at different levels
            }
        }
        Ok(())
    }

    fn validate_node_constraints(
        &self,
        node_type: &NodeTypeDefinition,
    ) -> Result<(), ValidationError> {
        for constraint in &node_type.constraints {
            match constraint {
                Constraint::NotNull => {
                    // Already handled in required property validation
                }
                Constraint::Unique => {
                    // TODO: Check uniqueness against existing data
                }
                Constraint::MinLength(_) => {
                    // TODO: Validate string lengths
                }
                Constraint::MaxLength(_) => {
                    // TODO: Validate string lengths
                }
                Constraint::Pattern(_) => {
                    // TODO: Validate against regex pattern
                }
                _ => {
                    // TODO: Implement other constraint validations
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_validate_property_type() {
        // TODO: Add comprehensive tests for property type validation
    }

    #[test]
    fn test_validate_node_constraints() {
        // TODO: Add tests for constraint validation
    }
}
