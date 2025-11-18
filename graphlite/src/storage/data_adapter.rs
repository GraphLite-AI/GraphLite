// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Data adapter for graph storage - handles serialization and data organization
//!
//! This module bridges application data structures (Graph, Catalog, Auth) and low-level storage drivers.
//! It handles serialization/deserialization and organizes data into logical collections,
//! working with any StorageDriver implementation (Sled, RocksDB, Redis/Valkey).

use crate::storage::{GraphCache, types::{Node, Edge}, value::Value};
use crate::storage::{StorageDriver, StorageTree};
use crate::catalog::providers::graph_metadata::{Graph, GraphType};
use crate::catalog::providers::schema::Schema;
use crate::catalog::manager::CatalogManager;
use std::collections::HashMap;
use bincode;
use serde::{Serialize, Deserialize};

// Safe to use block_on here as they're not called from within async contexts
thread_local! {
    static DATA_ADAPTER_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime for data adapter operations");
}

/// Serializable node structure for storage
#[derive(Serialize, Deserialize, Debug)]
struct SerializableNode {
    id: String,
    labels: Vec<String>,
    properties: HashMap<String, Value>,
}

/// Serializable edge structure for storage
#[derive(Serialize, Deserialize, Debug)]
struct SerializableEdge {
    id: String,
    label: String,
    from_node: String,
    to_node: String,
    properties: HashMap<String, Value>,
}

/// Serializable catalog structure for storage
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
struct SerializableCatalogData {
    schemas: HashMap<String, Schema>,
    graphs: HashMap<String, Graph>,
    graph_types: HashMap<String, GraphType>,
}

/// Data adapter that handles serialization and organization of graph data
/// Works with any StorageDriver to persist application data structures
pub struct DataAdapter {}

impl DataAdapter {
    /// Create a new empty DataAdapter instance
    pub fn new() -> Self {
        Self {}
    }
    
    
    /// Normalize graph path to be safe for use as storage key
    fn normalize_graph_path(graph_path: &str) -> String {
        graph_path
            .replace("/", "_")
            .replace("\\", "_")
            .replace(":", "_")
            .replace(".", "_")
            .trim_start_matches('_')
            .to_string()
    }
    
    /// Get the storage type being used
 
    
    
    /// Load a GraphCache for a specific graph path using provided driver connection
    pub fn load_graph_by_path(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, graph_path: &str) -> Result<GraphCache, Box<dyn std::error::Error>> {
        let graph_prefix = Self::normalize_graph_path(graph_path);
        
        // Open graph-specific trees using provided driver
        let nodes_tree = match driver.open_tree(&format!("nodes_{}", graph_prefix)) {
            Ok(tree) => tree,
            Err(e) => {
                // If tree doesn't exist, return empty graph
                if e.to_string().contains("does not exist") || e.to_string().contains("Column family") {
                    return Ok(GraphCache::new());
                }
                return Err(e.into());
            }
        };
        
        let edges_tree = match driver.open_tree(&format!("edges_{}", graph_prefix)) {
            Ok(tree) => tree,
            Err(e) => {
                // If tree doesn't exist, return empty graph  
                if e.to_string().contains("does not exist") || e.to_string().contains("Column family") {
                    return Ok(GraphCache::new());
                }
                return Err(e.into());
            }
        };
        
        let mut graph = GraphCache::new();
        
        // Load all nodes from graph-specific tree
        for result in nodes_tree.iter()? {
            let (_, data) = result?;
            let serializable_node: SerializableNode = bincode::deserialize(&data)?;
            let node = Node {
                id: serializable_node.id,
                labels: serializable_node.labels,
                properties: serializable_node.properties,
            };
            graph.add_node(node)?;
        }
        
        // Load all edges from graph-specific tree
        for result in edges_tree.iter()? {
            let (_, data) = result?;
            let serializable_edge: SerializableEdge = bincode::deserialize(&data)?;
            let edge = Edge {
                id: serializable_edge.id,
                label: serializable_edge.label,
                from_node: serializable_edge.from_node,
                to_node: serializable_edge.to_node,
                properties: serializable_edge.properties,
            };
            graph.add_edge(edge)?;
        }
        
        Ok(graph)
    }

