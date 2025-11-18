// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Ingestion schema validator - validates data during bulk ingestion
// Follows the synchronous pattern used by StorageManager and CatalogManager

use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashMap;
use serde_json::{Value, json};

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::QueryType;
use crate::schema::validator::{SchemaValidator};
use crate::schema::enforcement::config::SchemaEnforcementConfig;
use crate::schema::types::SchemaEnforcementMode;
use crate::schema::types::GraphTypeDefinition;
use crate::exec::ExecutionError;

/// Validates data during ingestion operations
/// Uses synchronous operations using synchronous operations
#[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion pipelines
pub struct IngestionSchemaValidator {
    catalog_manager: Arc<RwLock<CatalogManager>>,
    schema_validator: SchemaValidator,
    enforcement_config: SchemaEnforcementConfig,
    validation_stats: ValidationStats,
}

#[derive(Debug, Default, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Validation statistics tracking for ingestion monitoring
pub struct ValidationStats {
    pub total_records: usize,
    pub valid_records: usize,
    pub invalid_records: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl IngestionSchemaValidator {
    /// Create a new ingestion schema validator
    #[allow(dead_code)] // ROADMAP v0.4.0 - Ingestion validator constructor for schema enforcement
    pub fn new(catalog_manager: Arc<RwLock<CatalogManager>>) -> Self {
        let schema_validator = SchemaValidator::new(catalog_manager.clone());
        Self {
            catalog_manager,
            schema_validator,
            enforcement_config: SchemaEnforcementConfig::default(),
            validation_stats: ValidationStats::default(),
        }
    }

    /// Create with specific enforcement configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn with_config(
        catalog_manager: Arc<RwLock<CatalogManager>>,
        config: SchemaEnforcementConfig,
    ) -> Self {
        let schema_validator = SchemaValidator::new(catalog_manager.clone());
        Self {
            catalog_manager,
            schema_validator,
            enforcement_config: config,
            validation_stats: ValidationStats::default(),
        }
    }

