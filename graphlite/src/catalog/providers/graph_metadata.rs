// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Graph metadata catalog provider implementation
//!
//! This module provides the graph catalog implementation that follows the
//! pluggable catalog architecture. It manages graph instances, graph types,
//! vertex types, edge types, and property type definitions.

use crate::catalog::error::{CatalogError, CatalogResult};
use crate::catalog::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use crate::catalog::traits::{CatalogProvider, CatalogSchema};
use crate::storage::StorageManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Graph identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphId {
    pub id: Uuid,
    pub name: String,
    pub schema_name: String,
}

impl GraphId {
    pub fn new(name: String, schema_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            schema_name,
        }
    }
}

/// Graph type identifier  
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GraphTypeId {
    pub id: Uuid,
    pub name: String,
    pub schema_name: String,
}

impl GraphTypeId {
    pub fn new(name: String, schema_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            schema_name,
        }
    }
}

/// Property graph instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub id: GraphId,
    pub graph_type_id: Option<GraphTypeId>,

    /// Graph-level properties
    pub properties: HashMap<String, GraphProperty>,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modification timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,

    /// Graph description/documentation
    pub description: Option<String>,

    /// Whether this graph is materialized or virtual
    pub is_materialized: bool,
}

impl Graph {
    pub fn new(id: GraphId, graph_type_id: Option<GraphTypeId>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            graph_type_id,
            properties: HashMap::new(),
            created_at: now,
            modified_at: now,
            description: None,
            is_materialized: true,
        }
    }

    /// Create from parameters
    pub fn from_params(name: String, params: &Value) -> Self {
        let schema_name = params
            .get("schema_name")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let id = GraphId::new(name, schema_name.clone());

        let graph_type_id = params
            .get("graph_type")
            .and_then(|v| v.as_str())
            .map(|type_name| GraphTypeId::new(type_name.to_string(), schema_name.clone()));

        let mut graph = Self::new(id, graph_type_id);

        if let Some(desc) = params.get("description").and_then(|v| v.as_str()) {
            graph.description = Some(desc.to_string());
        }

        if let Some(mat) = params.get("is_materialized").and_then(|v| v.as_bool()) {
            graph.is_materialized = mat;
        }

        if let Some(props) = params.get("properties").and_then(|v| v.as_object()) {
            for (key, value) in props {
                if let Ok(prop) = serde_json::from_value::<GraphProperty>(value.clone()) {
                    graph.properties.insert(key.clone(), prop);
                }
            }
        }

        graph
    }

    /// Set a graph property
    pub fn set_property(&mut self, key: String, value: GraphProperty) {
        self.properties.insert(key, value);
        self.modified_at = chrono::Utc::now();
    }

    /// Get a graph property
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph property accessor for metadata queries
    pub fn get_property(&self, key: &str) -> Option<&GraphProperty> {
        self.properties.get(key)
    }
}

/// Graph-level properties  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphProperty {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<GraphProperty>),
    Map(HashMap<String, GraphProperty>),
}

/// Graph type definition (schema for graphs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphType {
    pub id: GraphTypeId,

    /// Vertex types in this graph type
    pub vertex_types: HashMap<String, VertexType>,

    /// Edge types in this graph type
    pub edge_types: HashMap<String, EdgeType>,

    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last modification timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,

    /// Graph type description
    pub description: Option<String>,
}

impl GraphType {
    pub fn new(id: GraphTypeId) -> Self {
        let now = chrono::Utc::now();
        Self {
            id,
            vertex_types: HashMap::new(),
            edge_types: HashMap::new(),
            created_at: now,
            modified_at: now,
            description: None,
        }
    }