    /// Save a GraphCache for a specific graph path using provided driver connection
    pub fn save_graph_by_path(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, graph: &GraphCache, graph_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let graph_prefix = Self::normalize_graph_path(graph_path);
        
        // Open graph-specific trees using provided driver
        let nodes_tree = driver.open_tree(&format!("nodes_{}", graph_prefix))?;
        let edges_tree = driver.open_tree(&format!("edges_{}", graph_prefix))?;
        let metadata_tree = driver.open_tree(&format!("metadata_{}", graph_prefix))?;
        
        // Clear existing data
        nodes_tree.clear()?;
        edges_tree.clear()?;
        
        // Store each node individually
        for node in graph.get_all_nodes() {
            let serializable_node = SerializableNode {
                id: node.id.clone(),
                labels: node.labels.clone(),
                properties: node.properties.clone(),
            };
            let data = bincode::serialize(&serializable_node)?;
            nodes_tree.insert(node.id.as_bytes(), &data)?;
        }
        
        // Store each edge individually
        for edge_id in graph.edge_ids() {
            if let Some(edge) = graph.get_edge(edge_id) {
                let serializable_edge = SerializableEdge {
                    id: edge.id.clone(),
                    label: edge.label.clone(),
                    from_node: edge.from_node.clone(),
                    to_node: edge.to_node.clone(),
                    properties: edge.properties.clone(),
                };
                let data = bincode::serialize(&serializable_edge)?;
                edges_tree.insert(edge.id.as_bytes(), &data)?;
            }
        }
        
        // Store metadata
        let stats = graph.stats();
        let metadata = serde_json::json!({
            "node_count": stats.node_count,
            "edge_count": stats.edge_count,
            "node_label_count": stats.node_label_count,
            "edge_label_count": stats.edge_label_count,
            "saved_at": chrono::Utc::now().to_rfc3339()
        });
        let metadata_data = bincode::serialize(&metadata)?;
        metadata_tree.insert(b"stats", &metadata_data)?;
        
        // Flush to disk
        driver.flush()?;
        
        Ok(())
    }
    
