// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// GraphTypeCatalog - Catalog provider for ISO GQL Graph Type definitions

use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;

use crate::catalog::error::{CatalogError, CatalogResult};
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::catalog::traits::{CatalogProvider, CatalogSchema};
use crate::schema::types::GraphTypeDefinition;
use crate::storage::StorageManager;

/// GraphTypeCatalog manages graph type definitions persistently
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphTypeCatalog {
    /// Map of graph type name to its definition
    graph_types: HashMap<String, GraphTypeDefinition>,

    /// Storage manager reference (not serialized)
    #[serde(skip)]
    storage: Option<Arc<StorageManager>>,
}

impl GraphTypeCatalog {
    /// Create a new empty GraphTypeCatalog
    pub fn new() -> Self {
        Self {
            graph_types: HashMap::new(),
            storage: None,
        }
    }

    /// List all graph types
    fn list_graph_types(&self) -> CatalogResult<CatalogResponse> {
        let types: Vec<serde_json::Value> = self
            .graph_types
            .iter()
            .map(|(name, def)| {
                serde_json::json!({
                    "name": name,
                    "version": def.version.to_string(),
                    "node_types": def.node_types.len(),
                    "edge_types": def.edge_types.len(),
                    "created_at": def.created_at,
                    "created_by": def.created_by,
                })
            })
            .collect();

        Ok(CatalogResponse::list(types))
    }

    /// Get a specific graph type
    fn get_graph_type(&self, name: &str) -> CatalogResult<CatalogResponse> {
        match self.graph_types.get(name) {
            Some(graph_type) => {
                // Serialize the graph type - version is already part of the struct
                let response = serde_json::to_value(graph_type)
                    .map_err(|e| CatalogError::SerializationError(e.to_string()))?;

                Ok(CatalogResponse::success_with_data(response))
            }
            None => Err(CatalogError::NotFound(format!(
                "Graph type '{}' not found",
                name
            ))),
        }
    }

    /// Describe a graph type (detailed information)
    #[allow(dead_code)] // ROADMAP v0.4.0 - DESCRIBE GRAPH TYPE command for schema introspection
    fn describe_graph_type(&self, name: &str) -> CatalogResult<CatalogResponse> {
        match self.graph_types.get(name) {
            Some(graph_type) => {
                let description = serde_json::json!({
                    "name": graph_type.name,
                    "version": graph_type.version.to_string(),
                    "previous_version": graph_type.previous_version.as_ref().map(|v| v.to_string()),
                    "description": graph_type.description,
                    "created_at": graph_type.created_at,
                    "updated_at": graph_type.updated_at,
                    "created_by": graph_type.created_by,
                    "node_types": graph_type.node_types.iter().map(|nt| {
                        serde_json::json!({
                            "label": nt.label,
                            "properties": nt.properties.len(),
                            "constraints": nt.constraints.len(),
                            "is_abstract": nt.is_abstract,
                            "extends": nt.extends,
                        })
                    }).collect::<Vec<_>>(),
                    "edge_types": graph_type.edge_types.iter().map(|et| {
                        serde_json::json!({
                            "type_name": et.type_name,
                            "from_node_types": et.from_node_types,
                            "to_node_types": et.to_node_types,
                            "properties": et.properties.len(),
                            "constraints": et.constraints.len(),
                        })
                    }).collect::<Vec<_>>(),
                });

                Ok(CatalogResponse::query(description))
            }
            None => Err(CatalogError::NotFound(format!(
                "Graph type '{}' not found",
                name
            ))),
        }
    }

    /// Check if a graph type exists
    fn exists(&self, name: &str) -> CatalogResult<CatalogResponse> {
        let exists = self.graph_types.contains_key(name);
        Ok(CatalogResponse::success_with_data(serde_json::json!({
            "exists": exists,
            "graph_type": name,
        })))
    }
}

impl CatalogProvider for GraphTypeCatalog {
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()> {
        self.storage = Some(storage.clone());

        // Try to load existing catalog from storage
        if let Ok(Some(data)) = storage.load_catalog_provider("graph_type") {
            if !data.is_empty() {
                self.load(&data)?;
            }
        }

        Ok(())
    }