    /// Create from parameters
    pub fn from_params(name: String, params: &Value) -> Self {
        let schema_name = params
            .get("schema_name")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let id = GraphTypeId::new(name, schema_name);
        let mut graph_type = Self::new(id);

        if let Some(desc) = params.get("description").and_then(|v| v.as_str()) {
            graph_type.description = Some(desc.to_string());
        }

        // Parse vertex types
        if let Some(vertex_types) = params.get("vertex_types").and_then(|v| v.as_object()) {
            for (name, spec) in vertex_types {
                if let Ok(vtype) = serde_json::from_value::<VertexType>(spec.clone()) {
                    graph_type.vertex_types.insert(name.clone(), vtype);
                }
            }
        }

        // Parse edge types
        if let Some(edge_types) = params.get("edge_types").and_then(|v| v.as_object()) {
            for (name, spec) in edge_types {
                if let Ok(etype) = serde_json::from_value::<EdgeType>(spec.clone()) {
                    graph_type.edge_types.insert(name.clone(), etype);
                }
            }
        }

        graph_type
    }

    /// Add a vertex type
    pub fn add_vertex_type(&mut self, name: String, vertex_type: VertexType) {
        self.vertex_types.insert(name, vertex_type);
        self.modified_at = chrono::Utc::now();
    }

    /// Add an edge type
    pub fn add_edge_type(&mut self, name: String, edge_type: EdgeType) {
        self.edge_types.insert(name, edge_type);
        self.modified_at = chrono::Utc::now();
    }
}

/// Vertex type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VertexType {
    pub name: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, PropertyType>,
    pub description: Option<String>,
}

impl VertexType {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Vertex type builder for graph type DDL
    pub fn new(name: String) -> Self {
        Self {
            name,
            labels: Vec::new(),
            properties: HashMap::new(),
            description: None,
        }
    }

    /// Add a label to this vertex type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn add_label(&mut self, label: String) {
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }

    /// Add a property to this vertex type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn add_property(&mut self, name: String, property_type: PropertyType) {
        self.properties.insert(name, property_type);
    }
}

/// Edge type definition  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeType {
    pub name: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, PropertyType>,
    pub source_vertex_type: Option<String>,
    pub destination_vertex_type: Option<String>,
    pub description: Option<String>,
}

impl EdgeType {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Edge type builder for graph type DDL
    pub fn new(name: String) -> Self {
        Self {
            name,
            labels: Vec::new(),
            properties: HashMap::new(),
            source_vertex_type: None,
            destination_vertex_type: None,
            description: None,
        }
    }

    /// Add a label to this edge type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn add_label(&mut self, label: String) {
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }

    /// Add a property to this edge type
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn add_property(&mut self, name: String, property_type: PropertyType) {
        self.properties.insert(name, property_type);
    }

    /// Set source and destination vertex types
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn set_endpoints(&mut self, source: Option<String>, destination: Option<String>) {
        self.source_vertex_type = source;
        self.destination_vertex_type = destination;
    }
}

/// Property type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyType {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default_value: Option<PropertyValue>,
    pub description: Option<String>,
}

impl PropertyType {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Property type builder for schema definitions
    pub fn new(name: String, data_type: DataType) -> Self {
        Self {
            name,
            data_type,
            nullable: true,
            default_value: None,
            description: None,
        }
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn with_nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type builder methods (see ROADMAP.md §4)
    pub fn with_default(mut self, default: PropertyValue) -> Self {
        self.default_value = Some(default);
        self
    }
}

/// Data types supported in graph properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    Boolean,
    String {
        max_length: Option<usize>,
    },
    Bytes {
        max_length: Option<usize>,
    },
    Integer,
    BigInteger,
    SmallInteger,
    Float,
    Double,
    Decimal {
        precision: Option<u8>,
        scale: Option<u8>,
    },
    Date,
    Time {
        with_timezone: bool,
    },
    DateTime {
        with_timezone: bool,
    },
    Duration,
    List {
        element_type: Box<DataType>,
        max_length: Option<usize>,
    },
    Record,
    Path,
    Graph,
    Ref {
        target_type: Box<DataType>,
    },
}

/// Property values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyValue {
    Null,
    Boolean(bool),
    String(String),
    Bytes(Vec<u8>),
    Integer(i64),
    Float(f64),
    List(Vec<PropertyValue>),
    Map(HashMap<String, PropertyValue>),
}

