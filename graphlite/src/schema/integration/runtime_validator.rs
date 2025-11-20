// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Runtime validator for INSERT/UPDATE operations
// Follows the synchronous pattern used by StorageManager and CatalogManager

use parking_lot::RwLock;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::manager::CatalogManager;
use crate::catalog::operations::QueryType;
use crate::exec::ExecutionError;
use crate::schema::enforcement::config::SchemaEnforcementConfig;
use crate::schema::types::GraphTypeDefinition;
use crate::schema::types::SchemaEnforcementMode;
use crate::schema::validator::SchemaValidator;

/// Runtime validator that hooks into query execution
/// Uses synchronous operations using synchronous operations
pub struct RuntimeValidator {
    catalog_manager: Arc<RwLock<CatalogManager>>,
    schema_validator: SchemaValidator,
    enforcement_config: SchemaEnforcementConfig,
}

impl RuntimeValidator {
    /// Create a new runtime validator
    pub fn new(catalog_manager: Arc<RwLock<CatalogManager>>) -> Self {
        let schema_validator = SchemaValidator::new(catalog_manager.clone());
        Self {
            catalog_manager,
            schema_validator,
            enforcement_config: SchemaEnforcementConfig::default(),
        }
    }

    /// Create with specific enforcement configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Configurable runtime validation for different enforcement modes
    pub fn with_config(
        catalog_manager: Arc<RwLock<CatalogManager>>,
        config: SchemaEnforcementConfig,
    ) -> Self {
        let schema_validator = SchemaValidator::new(catalog_manager.clone());
        Self {
            catalog_manager,
            schema_validator,
            enforcement_config: config,
        }
    }

    /// Validate INSERT operation before execution (synchronous)
    pub fn validate_insert(
        &self,
        graph_name: &str,
        label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ExecutionError> {
        // Check if validation is enabled
        if !self.enforcement_config.validate_on_write {
            return Ok(());
        }

        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => {
                // No schema defined, check enforcement mode
                match self.enforcement_config.mode {
                    SchemaEnforcementMode::Strict => {
                        return Err(ExecutionError::SchemaValidation(format!(
                            "No schema defined for graph '{}'",
                            graph_name
                        )));
                    }
                    SchemaEnforcementMode::Advisory => {
                        log::warn!(
                            "No schema defined for graph '{}', skipping validation",
                            graph_name
                        );
                        return Ok(());
                    }
                    SchemaEnforcementMode::Disabled => return Ok(()),
                }
            }
            Err(e) => return Err(e),
        };

        // Validate the node
        match self
            .schema_validator
            .validate_node_with_type(&graph_type, label, properties)
        {
            Ok(()) => Ok(()),
            Err(validation_error) => match self.enforcement_config.mode {
                SchemaEnforcementMode::Strict => Err(ExecutionError::SchemaValidation(
                    validation_error.to_string(),
                )),
                SchemaEnforcementMode::Advisory => {
                    log::warn!("Schema validation warning: {}", validation_error);
                    Ok(())
                }
                SchemaEnforcementMode::Disabled => Ok(()),
            },
        }
    }

