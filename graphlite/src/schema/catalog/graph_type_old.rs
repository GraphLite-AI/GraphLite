// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// GraphTypeCatalog - Catalog provider for ISO GQL Graph Type definitions

use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::catalog::traits::{CatalogProvider, CatalogSchema};
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::catalog::error::{CatalogError, CatalogResult};
use crate::storage::StorageManager;
use crate::schema::types::{GraphTypeDefinition, GraphTypeVersion};

/// GraphTypeCatalog manages graph type definitions persistently
#[derive(Debug, Serialize, Deserialize)]
pub struct GraphTypeCatalog {
    /// Map of graph type name to its definition
    graph_types: HashMap<String, GraphTypeDefinition>,

    /// Map of graph type name to all its versions
    version_history: HashMap<String, Vec<GraphTypeVersion>>,

    /// Storage manager reference (not serialized)
    #[serde(skip)]
    storage: Option<Arc<StorageManager>>,
}

impl GraphTypeCatalog {
    /// Create a new empty GraphTypeCatalog
    pub fn new() -> Self {
        Self {
            graph_types: HashMap::new(),
            version_history: HashMap::new(),
            storage: None,
        }
    }

    /// Create a graph type
    fn create_graph_type(&mut self, name: String, params: serde_json::Value) -> CatalogResult<CatalogResponse> {
        let graph_type: GraphTypeDefinition = serde_json::from_value(params)
            .map_err(|e| CatalogError::InvalidOperation(format!("Invalid graph type definition: {}", e)))?;

        // Check if graph type already exists
        if self.graph_types.contains_key(&graph_type.name) {
            // Check if this is a new version
            let existing = &self.graph_types[&graph_type.name];
            if existing.version == graph_type.version {
                return Err(CatalogError::DuplicateEntity(
                    format!("Graph type '{}' version {} already exists",
                            graph_type.name, graph_type.version.to_string())
                ));
            }

            // This is a new version, add to version history
            if let Some(versions) = self.version_history.get_mut(&graph_type.name) {
                versions.push(graph_type.version.clone());
            }
        } else {
            // First version of this graph type
            self.version_history.insert(
                graph_type.name.clone(),
                vec![graph_type.version.clone()]
            );
        }

        let type_name = graph_type.name.clone();
        self.graph_types.insert(type_name.clone(), graph_type);

        // Persist to storage
        if let Some(storage) = &self.storage {
            storage.save_catalog("graph_type", &self.save()?)?;
        }

        Ok(CatalogResponse::success_with_data(
            serde_json::json!({
                "name": type_name,
                "message": "Graph type created successfully"
            })
        ))
    }

    /// Drop a graph type
    fn drop_graph_type(&mut self, name: &str) -> CatalogResult<CatalogResponse> {
        if !self.graph_types.contains_key(name) {
            return Err(CatalogError::NotFound(
                format!("Graph type '{}' not found", name)
            ));
        }

        self.graph_types.remove(name);
        self.version_history.remove(name);

        // Persist to storage
        if let Some(storage) = &self.storage {
            storage.save_catalog("graph_type", &self.save()?)?;
        }

        Ok(CatalogResponse::Deleted {
            entity_type: EntityType::Other("GraphType".to_string()),
            id: name.to_string(),
            message: format!("Graph type '{}' dropped successfully", name),
        })
    }

    /// List all graph types
    fn list_graph_types(&self) -> CatalogResult<CatalogResponse> {
        let types: Vec<serde_json::Value> = self.graph_types
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

        Ok(CatalogResponse::List {
            entity_type: EntityType::Other("GraphType".to_string()),
            items: types,
        })
    }

    /// Get a specific graph type
    fn get_graph_type(&self, name: &str) -> CatalogResult<CatalogResponse> {
        match self.graph_types.get(name) {
            Some(graph_type) => {
                let data = serde_json::to_value(graph_type)
                    .map_err(|e| CatalogError::SerializationError(e.to_string()))?;

                Ok(CatalogResponse::Data(data))
            }
            None => Err(CatalogError::NotFound(
                format!("Graph type '{}' not found", name)
            ))
        }
    }

    /// Describe a graph type (detailed information)
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
                    "version_history": self.version_history.get(name)
                        .map(|versions| versions.iter().map(|v| v.to_string()).collect::<Vec<_>>()),
                });

                Ok(CatalogResponse::Data(description))
            }
            None => Err(CatalogError::NotFound(
                format!("Graph type '{}' not found", name)
            ))
        }
    }

    /// Get graph type versions
    fn get_versions(&self, name: &str) -> CatalogResult<CatalogResponse> {
        match self.version_history.get(name) {
            Some(versions) => {
                let version_list: Vec<String> = versions.iter()
                    .map(|v| v.to_string())
                    .collect();

                Ok(CatalogResponse::Data(serde_json::json!({
                    "graph_type": name,
                    "versions": version_list,
                    "current_version": self.graph_types.get(name).map(|gt| gt.version.to_string()),
                })))
            }
            None => Err(CatalogError::NotFound(
                format!("Graph type '{}' not found", name)
            ))
        }
    }

    /// Check if a graph type exists
    fn exists(&self, name: &str) -> CatalogResult<CatalogResponse> {
        let exists = self.graph_types.contains_key(name);
        Ok(CatalogResponse::Data(serde_json::json!({
            "exists": exists,
            "graph_type": name,
        })))
    }
}