/// Graph metadata catalog state
#[derive(Clone, Serialize, Deserialize)]
struct GraphMetadataState {
    graphs: HashMap<String, Graph>,
    graph_types: HashMap<String, GraphType>,
}

/// Graph metadata catalog provider
#[derive(Clone)]
pub struct GraphMetadataCatalog {
    /// Map of graph name to graph instance
    graphs: HashMap<String, Graph>,

    /// Map of graph type name to graph type definition
    graph_types: HashMap<String, GraphType>,

    /// Storage manager reference
    storage: Option<Arc<StorageManager>>,
}

impl GraphMetadataCatalog {
    /// Create a new graph metadata catalog provider
    pub fn new() -> Box<dyn CatalogProvider> {
        Box::new(Self {
            graphs: HashMap::new(),
            graph_types: HashMap::new(),
            storage: None,
        })
    }

    /// Create a schema-qualified key for graph storage
    fn qualified_graph_key(schema_name: &str, graph_name: &str) -> String {
        format!("{}/{}", schema_name, graph_name)
    }

    /// Create a schema-qualified key for graph type storage  
    fn qualified_graph_type_key(schema_name: &str, graph_type_name: &str) -> String {
        format!("{}/{}", schema_name, graph_type_name)
    }

    /// Add a graph to the catalog
    fn add_graph(&mut self, graph: Graph) -> CatalogResult<()> {
        let qualified_key = Self::qualified_graph_key(&graph.id.schema_name, &graph.id.name);
        if self.graphs.contains_key(&qualified_key) {
            return Err(CatalogError::DuplicateEntry(format!(
                "Graph '{}' already exists",
                qualified_key
            )));
        }

        self.graphs.insert(qualified_key, graph);
        Ok(())
    }

    /// Add a graph type to the catalog
    fn add_graph_type(&mut self, graph_type: GraphType) -> CatalogResult<()> {
        let qualified_key =
            Self::qualified_graph_type_key(&graph_type.id.schema_name, &graph_type.id.name);
        if self.graph_types.contains_key(&qualified_key) {
            return Err(CatalogError::DuplicateEntry(format!(
                "Graph type '{}' already exists",
                qualified_key
            )));
        }

        self.graph_types.insert(qualified_key, graph_type);
        Ok(())
    }

    /// Get a graph by name (backward compatibility - searches all schemas)
    fn get_graph(&self, name: &str) -> Option<&Graph> {
        // First try direct lookup for backward compatibility
        if let Some(graph) = self.graphs.get(name) {
            return Some(graph);
        }

        // Then search all qualified names ending with the graph name
        self.graphs.values().find(|g| g.id.name == name)
    }

    /// Get a graph type by name (backward compatibility - searches all schemas)
    fn get_graph_type(&self, name: &str) -> Option<&GraphType> {
        // First try direct lookup for backward compatibility
        if let Some(graph_type) = self.graph_types.get(name) {
            return Some(graph_type);
        }

        // Then search all qualified names ending with the graph type name
        self.graph_types.values().find(|gt| gt.id.name == name)
    }

    /// Remove a graph (backward compatibility - searches all schemas)
    fn remove_graph(&mut self, name: &str) -> CatalogResult<Graph> {
        // First try direct lookup for backward compatibility
        if let Some(graph) = self.graphs.remove(name) {
            return Ok(graph);
        }

        // Then search all qualified names ending with the graph name and remove
        let qualified_key = self
            .graphs
            .keys()
            .find(|k| k.ends_with(&format!("/{}", name)))
            .cloned();

        if let Some(key) = qualified_key {
            self.graphs
                .remove(&key)
                .ok_or_else(|| CatalogError::NotFound(format!("Graph '{}' not found", name)))
        } else {
            Err(CatalogError::NotFound(format!(
                "Graph '{}' not found",
                name
            )))
        }
    }

