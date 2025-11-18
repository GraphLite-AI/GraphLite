// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Multi-Graph Storage Manager
//!
//! This module implements the core multi-graph functionality for ISO GQL compliance.
//! It provides:
//! - Multi-graph storage and management
//! - Graph union operations (UNION and UNION ALL)
//! - Session graph context management
//! - CURRENT_PROPERTY_GRAPH resolution
//!
//! Features supported:
//! - FROM (graph1 UNION ALL graph2) - Graph union operations
//! - FROM CURRENT_PROPERTY_GRAPH - Session-context graph references
//! - SET SESSION GRAPH = graph_name - Session graph switching

use crate::storage::{GraphCache, StorageError};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use log::{debug, warn};

/// Multi-graph storage manager that handles multiple named graphs
#[derive(Debug, Clone)]
pub struct MultiGraphManager {
    /// Collection of named graphs
    graphs: Arc<RwLock<HashMap<String, GraphCache>>>,
}

impl MultiGraphManager {
    /// Create a new multi-graph manager
    pub fn new() -> Self {
        Self {
            graphs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new graph with the given name
    pub fn add_graph(&self, name: String, graph: GraphCache) -> Result<(), StorageError> {
        debug!("Adding graph: '{}' (key length: {})", name, name.len());
        
        let mut graphs = self.graphs.write().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if graphs.contains_key(&name) {
            debug!("Graph {} already exists in memory, replacing", name);
            
        }
        
        
        graphs.insert(name.clone(), graph);
        debug!("Successfully added graph '{}' to memory", name);
        Ok(())
    }

    /// Get a graph by name (read-only access)
    pub fn get_graph(&self, name: &str) -> Result<Option<GraphCache>, StorageError> {
        debug!("Getting graph: '{}' (key length: {})", name, name.len());
        
        let graphs = self.graphs.read().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        let result = graphs.get(name).cloned();
        Ok(result)
    }

    /// Get a mutable reference to a graph
    pub fn get_graph_mut(&self, name: &str) -> Result<Option<GraphCache>, StorageError> {
        debug!("Getting mutable graph: {}", name);
        
        let graphs = self.graphs.read().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(graphs.get(name).cloned())
    }

    /// Check if a graph exists
    pub fn has_graph(&self, name: &str) -> bool {
        match self.graphs.read() {
            Ok(graphs) => graphs.contains_key(name),
            Err(e) => {
                warn!("Failed to check graph existence: {}", e);
                false
            }
        }
    }

    /// Get all graph names
    pub fn get_graph_names(&self) -> Result<Vec<String>, StorageError> {
        let graphs = self.graphs.read().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire read lock: {}", e))
        })?;
        
        Ok(graphs.keys().cloned().collect())
    }

    /// Remove a graph by name
    pub fn remove_graph(&self, name: &str) -> Result<(), StorageError> {
        debug!("Removing graph: {}", name);
        let mut graphs = self.graphs.write().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        if graphs.remove(name).is_some() {
            debug!("Successfully removed graph: {}", name);
        } else {
            warn!("Attempted to remove non-existent graph: {}", name);
        }
        
        Ok(())
    }

    /// Perform a graph union operation
    pub fn union_graphs(&self, graphs_to_union: Vec<GraphCache>) -> Result<GraphCache, StorageError> {
        debug!("Performing graph union on {} graphs", graphs_to_union.len());
        
        if graphs_to_union.is_empty() {
            return Err(StorageError::InvalidOperation("Cannot union empty graph list".to_string()));
        }

        // Start with the first graph
        let mut result_graph = graphs_to_union[0].clone();
        
        // Union with remaining graphs
        for graph in graphs_to_union.iter().skip(1) {
            // Manually union graphs by adding all nodes and edges
            for node in graph.get_all_nodes() {
                let has_node = result_graph.has_node(&node.id)
                    .map_err(|e| StorageError::Graph(e))?;
                if !has_node {
                    result_graph.add_node(node.clone())
                        .map_err(|e| StorageError::Graph(e))?;
                }
            }
            for edge in graph.get_all_edges() {
                let has_edge = result_graph.has_edge(&edge.id)
                    .map_err(|e| StorageError::Graph(e))?;
                if !has_edge {
                    result_graph.add_edge(edge.clone())
                        .map_err(|e| StorageError::Graph(e))?;
                }
            }
        }
        
        debug!("Successfully created union graph");
        Ok(result_graph)
    }

    /// Clear all graphs
    pub fn clear(&self) -> Result<(), StorageError> {
        debug!("Clearing all graphs");
        
        let mut graphs = self.graphs.write().map_err(|e| {
            StorageError::LockError(format!("Failed to acquire write lock: {}", e))
        })?;
        
        graphs.clear();
        
        debug!("Successfully cleared all graphs");
        Ok(())
    }

    /// Get the number of graphs
    pub fn graph_count(&self) -> usize {
        match self.graphs.read() {
            Ok(graphs) => graphs.len(),
            Err(_) => 0,
        }
    }
}

impl Default for MultiGraphManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_graph_basic_operations() {
        let manager = MultiGraphManager::new();
        
        // Create and add a graph
        let graph = GraphCache::new();
        manager.add_graph("test_graph".to_string(), graph).unwrap();
        
        // Check if graph exists
        assert!(manager.has_graph("test_graph"));
        
        // Get graph
        let retrieved = manager.get_graph("test_graph").unwrap();
        assert!(retrieved.is_some());
        
        // Get graph names
        let names = manager.get_graph_names().unwrap();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "test_graph");
        
        // Remove graph
        manager.remove_graph("test_graph").unwrap();
        assert!(!manager.has_graph("test_graph"));
    }

}