impl CatalogProvider for GraphTypeCatalog {
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()> {
        self.storage = Some(storage.clone());

        // Try to load existing catalog from storage
        if let Ok(data) = storage.load_catalog("graph_type") {
            if !data.is_empty() {
                self.load(&data)?;
            }
        }

        Ok(())
    }

    fn execute(&mut self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Create { entity_type, data } => {
                match entity_type {
                    EntityType::Other(ref type_name) if type_name == "GraphType" => {
                        self.create_graph_type(data)
                    }
                    _ => Err(CatalogError::InvalidOperation(
                        format!("GraphTypeCatalog does not support creating {:?}", entity_type)
                    ))
                }
            }

            CatalogOperation::Delete { entity_type, identifier } => {
                match entity_type {
                    EntityType::Other(ref type_name) if type_name == "GraphType" => {
                        self.drop_graph_type(&identifier)
                    }
                    _ => Err(CatalogError::InvalidOperation(
                        format!("GraphTypeCatalog does not support deleting {:?}", entity_type)
                    ))
                }
            }

            CatalogOperation::Query { query_type, params } => {
                self.execute_read_only(CatalogOperation::Query { query_type, params })
            }

            _ => Err(CatalogError::NotSupported(
                format!("Operation not supported by GraphTypeCatalog")
            ))
        }
    }

    fn execute_read_only(&self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        match op {
            CatalogOperation::Query { query_type, params } => {
                match query_type {
                    QueryType::List => self.list_graph_types(),

                    QueryType::Get => {
                        let name = params.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CatalogError::InvalidData(
                                "Missing 'name' parameter for Get query".to_string()
                            ))?;
                        self.get_graph_type(name)
                    }

                    QueryType::Describe => {
                        let name = params.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CatalogError::InvalidData(
                                "Missing 'name' parameter for Describe query".to_string()
                            ))?;
                        self.describe_graph_type(name)
                    }

                    QueryType::Custom(ref custom) => {
                        match custom.as_str() {
                            "versions" => {
                                let name = params.get("name")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| CatalogError::InvalidData(
                                        "Missing 'name' parameter for versions query".to_string()
                                    ))?;
                                self.get_versions(name)
                            }
                            "exists" => {
                                let name = params.get("name")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| CatalogError::InvalidData(
                                        "Missing 'name' parameter for exists query".to_string()
                                    ))?;
                                self.exists(name)
                            }
                            _ => Err(CatalogError::NotSupported(
                                format!("Custom query '{}' not supported", custom)
                            ))
                        }
                    }

                    _ => Err(CatalogError::NotSupported(
                        format!("Query type {:?} not supported", query_type)
                    ))
                }
            }
            _ => Err(CatalogError::InvalidOperation(
                "Only query operations are supported in read-only mode".to_string()
            ))
        }
    }

    fn save(&self) -> CatalogResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| CatalogError::SerializationError(
                format!("Failed to serialize GraphTypeCatalog: {}", e)
            ))
    }

    fn load(&mut self, data: &[u8]) -> CatalogResult<()> {
        let loaded: GraphTypeCatalog = serde_json::from_slice(data)
            .map_err(|e| CatalogError::SerializationError(
                format!("Failed to deserialize GraphTypeCatalog: {}", e)
            ))?;

        self.graph_types = loaded.graph_types;
        self.version_history = loaded.version_history;

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
                "List GraphTypes".to_string(),
                "Get GraphType".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_graph_type() {
        let mut catalog = GraphTypeCatalog::new();

        let graph_type_def = serde_json::json!({
            "name": "TestGraphType",
            "version": {
                "major": 1,
                "minor": 0,
                "patch": 0
            },
            "node_types": [],
            "edge_types": [],
            "created_at": Utc::now(),
            "updated_at": Utc::now(),
            "created_by": "test_user"
        });

        let result = catalog.create_graph_type(graph_type_def);
        assert!(result.is_ok());

        // Verify the graph type exists
        let exists_result = catalog.exists("TestGraphType");
        assert!(exists_result.is_ok());
        if let Ok(CatalogResponse::Data(data)) = exists_result {
            assert_eq!(data["exists"], true);
        }
    }

    #[test]
    fn test_list_graph_types() {
        let mut catalog = GraphTypeCatalog::new();

        // Create two graph types
        for i in 1..=2 {
            let graph_type_def = serde_json::json!({
                "name": format!("GraphType{}", i),
                "version": {
                    "major": 1,
                    "minor": 0,
                    "patch": 0
                },
                "node_types": [],
                "edge_types": [],
                "created_at": Utc::now(),
                "updated_at": Utc::now(),
                "created_by": "test_user"
            });
            catalog.create_graph_type(graph_type_def).unwrap();
        }

        let result = catalog.list_graph_types();
        assert!(result.is_ok());

        if let Ok(CatalogResponse::List { items, .. }) = result {
            assert_eq!(items.len(), 2);
        }
    }
}