    /// Remove a graph type (backward compatibility - searches all schemas)
    fn remove_graph_type(&mut self, name: &str, cascade: bool) -> CatalogResult<GraphType> {
        // Check if any graphs use this type
        if !cascade {
            let graphs_using_type: Vec<_> = self
                .graphs
                .values()
                .filter(|g| {
                    g.graph_type_id
                        .as_ref()
                        .map(|t| t.name == name)
                        .unwrap_or(false)
                })
                .map(|g| format!("{}/{}", g.id.schema_name, g.id.name))
                .collect();

            if !graphs_using_type.is_empty() {
                return Err(CatalogError::InvalidOperation(format!(
                    "Cannot drop graph type '{}': used by graphs: {:?}",
                    name, graphs_using_type
                )));
            }
        }

        // First try direct lookup for backward compatibility
        if let Some(graph_type) = self.graph_types.remove(name) {
            return Ok(graph_type);
        }

        // Then search all qualified names ending with the graph type name and remove
        let qualified_key = self
            .graph_types
            .keys()
            .find(|k| k.ends_with(&format!("/{}", name)))
            .cloned();

        if let Some(key) = qualified_key {
            self.graph_types
                .remove(&key)
                .ok_or_else(|| CatalogError::NotFound(format!("Graph type '{}' not found", name)))
        } else {
            Err(CatalogError::NotFound(format!(
                "Graph type '{}' not found",
                name
            )))
        }
    }

    /// Update graph properties
    fn update_graph(&mut self, name: &str, updates: &Value) -> CatalogResult<()> {
        let graph = self
            .graphs
            .get_mut(name)
            .ok_or_else(|| CatalogError::NotFound(format!("Graph '{}' not found", name)))?;

        if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
            graph.description = Some(desc.to_string());
        }

        if let Some(mat) = updates.get("is_materialized").and_then(|v| v.as_bool()) {
            graph.is_materialized = mat;
        }

        if let Some(props) = updates.get("properties").and_then(|v| v.as_object()) {
            for (key, value) in props {
                if let Ok(prop) = serde_json::from_value::<GraphProperty>(value.clone()) {
                    graph.set_property(key.clone(), prop);
                }
            }
        }

        graph.modified_at = chrono::Utc::now();
        Ok(())
    }

    /// Add vertex type to a graph type
    fn add_vertex_type_to_graph_type(
        &mut self,
        graph_type_name: &str,
        vertex_type: VertexType,
    ) -> CatalogResult<()> {
        // First try direct lookup for backward compatibility
        if let Some(graph_type) = self.graph_types.get_mut(graph_type_name) {
            graph_type.add_vertex_type(vertex_type.name.clone(), vertex_type);
            return Ok(());
        }

        // Then search all qualified names ending with the graph type name
        let qualified_key = self
            .graph_types
            .keys()
            .find(|key| {
                let parts: Vec<&str> = key.split('/').collect();
                parts.last() == Some(&graph_type_name)
            })
            .cloned();

        if let Some(key) = qualified_key {
            if let Some(graph_type) = self.graph_types.get_mut(&key) {
                graph_type.add_vertex_type(vertex_type.name.clone(), vertex_type);
                return Ok(());
            }
        }

        Err(CatalogError::NotFound(format!(
            "Graph type '{}' not found",
            graph_type_name
        )))
    }

    /// Add edge type to a graph type
    fn add_edge_type_to_graph_type(
        &mut self,
        graph_type_name: &str,
        edge_type: EdgeType,
    ) -> CatalogResult<()> {
        // First try direct lookup for backward compatibility
        if let Some(graph_type) = self.graph_types.get_mut(graph_type_name) {
            graph_type.add_edge_type(edge_type.name.clone(), edge_type);
            return Ok(());
        }

        // Then search all qualified names ending with the graph type name
        let qualified_key = self
            .graph_types
            .keys()
            .find(|key| {
                let parts: Vec<&str> = key.split('/').collect();
                parts.last() == Some(&graph_type_name)
            })
            .cloned();

        if let Some(key) = qualified_key {
            if let Some(graph_type) = self.graph_types.get_mut(&key) {
                graph_type.add_edge_type(edge_type.name.clone(), edge_type);
                return Ok(());
            }
        }

        Err(CatalogError::NotFound(format!(
            "Graph type '{}' not found",
            graph_type_name
        )))
    }

    /// Get graphs by schema
    fn get_graphs_by_schema(&self, schema_name: &str) -> Vec<&Graph> {
        self.graphs
            .values()
            .filter(|g| g.id.schema_name == schema_name)
            .collect()
    }

    /// Get graph types by schema
    fn get_graph_types_by_schema(&self, schema_name: &str) -> Vec<&GraphType> {
        self.graph_types
            .values()
            .filter(|gt| gt.id.schema_name == schema_name)
            .collect()
    }
}