    /// Validate a batch of nodes during ingestion (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn validate_node_batch(
        &mut self,
        graph_name: &str,
        nodes: &[(String, HashMap<String, Value>)], // (label, properties)
    ) -> Result<Vec<bool>, ExecutionError> {
        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name)? {
            Some(gt) => gt,
            None => {
                // No schema defined
                match self.enforcement_config.mode {
                    SchemaEnforcementMode::Strict => {
                        return Err(ExecutionError::SchemaValidation(
                            format!("No schema defined for graph '{}' - cannot ingest with strict enforcement", graph_name)
                        ));
                    }
                    _ => {
                        // Allow all records without validation
                        self.validation_stats.total_records += nodes.len();
                        self.validation_stats.valid_records += nodes.len();
                        return Ok(vec![true; nodes.len()]);
                    }
                }
            }
        };

        let mut results = Vec::new();

        for (label, properties) in nodes {
            self.validation_stats.total_records += 1;

            match self.schema_validator.validate_node_with_type(&graph_type, label, properties) {
                Ok(()) => {
                    self.validation_stats.valid_records += 1;
                    results.push(true);
                }
                Err(validation_error) => {
                    match self.enforcement_config.mode {
                        SchemaEnforcementMode::Strict => {
                            self.validation_stats.invalid_records += 1;
                            self.validation_stats.errors.push(format!(
                                "Node with label '{}': {}",
                                label, validation_error
                            ));
                            results.push(false);
                        }
                        SchemaEnforcementMode::Advisory => {
                            self.validation_stats.warnings.push(format!(
                                "Node with label '{}': {}",
                                label, validation_error
                            ));
                            self.validation_stats.valid_records += 1;
                            results.push(true);
                        }
                        SchemaEnforcementMode::Disabled => {
                            self.validation_stats.valid_records += 1;
                            results.push(true);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Validate a batch of edges during ingestion (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn validate_edge_batch(
        &mut self,
        graph_name: &str,
        edges: &[(String, String, String, HashMap<String, Value>)], // (edge_type, from_label, to_label, properties)
    ) -> Result<Vec<bool>, ExecutionError> {
        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name)? {
            Some(gt) => gt,
            None => {
                match self.enforcement_config.mode {
                    SchemaEnforcementMode::Strict => {
                        return Err(ExecutionError::SchemaValidation(
                            format!("No schema defined for graph '{}' - cannot ingest with strict enforcement", graph_name)
                        ));
                    }
                    _ => {
                        // Allow all records without validation
                        self.validation_stats.total_records += edges.len();
                        self.validation_stats.valid_records += edges.len();
                        return Ok(vec![true; edges.len()]);
                    }
                }
            }
        };

        let mut results = Vec::new();

        for (edge_type, from_label, to_label, properties) in edges {
            self.validation_stats.total_records += 1;

            match self.schema_validator.validate_edge(&graph_type, edge_type, from_label, to_label, properties) {
                Ok(()) => {
                    self.validation_stats.valid_records += 1;
                    results.push(true);
                }
                Err(validation_error) => {
                    match self.enforcement_config.mode {
                        SchemaEnforcementMode::Strict => {
                            self.validation_stats.invalid_records += 1;
                            self.validation_stats.errors.push(format!(
                                "Edge '{}' from '{}' to '{}': {}",
                                edge_type, from_label, to_label, validation_error
                            ));
                            results.push(false);
                        }
                        SchemaEnforcementMode::Advisory => {
                            self.validation_stats.warnings.push(format!(
                                "Edge '{}' from '{}' to '{}': {}",
                                edge_type, from_label, to_label, validation_error
                            ));
                            self.validation_stats.valid_records += 1;
                            results.push(true);
                        }
                        SchemaEnforcementMode::Disabled => {
                            self.validation_stats.valid_records += 1;
                            results.push(true);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get validation statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn get_stats(&self) -> &ValidationStats {
        &self.validation_stats
    }

    /// Reset validation statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn reset_stats(&mut self) {
        self.validation_stats = ValidationStats::default();
    }

    /// Get a summary of validation results
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn get_validation_summary(&self) -> String {
        let stats = &self.validation_stats;
        let success_rate = if stats.total_records > 0 {
            (stats.valid_records as f64 / stats.total_records as f64) * 100.0
        } else {
            100.0
        };

        format!(
            "Ingestion Validation Summary:\n\
             Total Records: {}\n\
             Valid Records: {} ({:.2}%)\n\
             Invalid Records: {}\n\
             Warnings: {}\n\
             Errors: {}",
            stats.total_records,
            stats.valid_records,
            success_rate,
            stats.invalid_records,
            stats.warnings.len(),
            stats.errors.len()
        )
    }

    /// Set enforcement mode
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    pub fn set_enforcement_mode(&mut self, mode: SchemaEnforcementMode) {
        self.enforcement_config.mode = mode;
    }

    /// Get the graph type definition for a graph (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for data ingestion (see ROADMAP.md §4)
    fn get_graph_type(&self, graph_name: &str) -> Result<Option<GraphTypeDefinition>, ExecutionError> {
        // Try to get the graph type from the catalog
        let catalog_manager = self.catalog_manager.read();

        // First, try to get the graph metadata to find its type
        match catalog_manager.query_read_only("graph", QueryType::GetGraph, json!({ "name": graph_name })) {
            Ok(response) => {
                // Extract graph type name from response
                if let Some(data) = response.data() {
                    if let Some(graph_type_name) = data.get("graph_type").and_then(|v| v.as_str()) {
                        // Now get the graph type definition
                        match catalog_manager.query_read_only(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_stats() {
        let mut stats = ValidationStats::default();
        stats.total_records = 100;
        stats.valid_records = 95;
        stats.invalid_records = 5;

        assert_eq!(stats.total_records, 100);
        assert_eq!(stats.valid_records, 95);
        assert_eq!(stats.invalid_records, 5);
    }
}
