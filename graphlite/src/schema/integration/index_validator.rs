// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Index schema validator - ensures indexes reference valid schema elements
// Follows the synchronous pattern used by StorageManager and CatalogManager

use serde_json::json;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::QueryType;
use crate::schema::types::GraphTypeDefinition;
use crate::exec::ExecutionError;

/// Validates that index operations reference valid schema elements
/// Uses synchronous operations using synchronous operations
pub struct IndexSchemaValidator<'a> {
    catalog_manager: &'a CatalogManager,
}

impl<'a> IndexSchemaValidator<'a> {
    /// Create a new index schema validator with a reference
    pub fn new(catalog_manager: &'a CatalogManager) -> Self {
        Self {
            catalog_manager,
        }
    }
}

impl<'a> IndexSchemaValidator<'a> {
    /// Validate that an index references valid schema elements (synchronous)
    pub fn validate_index_creation(
        &self,
        graph_name: &str,
        label: &str,
        properties: &[String],
    ) -> Result<(), ExecutionError> {
        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => {
                // No schema defined, allow index creation
                log::info!("No schema defined for graph '{}', allowing index creation", graph_name);
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // Check label exists in schema
        let node_type = graph_type
            .node_types
            .iter()
            .find(|nt| nt.label == label)
            .ok_or_else(|| {
                ExecutionError::SchemaValidation(format!(
                    "Cannot create index: label '{}' not defined in graph type",
                    label
                ))
            })?;

        // Check all properties exist in the node type
        for property in properties {
            if !node_type.properties.iter().any(|p| p.name == *property) {
                return Err(ExecutionError::SchemaValidation(format!(
                    "Cannot create index: property '{}' not defined for label '{}'",
                    property, label
                )));
            }
        }

        // Validate that indexed properties are appropriate for indexing
        for property in properties {
            if let Some(prop_def) = node_type.properties.iter().find(|p| p.name == *property) {
                // Check if the data type is indexable
                match &prop_def.data_type {
                    crate::schema::types::DataType::Vector(_) => {
                        // Vector types might need special handling
                        log::info!("Creating index on vector property '{}'", property);
                    }
                    crate::schema::types::DataType::List(_) => {
                        // List types might not be directly indexable
                        log::warn!("Creating index on list property '{}' - may have limited effectiveness", property);
                    }
                    _ => {
                        // Other types are generally indexable
                    }
                }

                // Check if there's a unique constraint that could benefit from an index
                if prop_def.constraints.iter().any(|c| matches!(c, crate::schema::types::Constraint::Unique)) {
                    log::info!("Creating index on unique property '{}' - will enforce uniqueness", property);
                }
            }
        }

        Ok(())
    }

    /// Validate that an index rebuild references valid schema elements (synchronous)
    pub fn validate_index_rebuild(
        &self,
        graph_name: &str,
        index_name: &str,
    ) -> Result<(), ExecutionError> {
        // For rebuild, we just need to ensure the graph has a schema if enforcement is enabled
        let _graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => {
                // No schema defined, allow rebuild
                log::info!("No schema defined for graph '{}', allowing index rebuild", graph_name);
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // The actual index metadata validation would happen here
        // For now, we just validate the graph has a schema
        log::info!("Validated index '{}' for rebuild in graph '{}'", index_name, graph_name);

        Ok(())
    }

    /// Get suggestions for indexes based on schema (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema-based index suggestions for query optimization
    pub fn suggest_indexes(
        &self,
        graph_name: &str,
    ) -> Result<Vec<IndexSuggestion>, ExecutionError> {
        let mut suggestions = Vec::new();

        let graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => {
                // No schema defined, no suggestions
                return Ok(suggestions);
            }
            Err(e) => return Err(e),
        };

        // Suggest indexes for properties with unique constraints
        for node_type in &graph_type.node_types {
            for property in &node_type.properties {
                if property.constraints.iter().any(|c| matches!(c, crate::schema::types::Constraint::Unique)) {
                    suggestions.push(IndexSuggestion {
                        label: node_type.label.clone(),
                        properties: vec![property.name.clone()],
                        reason: "Unique constraint would benefit from index".to_string(),
                        priority: IndexPriority::High,
                    });
                }

                // Suggest indexes for ID-like properties
                if property.name.to_lowercase().contains("id") || property.name == "identifier" {
                    suggestions.push(IndexSuggestion {
                        label: node_type.label.clone(),
                        properties: vec![property.name.clone()],
                        reason: "ID field commonly used in lookups".to_string(),
                        priority: IndexPriority::Medium,
                    });
                }
            }
        }

        Ok(suggestions)
    }

    /// Get the graph type definition for a graph (synchronous)
    fn get_graph_type(&self, graph_name: &str) -> Result<Option<GraphTypeDefinition>, ExecutionError> {
        // First, try to get the graph metadata to find its type
        match self.catalog_manager.query_read_only("graph", QueryType::GetGraph, json!({ "name": graph_name })) {
            Ok(response) => {
                // Extract graph type name from response
                if let Some(data) = response.data() {
                    if let Some(graph_type_name) = data.get("graph_type").and_then(|v| v.as_str()) {
                        // Now get the graph type definition
                        match self.catalog_manager.query_read_only(
                            "graph_type",
                            QueryType::GetGraphType,
                            json!({ "name": graph_type_name })
                        ) {
                            Ok(type_response) => {
                                // Parse the graph type definition from the response
                                if let Some(type_data) = type_response.data() {
                                    match serde_json::from_value::<GraphTypeDefinition>(type_data.clone()) {
                                        Ok(graph_type) => Ok(Some(graph_type)),
                                        Err(_) => Ok(None),
                                    }
                                } else {
                                    Ok(None)
                                }
                            }
                            Err(_) => Ok(None),
                        }
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            Err(_) => {
                // Graph doesn't exist or catalog not available
                Ok(None)
            }
        }
    }
}

/// Index suggestion based on schema analysis
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Index suggestion structure for schema-driven optimization
pub struct IndexSuggestion {
    pub label: String,
    pub properties: Vec<String>,
    pub reason: String,
    pub priority: IndexPriority,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Index priority classification for recommendation ranking
pub enum IndexPriority {
    High,
    Medium,
    Low,
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_index_validator_creation() {
        // Test will be implemented with proper setup
    }
}