impl CatalogProvider for GraphMetadataCatalog {
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()> {
        self.storage = Some(storage.clone());

        // Try to load persisted state from storage (same pattern as SecurityCatalog)
        match storage.load_catalog_provider("graph_metadata") {
            Ok(Some(data)) => {
                // Load persisted catalog state
                match self.load(&data) {
                    Ok(_) => {
                        log::info!("Graph metadata catalog loaded from storage with {} graphs, {} graph types",
                            self.graphs.len(), self.graph_types.len());
                    }
                    Err(e) => {
                        log::warn!("Failed to deserialize graph metadata catalog from storage: {}. Using defaults.", e);
                    }
                }
            }
            Ok(None) => {
                // This is expected on first run - no catalog data exists yet
                log::debug!(
                    "No persisted graph metadata catalog found. Using default initialization."
                );
            }
            Err(e) => {
                log::warn!(
                    "Error loading graph metadata catalog: {}. Using default initialization.",
                    e
                );
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
            } => match entity_type {
                EntityType::Graph => {
                    let graph = Graph::from_params(name.clone(), &params);
                    self.add_graph(graph)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("Graph '{}' created", name) })),
                    })
                }
                EntityType::GraphType => {
                    let graph_type = GraphType::from_params(name.clone(), &params);
                    self.add_graph_type(graph_type)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("Graph type '{}' created", name) })),
                    })
                }
                EntityType::VertexType => {
                    let graph_type_name = params
                        .get("graph_type")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            CatalogError::InvalidParameters(
                                "Missing 'graph_type' parameter".to_string(),
                            )
                        })?;

                    let vertex_type = serde_json::from_value::<VertexType>(params.clone())
                        .map_err(|e| CatalogError::InvalidParameters(e.to_string()))?;

                    self.add_vertex_type_to_graph_type(graph_type_name, vertex_type)?;
                    Ok(CatalogResponse::Success {
                        data: Some(
                            json!({ "message": format!("Vertex type '{}' added to graph type '{}'", name, graph_type_name) }),
                        ),
                    })
                }
                EntityType::EdgeType => {
                    let graph_type_name = params
                        .get("graph_type")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            CatalogError::InvalidParameters(
                                "Missing 'graph_type' parameter".to_string(),
                            )
                        })?;

                    let edge_type = serde_json::from_value::<EdgeType>(params.clone())
                        .map_err(|e| CatalogError::InvalidParameters(e.to_string()))?;

                    self.add_edge_type_to_graph_type(graph_type_name, edge_type)?;
                    Ok(CatalogResponse::Success {
                        data: Some(
                            json!({ "message": format!("Edge type '{}' added to graph type '{}'", name, graph_type_name) }),
                        ),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::Drop {
                entity_type,
                name,
                cascade,
            } => match entity_type {
                EntityType::Graph => {
                    let removed = self.remove_graph(&name)?;
                    Ok(CatalogResponse::Success {
                        data: Some(serde_json::to_value(removed)?),
                    })
                }
                EntityType::GraphType => {
                    let removed = self.remove_graph_type(&name, cascade)?;
                    Ok(CatalogResponse::Success {
                        data: Some(serde_json::to_value(removed)?),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::Query { query_type, params } => match query_type {
                QueryType::GetGraph => {
                    if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                        if let Some(graph) = self.get_graph(name) {
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(graph)?,
                            })
                        } else {
                            Err(CatalogError::NotFound(format!(
                                "Graph '{}' not found",
                                name
                            )))
                        }
                    } else {
                        Err(CatalogError::InvalidParameters(
                            "Missing 'name' parameter".to_string(),
                        ))
                    }
                }
                QueryType::GetGraphType => {
                    if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                        if let Some(graph_type) = self.get_graph_type(name) {
                            Ok(CatalogResponse::Query {
                                results: serde_json::to_value(graph_type)?,
                            })
                        } else {
                            Err(CatalogError::NotFound(format!(
                                "Graph type '{}' not found",
                                name
                            )))
                        }
                    } else {
                        Err(CatalogError::InvalidParameters(
                            "Missing 'name' parameter".to_string(),
                        ))
                    }
                }
                QueryType::List => {
                    if let Some(schema_name) = params.get("schema_name").and_then(|v| v.as_str()) {
                        let entity_type = params
                            .get("entity_type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("graph");

                        let results = match entity_type {
                            "graph" => {
                                serde_json::to_value(self.get_graphs_by_schema(schema_name))?
                            }
                            "graph_type" => {
                                serde_json::to_value(self.get_graph_types_by_schema(schema_name))?
                            }
                            _ => return Ok(CatalogResponse::NotSupported),
                        };

                        Ok(CatalogResponse::Query { results })
                    } else {
                        Err(CatalogError::InvalidParameters(
                            "Missing 'schema_name' parameter".to_string(),
                        ))
                    }
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::Update {
                entity_type,
                name,
                updates,
            } => match entity_type {
                EntityType::Graph => {
                    self.update_graph(&name, &updates)?;
                    Ok(CatalogResponse::Success {
                        data: Some(json!({ "message": format!("Graph '{}' updated", name) })),
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            CatalogOperation::List {
                entity_type,
                filters,
            } => match entity_type {
                EntityType::Graph => {
                    let graphs: Vec<&Graph> = if let Some(filters) = filters {
                        if let Some(schema_name) =
                            filters.get("schema_name").and_then(|v| v.as_str())
                        {
                            self.get_graphs_by_schema(schema_name)
                        } else {
                            self.graphs.values().collect()
                        }
                    } else {
                        self.graphs.values().collect()
                    };

                    Ok(CatalogResponse::List {
                        items: graphs
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                EntityType::GraphType => {
                    let graph_types: Vec<&GraphType> = if let Some(filters) = filters {
                        if let Some(schema_name) =
                            filters.get("schema_name").and_then(|v| v.as_str())
                        {
                            self.get_graph_types_by_schema(schema_name)
                        } else {
                            self.graph_types.values().collect()
                        }
                    } else {
                        self.graph_types.values().collect()
                    };

                    Ok(CatalogResponse::List {
                        items: graph_types
                            .iter()
                            .map(serde_json::to_value)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                }
                _ => Ok(CatalogResponse::NotSupported),
            },

            _ => Ok(CatalogResponse::NotSupported),
        }
    }

    fn save(&self) -> CatalogResult<Vec<u8>> {
        let state = GraphMetadataState {
            graphs: self.graphs.clone(),
            graph_types: self.graph_types.clone(),
        };

        let data = bincode::serialize(&state)
            .map_err(|e| CatalogError::SerializationError(e.to_string()))?;
        Ok(data)
    }

    fn load(&mut self, data: &[u8]) -> CatalogResult<()> {
        let state: GraphMetadataState = bincode::deserialize(data)
            .map_err(|e| CatalogError::DeserializationError(e.to_string()))?;

        self.graphs = state.graphs;
        self.graph_types = state.graph_types;
        Ok(())
    }

    fn schema(&self) -> CatalogSchema {
        CatalogSchema {
            name: "graph_metadata".to_string(),
            version: "1.0.0".to_string(),
            entities: vec![
                EntityType::Graph.to_string(),
                EntityType::GraphType.to_string(),
                EntityType::VertexType.to_string(),
                EntityType::EdgeType.to_string(),
            ],
            operations: self.supported_operations(),
        }
    }

    fn supported_operations(&self) -> Vec<String> {
        vec![
            "create_graph".to_string(),
            "create_graph_type".to_string(),
            "add_vertex_type".to_string(),
            "add_edge_type".to_string(),
            "drop_graph".to_string(),
            "drop_graph_type".to_string(),
            "get_graph".to_string(),
            "get_graph_type".to_string(),
            "list_graphs".to_string(),
            "list_graph_types".to_string(),
            "update_graph".to_string(),
            "query_by_schema".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_catalog_creation() {
        let catalog = GraphMetadataCatalog::new();
        assert_eq!(catalog.schema().name, "graph_metadata");
    }

    #[test]
    fn test_create_and_get_graph() {
        let mut catalog = GraphMetadataCatalog {
            graphs: HashMap::new(),
            graph_types: HashMap::new(),
            storage: None,
        };

        let params = json!({
            "schema_name": "test_schema",
            "description": "Test graph",
            "is_materialized": true
        });

        let result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::Graph,
            name: "test_graph".to_string(),
            params,
        });

        assert!(result.is_ok());

        let query_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::GetGraph,
            params: json!({ "name": "test_graph" }),
        });

        assert!(query_result.is_ok());
    }

    #[test]
    fn test_create_graph_type_with_vertex_and_edge_types() {
        let mut catalog = GraphMetadataCatalog {
            graphs: HashMap::new(),
            graph_types: HashMap::new(),
            storage: None,
        };

        // Create graph type
        let graph_type_params = json!({
            "schema_name": "test_schema",
            "description": "Social network graph type"
        });

        catalog
            .execute(CatalogOperation::Create {
                entity_type: EntityType::GraphType,
                name: "social_network".to_string(),
                params: graph_type_params,
            })
            .unwrap();

        // Add vertex type
        let vertex_params = json!({
            "graph_type": "social_network",
            "name": "Person",
            "labels": ["User", "Member"],
            "properties": {
                "id": {
                    "name": "id",
                    "data_type": "Integer",
                    "nullable": false
                },
                "name": {
                    "name": "name",
                    "data_type": { "String": { "max_length": 100 } },
                    "nullable": false
                }
            }
        });

        let result = catalog.execute(CatalogOperation::Create {
            entity_type: EntityType::VertexType,
            name: "Person".to_string(),
            params: vertex_params,
        });

        assert!(result.is_ok());

        // Verify graph type contains vertex type
        let query_result = catalog.execute(CatalogOperation::Query {
            query_type: QueryType::GetGraphType,
            params: json!({ "name": "social_network" }),
        });

        assert!(query_result.is_ok());
        if let Ok(CatalogResponse::Query { results }) = query_result {
            let graph_type: GraphType = serde_json::from_value(results).unwrap();
            assert!(graph_type.vertex_types.contains_key("Person"));
        }
    }

    #[test]
    fn test_list_graphs_by_schema() {
        let mut catalog = GraphMetadataCatalog {
            graphs: HashMap::new(),
            graph_types: HashMap::new(),
            storage: None,
        };

        // Add graphs to different schemas
        catalog
            .execute(CatalogOperation::Create {
                entity_type: EntityType::Graph,
                name: "graph1".to_string(),
                params: json!({ "schema_name": "schema1" }),
            })
            .unwrap();

        catalog
            .execute(CatalogOperation::Create {
                entity_type: EntityType::Graph,
                name: "graph2".to_string(),
                params: json!({ "schema_name": "schema1" }),
            })
            .unwrap();

        catalog
            .execute(CatalogOperation::Create {
                entity_type: EntityType::Graph,
                name: "graph3".to_string(),
                params: json!({ "schema_name": "schema2" }),
            })
            .unwrap();

        // List graphs in schema1
        let result = catalog.execute(CatalogOperation::List {
            entity_type: EntityType::Graph,
            filters: Some(json!({ "schema_name": "schema1" })),
        });

        assert!(result.is_ok());
        if let Ok(CatalogResponse::List { items }) = result {
            assert_eq!(items.len(), 2);
        }
    }
}