    /// Check if the database contains saved graph data
    pub fn has_data(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<bool, Box<dyn std::error::Error>> {
        let nodes_tree = driver.open_tree("nodes")?;
        let edges_tree = driver.open_tree("edges")?;
        Ok(!nodes_tree.is_empty()? || !edges_tree.is_empty()?)
    }
    
    /// Save catalog metadata to persistent storage
    pub fn save_catalog(&self, catalog_manager: &CatalogManager) -> Result<(), Box<dyn std::error::Error>> {
        // For now, we'll use the catalog manager's persist_all method
        // The catalog system now handles its own persistence

        let persist_result = if tokio::runtime::Handle::try_current().is_ok() {
            // We're in async context - use block_in_place for critical operations
            tokio::task::block_in_place(|| {
                DATA_ADAPTER_RUNTIME.with(|rt| {
                    rt.block_on(catalog_manager.persist_all())
                })
            })
        } else {
            // We're not in async context - safe to use block_on
            DATA_ADAPTER_RUNTIME.with(|rt| {
                rt.block_on(catalog_manager.persist_all())
            })
        };
        persist_result.map_err(|e| format!("Failed to persist catalogs: {}", e).into())
    }

    /// Load catalog metadata from persistent storage
    pub fn load_catalog(&self, catalog_manager: &mut CatalogManager) -> Result<(), Box<dyn std::error::Error>> {
        // The catalog system now handles its own loading

        let load_result = if tokio::runtime::Handle::try_current().is_ok() {
            // We're in async context - use block_in_place for critical operations
            tokio::task::block_in_place(|| {
                DATA_ADAPTER_RUNTIME.with(|rt| {
                    rt.block_on(catalog_manager.load_all())
                })
            })
        } else {
            // We're not in async context - safe to use block_on
            DATA_ADAPTER_RUNTIME.with(|rt| {
                rt.block_on(catalog_manager.load_all())
            })
        };
        load_result.map_err(|e| format!("Failed to load catalogs: {}", e).into())
    }

    /// Check if the database contains saved catalog data
    pub fn has_catalog_data(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<bool, Box<dyn std::error::Error>> {
        let catalog_tree = driver.open_tree("catalog")?;
        catalog_tree.contains_key(b"catalog_data")
            .map_err(|e| e.into())
    }

    /// Clear all data from the database
    pub fn clear(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<(), Box<dyn std::error::Error>> {
        let nodes_tree = driver.open_tree("nodes")?;
        let edges_tree = driver.open_tree("edges")?;
        let metadata_tree = driver.open_tree("metadata")?;
        let catalog_tree = driver.open_tree("catalog")?;
        
        nodes_tree.clear()?;
        edges_tree.clear()?;
        metadata_tree.clear()?;
        catalog_tree.clear()?;
        driver.flush()?;
        Ok(())
    }
    
    /// Delete a specific graph's data from storage
    pub fn delete_graph(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, graph_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        use log::{debug, info};
        let graph_prefix = Self::normalize_graph_path(graph_name);
        
        // Delete graph-specific trees
        let nodes_tree_name = format!("nodes_{}", graph_prefix);
        let edges_tree_name = format!("edges_{}", graph_prefix);
        let metadata_tree_name = format!("metadata_{}", graph_prefix);
        
        // Open and clear graph-specific trees if they exist
        if let Ok(nodes_tree) = driver.open_tree(&nodes_tree_name) {
            nodes_tree.clear()?;
            debug!("Cleared nodes tree for graph '{}'", graph_name);
        }
        
        if let Ok(edges_tree) = driver.open_tree(&edges_tree_name) {
            edges_tree.clear()?;
            debug!("Cleared edges tree for graph '{}'", graph_name);
        }
        
        if let Ok(metadata_tree) = driver.open_tree(&metadata_tree_name) {
            metadata_tree.clear()?;
            debug!("Cleared metadata tree for graph '{}'", graph_name);
        }
        
        // Flush changes to persistent storage
        driver.flush()?;
        
        info!("Successfully deleted all data for graph '{}'", graph_name);
        Ok(())
    }

    /// Save catalog provider data to persistent storage
    pub fn save_catalog_provider(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, name: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let catalog_tree = driver.open_tree("catalog")?;
        let key = format!("catalog_provider_{}", name);
        catalog_tree.insert(key.as_bytes(), data)?;
        driver.flush()?;
        Ok(())
    }

    /// Load catalog provider data from persistent storage
    pub fn load_catalog_provider(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let catalog_tree = driver.open_tree("catalog")?;
        let key = format!("catalog_provider_{}", name);
        match catalog_tree.get(key.as_bytes())? {
            Some(data) => Ok(data.to_vec()),
            None => Err(format!("Catalog provider '{}' not found", name).into()),
        }
    }

    /// Check if catalog provider data exists
    pub fn has_catalog_provider(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>, name: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let catalog_tree = driver.open_tree("catalog")?;
        let key = format!("catalog_provider_{}", name);
        catalog_tree.contains_key(key.as_bytes())
            .map_err(|e| e.into())
    }

    /// List all catalog provider names
    pub fn list_catalog_providers(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let catalog_tree = driver.open_tree("catalog")?;
        let mut providers = Vec::new();
        let prefix = "catalog_provider_";
        
        for result in catalog_tree.iter()? {
            let (key_bytes, _) = result?;
            let key = String::from_utf8(key_bytes)?;
            if let Some(provider_name) = key.strip_prefix(prefix) {
                providers.push(provider_name.to_string());
            }
        }
        
        providers.sort();
        Ok(providers)
    }
    
    /// Get metadata about stored graph
    pub fn get_metadata(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        let metadata_tree = driver.open_tree("metadata")?;
        if let Some(metadata_data) = metadata_tree.get(b"stats")? {
            let metadata: serde_json::Value = bincode::deserialize(&metadata_data)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }
    
    /// Explicitly shutdown the storage adapter and release file locks
    /// This should be called before dropping to ensure clean resource cleanup
    pub fn shutdown(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<(), Box<dyn std::error::Error>> {
        // Open and flush all trees first
        let nodes_tree = driver.open_tree("nodes")?;
        let edges_tree = driver.open_tree("edges")?;
        let metadata_tree = driver.open_tree("metadata")?;
        let catalog_tree = driver.open_tree("catalog")?;
        
        nodes_tree.flush()?;
        edges_tree.flush()?;
        metadata_tree.flush()?;
        catalog_tree.flush()?;
        
        // Flush the main persistence driver
        driver.flush()?;
        
        Ok(())
    }
    
    /// List all available graphs in storage
    pub fn list_graphs(&self, driver: &dyn StorageDriver<Tree = Box<dyn StorageTree>>) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut graph_names = std::collections::HashSet::new();
        
        // Scan all trees and extract graph names from tree names
        // Tree names follow pattern: {table}_{graph_path}
        let tree_names = driver.list_trees()?;
        
        for tree_name in tree_names {
            // Extract graph path from tree names like "nodes_graph1", "edges_graph2", etc.
            if let Some(graph_path) = self.extract_graph_path_from_tree_name(&tree_name) {
                graph_names.insert(graph_path);
            }
        }
        
        // Convert to sorted vector
        let mut result: Vec<String> = graph_names.into_iter().collect();
        result.sort();
        Ok(result)
    }
    
    /// Extract graph path from tree name
    fn extract_graph_path_from_tree_name(&self, tree_name: &str) -> Option<String> {
        // Skip shared trees like "catalog" and "auth"
        if tree_name == "catalog" || tree_name == "auth" {
            return None;
        }
        
        // Tree names follow pattern: {table}_{graph_path}
        let prefixes = ["nodes_", "edges_", "metadata_"];
        for prefix in &prefixes {
            if let Some(graph_path) = tree_name.strip_prefix(prefix) {
                return Some(graph_path.to_string());
            }
        }
        
        None
    }

}

impl Drop for DataAdapter {
    fn drop(&mut self) {
        // DataAdapter no longer owns storage resources
        // The StorageManager handles all persistence and cleanup
        // Nothing to do here
    }
}

impl std::fmt::Debug for DataAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataAdapter")
            .finish_non_exhaustive()
    }
}