    /// Validate UPDATE operation before execution (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn validate_update(
        &self,
        graph_name: &str,
        label: &str,
        properties: &HashMap<String, Value>,
        is_partial: bool,
    ) -> Result<(), ExecutionError> {
        // Check if validation is enabled
        if !self.enforcement_config.validate_on_write {
            return Ok(());
        }

        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => {
                // No schema defined, check enforcement mode
                match self.enforcement_config.mode {
                    SchemaEnforcementMode::Strict => {
                        return Err(ExecutionError::SchemaValidation(format!(
                            "No schema defined for graph '{}'",
                            graph_name
                        )));
                    }
                    _ => return Ok(()),
                }
            }
            Err(e) => return Err(e),
        };

        // For partial updates, only validate the properties being updated
        if is_partial {
            match self
                .schema_validator
                .validate_partial_node(&graph_type, label, properties)
            {
                Ok(()) => Ok(()),
                Err(validation_error) => match self.enforcement_config.mode {
                    SchemaEnforcementMode::Strict => Err(ExecutionError::SchemaValidation(
                        validation_error.to_string(),
                    )),
                    SchemaEnforcementMode::Advisory => {
                        log::warn!("Schema validation warning: {}", validation_error);
                        Ok(())
                    }
                    SchemaEnforcementMode::Disabled => Ok(()),
                },
            }
        } else {
            // Full update, validate all properties
            self.validate_insert(graph_name, label, properties)
        }
    }

    /// Validate edge creation before execution (synchronous)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn validate_edge_insert(
        &self,
        graph_name: &str,
        edge_type: &str,
        from_label: &str,
        to_label: &str,
        properties: &HashMap<String, Value>,
    ) -> Result<(), ExecutionError> {
        // Check if validation is enabled
        if !self.enforcement_config.validate_on_write {
            return Ok(());
        }

        // Get the graph type for validation
        let graph_type = match self.get_graph_type(graph_name) {
            Ok(Some(gt)) => gt,
            Ok(None) => match self.enforcement_config.mode {
                SchemaEnforcementMode::Strict => {
                    return Err(ExecutionError::SchemaValidation(format!(
                        "No schema defined for graph '{}'",
                        graph_name
                    )));
                }
                _ => return Ok(()),
            },
            Err(e) => return Err(e),
        };

        // Validate the edge
        match self.schema_validator.validate_edge(
            &graph_type,
            edge_type,
            from_label,
            to_label,
            properties,
        ) {
            Ok(()) => Ok(()),
            Err(validation_error) => match self.enforcement_config.mode {
                SchemaEnforcementMode::Strict => Err(ExecutionError::SchemaValidation(
                    validation_error.to_string(),
                )),
                SchemaEnforcementMode::Advisory => {
                    log::warn!("Schema validation warning: {}", validation_error);
                    Ok(())
                }
                SchemaEnforcementMode::Disabled => Ok(()),
            },
        }
    }

    /// Get the graph type definition for a graph (synchronous)
    fn get_graph_type(
        &self,
        graph_name: &str,
    ) -> Result<Option<GraphTypeDefinition>, ExecutionError> {
        // Try to get the graph type from the catalog
        let catalog_manager = self.catalog_manager.read();

        // First, try to get the graph metadata to find its type
        match catalog_manager.query_read_only(
            "graph",
            QueryType::GetGraph,
            json!({ "name": graph_name }),
        ) {
            Ok(response) => {
                // Extract graph type name from response
                if let Some(data) = response.data() {
                    if let Some(graph_type_name) = data.get("graph_type").and_then(|v| v.as_str()) {
                        // Now get the graph type definition
                        match catalog_manager.query_read_only(
                            "graph_type",
                            QueryType::GetGraphType,
                            json!({ "name": graph_type_name }),
                        ) {
                            Ok(type_response) => {
                                // Parse the graph type definition from the response
                                if let Some(type_data) = type_response.data() {
                                    match serde_json::from_value::<GraphTypeDefinition>(
                                        type_data.clone(),
                                    ) {
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

    /// Update enforcement configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn set_enforcement_mode(&mut self, mode: SchemaEnforcementMode) {
        self.enforcement_config.mode = mode;
    }

    /// Set validation on write
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn set_validate_on_write(&mut self, enabled: bool) {
        self.enforcement_config.validate_on_write = enabled;
    }

    /// Set validation on read
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn set_validate_on_read(&mut self, enabled: bool) {
        self.enforcement_config.validate_on_read = enabled;
    }

    /// Get current enforcement configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema validation for graph type DDL (see ROADMAP.md §4)
    pub fn get_config(&self) -> &SchemaEnforcementConfig {
        &self.enforcement_config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_validator_creation() {
        // Test will be implemented with proper setup
    }

    #[test]
    fn test_enforcement_modes() {
        // Test different enforcement modes
        let config_strict = SchemaEnforcementConfig {
            mode: SchemaEnforcementMode::Strict,
            validate_on_write: true,
            validate_on_read: false,
            allow_unknown_properties: false,
            auto_create_indexes: false,
            log_warnings: true,
            allow_schema_drift: false,
        };

        assert_eq!(config_strict.mode, SchemaEnforcementMode::Strict);
        assert!(config_strict.validate_on_write);
    }
}