    fn execute(&mut self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Create {
                entity_type,
                name,
                params,
            } => {
                match entity_type {
                    EntityType::GraphType => {
                        let graph_type: GraphTypeDefinition = serde_json::from_value(params)
                            .map_err(|e| {
                                CatalogError::InvalidOperation(format!(
                                    "Invalid graph type definition: {}",
                                    e
                                ))
                            })?;

                        // Check if graph type already exists
                        if self.graph_types.contains_key(&name) {
                            return Err(CatalogError::DuplicateEntry(format!(
                                "Graph type '{}' already exists",
                                name
                            )));
                        }

                        // Store the graph type
                        self.graph_types.insert(name.clone(), graph_type);

                        // Persist to storage
                        if let Some(storage) = &self.storage {
                            storage.save_catalog_provider("graph_type", &self.save()?)?;
                        }

                        Ok(CatalogResponse::success_with_data(serde_json::json!({
                            "name": name,
                            "message": "Graph type created successfully"
                        })))
                    }
                    _ => Err(CatalogError::InvalidOperation(format!(
                        "GraphTypeCatalog does not support creating {:?}",
                        entity_type
                    ))),
                }
            }

            CatalogOperation::Drop {
                entity_type, name, ..
            } => {
                match entity_type {
                    EntityType::GraphType => {
                        if !self.graph_types.contains_key(&name) {
                            return Err(CatalogError::NotFound(format!(
                                "Graph type '{}' not found",
                                name
                            )));
                        }

                        // Remove the graph type
                        self.graph_types.remove(&name);

                        // Persist to storage
                        if let Some(storage) = &self.storage {
                            storage.save_catalog_provider("graph_type", &self.save()?)?;
                        }

                        Ok(CatalogResponse::success_with_data(serde_json::json!({
                            "name": name,
                            "message": format!("Graph type '{}' dropped successfully", name)
                        })))
                    }
                    _ => Err(CatalogError::InvalidOperation(format!(
                        "GraphTypeCatalog does not support dropping {:?}",
                        entity_type
                    ))),
                }
            }

            CatalogOperation::Query { query_type, params } => {
                self.execute_read_only(CatalogOperation::Query { query_type, params })
            }

            CatalogOperation::List {
                entity_type,
                filters: _,
            } => match entity_type {
                EntityType::GraphType => self.list_graph_types(),
                _ => Err(CatalogError::InvalidOperation(format!(
                    "GraphTypeCatalog does not support listing {:?}",
                    entity_type
                ))),
            },

            _ => Err(CatalogError::NotSupported("Operation not supported by GraphTypeCatalog".to_string())),
        }
    }

    fn execute_read_only(&self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Query { query_type, params } => match query_type {
                QueryType::List => self.list_graph_types(),

                QueryType::Get | QueryType::GetGraphType => {
                    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                        CatalogError::InvalidOperation(
                            "Missing 'name' parameter for Get query".to_string(),
                        )
                    })?;
                    self.get_graph_type(name)
                }

                QueryType::Exists => {
                    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                        CatalogError::InvalidOperation(
                            "Missing 'name' parameter for Exists query".to_string(),
                        )
                    })?;
                    self.exists(name)
                }

                _ => Err(CatalogError::NotSupported(format!(
                    "Query type {:?} not supported",
                    query_type
                ))),
            },

            CatalogOperation::List {
                entity_type,
                filters: _,
            } => match entity_type {
                EntityType::GraphType => self.list_graph_types(),
                _ => Err(CatalogError::InvalidOperation(format!(
                    "GraphTypeCatalog does not support listing {:?}",
                    entity_type
                ))),
            },

            _ => Err(CatalogError::InvalidOperation(
                "Only query and list operations are supported in read-only mode".to_string(),
            )),
        }
    }

    fn save(&self) -> CatalogResult<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| {
            CatalogError::SerializationError(format!("Failed to serialize GraphTypeCatalog: {}", e))
        })
    }

    fn load(&mut self, data: &[u8]) -> CatalogResult<()> {
        let loaded: GraphTypeCatalog = serde_json::from_slice(data).map_err(|e| {
            CatalogError::SerializationError(format!(
                "Failed to deserialize GraphTypeCatalog: {}",
                e
            ))
        })?;

        self.graph_types = loaded.graph_types;

        Ok(())
    }

    fn schema(&self) -> CatalogSchema {
        CatalogSchema {
            name: "GraphTypeCatalog".to_string(),
            version: "1.0.0".to_string(),
            entities: vec!["GraphType".to_string()],
            operations: vec![
                "Create GraphType".to_string(),
                "Drop GraphType".to_string(),
                "Drop GraphType Version".to_string(),
                "Drop All Versions (cascade)".to_string(),
                "List GraphTypes".to_string(),
                "Get GraphType (latest)".to_string(),
                "Get GraphType Version".to_string(),
                "List GraphType Versions".to_string(),
                "Describe GraphType".to_string(),
                "Get Versions".to_string(),
                "Check Exists".to_string(),
            ],
        }
    }

    fn supported_operations(&self) -> Vec<String> {
        vec![
            "Create GraphType".to_string(),
            "Drop GraphType".to_string(),
            "List GraphTypes".to_string(),
            "Get GraphType".to_string(),
            "Describe GraphType".to_string(),
            "Get Versions".to_string(),
            "Check Exists".to_string(),
        ]
    }
}
