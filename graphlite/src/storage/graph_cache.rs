// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! In-memory graph cache implementation
//!
//! Provides fast graph storage using HashMap for nodes/edges and
//! adjacency lists for efficient graph traversal. Includes label
//! indices for quick lookup by node/edge types.

use crate::catalog::providers::schema::SchemaId;
use crate::storage::types::{Edge, GraphError, Node};
use std::collections::HashMap;

/// In-memory graph cache with indices for fast lookups
#[derive(Debug, Clone)]
pub struct GraphCache {
    /// Schema this graph belongs to
    pub schema_id: Option<SchemaId>,

    /// All nodes indexed by ID
    nodes: HashMap<String, Node>,

    /// All edges indexed by ID  
    edges: HashMap<String, Edge>,

    /// Index: label -> list of node IDs with that label
    node_labels: HashMap<String, Vec<String>>,

    /// Index: label -> list of edge IDs with that label
    edge_labels: HashMap<String, Vec<String>>,

    /// Adjacency list: node_id -> list of outgoing edge IDs
    adjacency_out: HashMap<String, Vec<String>>,

    /// Adjacency list: node_id -> list of incoming edge IDs
    adjacency_in: HashMap<String, Vec<String>>,
}

impl GraphCache {
    /// Create a new empty graph
    pub fn new() -> Self {
        Self {
            schema_id: None,
            nodes: HashMap::new(),
            edges: HashMap::new(),
            node_labels: HashMap::new(),
            edge_labels: HashMap::new(),
            adjacency_out: HashMap::new(),
            adjacency_in: HashMap::new(),
        }
    }

    /// Create a new graph with catalog identity

    /// Set the catalog identity of this graph

    /// Get the full path of this graph in /<schema>/<graph> format
    /// Returns None if catalog identity is not set

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) -> Result<(), GraphError> {
        // Check if node already exists
        if self.nodes.contains_key(&node.id) {
            return Err(GraphError::NodeAlreadyExists(node.id));
        }

        // Update label indices
        for label in &node.labels {
            self.node_labels
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(node.id.clone());
        }

        // Initialize adjacency lists for this node
        self.adjacency_out.insert(node.id.clone(), Vec::new());
        self.adjacency_in.insert(node.id.clone(), Vec::new());

        // Store the node
        self.nodes.insert(node.id.clone(), node);

        Ok(())
    }

    /// Add an edge to the graph
    pub fn add_edge(&mut self, edge: Edge) -> Result<(), GraphError> {
        // Check if edge already exists
        if self.edges.contains_key(&edge.id) {
            return Err(GraphError::EdgeAlreadyExists(edge.id));
        }

        // Check for semantic duplicate edge (same source, target, label, and properties)
        let has_duplicate = self.edges.values().any(|existing_edge| {
            existing_edge.from_node == edge.from_node
                && existing_edge.to_node == edge.to_node
                && existing_edge.label == edge.label
                && existing_edge.properties == edge.properties
        });

        if has_duplicate {
            return Err(GraphError::EdgeAlreadyExists(format!(
                "Relationship already exists: ({}) -[{}]-> ({})",
                edge.from_node, edge.label, edge.to_node
            )));
        }

        // Verify that both nodes exist
        if !self.nodes.contains_key(&edge.from_node) {
            return Err(GraphError::InvalidEdge {
                from: edge.from_node.clone(),
                to: edge.to_node.clone(),
            });
        }
        if !self.nodes.contains_key(&edge.to_node) {
            return Err(GraphError::InvalidEdge {
                from: edge.from_node.clone(),
                to: edge.to_node.clone(),
            });
        }

        // Update edge label index
        self.edge_labels
            .entry(edge.label.clone())
            .or_insert_with(Vec::new)
            .push(edge.id.clone());

        // Update adjacency lists
        self.adjacency_out
            .get_mut(&edge.from_node)
            .unwrap()
            .push(edge.id.clone());

        self.adjacency_in
            .get_mut(&edge.to_node)
            .unwrap()
            .push(edge.id.clone());

        // Store the edge
        self.edges.insert(edge.id.clone(), edge);

        Ok(())
    }

    /// Get a node by ID
    pub fn get_node(&self, id: &str) -> Option<&Node> {
        self.nodes.get(id)
    }

    /// Get a mutable reference to a node by ID
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// Get an edge by ID
    pub fn get_edge(&self, id: &str) -> Option<&Edge> {
        self.edges.get(id)
    }

    /// Get a mutable reference to an edge by ID
    pub fn get_edge_mut(&mut self, id: &str) -> Option<&mut Edge> {
        self.edges.get_mut(id)
    }

    /// Get all nodes with a specific label
    pub fn get_nodes_by_label(&self, label: &str) -> Vec<&Node> {
        self.node_labels
            .get(label)
            .map(|ids| ids.iter().filter_map(|id| self.nodes.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all nodes in the graph
    pub fn get_all_nodes(&self) -> Vec<&Node> {
        self.nodes.values().collect()
    }

    /// Get all nodes in the graph (owned)
    pub fn get_all_nodes_owned(&self) -> Result<Vec<Node>, GraphError> {
        Ok(self.nodes.values().cloned().collect())
    }

    /// Get all edges in the graph
    pub fn get_all_edges(&self) -> Vec<&Edge> {
        self.edges.values().collect()
    }

    /// Get all edges in the graph (owned)
    pub fn get_all_edges_owned(&self) -> Result<Vec<Edge>, GraphError> {
        Ok(self.edges.values().cloned().collect())
    }

    /// Check if a node exists
    pub fn has_node(&self, node_id: &str) -> Result<bool, GraphError> {
        Ok(self.nodes.contains_key(node_id))
    }

    /// Check if the graph is empty (no nodes and no edges)
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.edges.is_empty()
    }

    /// Check if an edge exists
    pub fn has_edge(&self, edge_id: &str) -> Result<bool, GraphError> {
        Ok(self.edges.contains_key(edge_id))
    }

    /// Get node count
    pub fn node_count(&self) -> Result<usize, GraphError> {
        Ok(self.nodes.len())
    }

    /// Get edge count
    pub fn edge_count(&self) -> Result<usize, GraphError> {
        Ok(self.edges.len())
    }

    /// Get all edges with a specific label
    pub fn get_edges_by_label(&self, label: &str) -> Vec<&Edge> {
        self.edge_labels
            .get(label)
            .map(|ids| ids.iter().filter_map(|id| self.edges.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all outgoing edges from a node
    pub fn get_outgoing_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.adjacency_out
            .get(node_id)
            .map(|edge_ids| {
                edge_ids
                    .iter()
                    .filter_map(|id| self.edges.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all incoming edges to a node
    pub fn get_incoming_edges(&self, node_id: &str) -> Vec<&Edge> {
        self.adjacency_in
            .get(node_id)
            .map(|edge_ids| {
                edge_ids
                    .iter()
                    .filter_map(|id| self.edges.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all edges connected to a node (both incoming and outgoing)
    pub fn get_connected_edges(&self, node_id: &str) -> Vec<&Edge> {
        let mut edges = self.get_outgoing_edges(node_id);
        edges.extend(self.get_incoming_edges(node_id));
        edges
    }

    /// Get neighbors of a node (nodes connected by outgoing edges)
    pub fn get_neighbors(&self, node_id: &str) -> Vec<&Node> {
        self.get_outgoing_edges(node_id)
            .into_iter()
            .filter_map(|edge| self.nodes.get(&edge.to_node))
            .collect()
    }

    /// Get all neighbors (both incoming and outgoing connections)
    pub fn get_all_neighbors(&self, node_id: &str) -> Vec<&Node> {
        let mut neighbors = Vec::new();

        // Add outgoing neighbors
        for edge in self.get_outgoing_edges(node_id) {
            if let Some(node) = self.nodes.get(&edge.to_node) {
                neighbors.push(node);
            }
        }

        // Add incoming neighbors
        for edge in self.get_incoming_edges(node_id) {
            if let Some(node) = self.nodes.get(&edge.from_node) {
                neighbors.push(node);
            }
        }

        neighbors
    }

    /// Remove a node and all its connected edges
    pub fn remove_node(&mut self, node_id: &str) -> Result<Node, GraphError> {
        let node = self
            .nodes
            .remove(node_id)
            .ok_or_else(|| GraphError::NodeNotFound(node_id.to_string()))?;

        // Remove from label indices
        for label in &node.labels {
            if let Some(nodes) = self.node_labels.get_mut(label) {
                nodes.retain(|id| id != node_id);
                if nodes.is_empty() {
                    self.node_labels.remove(label);
                }
            }
        }

        // Collect all connected edges to remove
        let mut edges_to_remove = Vec::new();
        if let Some(outgoing) = self.adjacency_out.get(node_id) {
            edges_to_remove.extend(outgoing.clone());
        }
        if let Some(incoming) = self.adjacency_in.get(node_id) {
            edges_to_remove.extend(incoming.clone());
        }

        // Remove all connected edges
        for edge_id in edges_to_remove {
            let _ = self.remove_edge(&edge_id);
        }

        // Remove adjacency lists
        self.adjacency_out.remove(node_id);
        self.adjacency_in.remove(node_id);

        Ok(node)
    }

    /// Remove an edge
    pub fn remove_edge(&mut self, edge_id: &str) -> Result<Edge, GraphError> {
        let edge = self
            .edges
            .remove(edge_id)
            .ok_or_else(|| GraphError::EdgeNotFound(edge_id.to_string()))?;

        // Remove from label index
        if let Some(edges) = self.edge_labels.get_mut(&edge.label) {
            edges.retain(|id| id != edge_id);
            if edges.is_empty() {
                self.edge_labels.remove(&edge.label);
            }
        }

        // Remove from adjacency lists
        if let Some(outgoing) = self.adjacency_out.get_mut(&edge.from_node) {
            outgoing.retain(|id| id != edge_id);
        }
        if let Some(incoming) = self.adjacency_in.get_mut(&edge.to_node) {
            incoming.retain(|id| id != edge_id);
        }

        Ok(edge)
    }

    /// Get graph statistics
    pub fn stats(&self) -> GraphStats {
        GraphStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.len(),
            node_label_count: self.node_labels.len(),
            edge_label_count: self.edge_labels.len(),
        }
    }

    /// Check if the graph contains a node
    pub fn contains_node(&self, node_id: &str) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Check if the graph contains an edge
    pub fn contains_edge(&self, edge_id: &str) -> bool {
        self.edges.contains_key(edge_id)
    }

    /// Get all node IDs
    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.nodes.keys()
    }

    /// Get all edge IDs
    pub fn edge_ids(&self) -> impl Iterator<Item = &String> {
        self.edges.keys()
    }

    /// Create a new empty graph without metadata support (for baseline performance)

    /// Add a metadata-enabled node to the graph

    /// Add a metadata-enabled edge to the graph

    /// Get a metadata node by ID

    /// Get a mutable metadata node by ID

    /// Get a metadata edge by ID

    /// Get a mutable metadata edge by ID

    /// Clean up expired nodes and edges based on TTL

    /// Remove a metadata node and all its connected edges

    /// Remove a metadata edge

    /// Get all metadata nodes

    /// Get all metadata edges

    /// Clear all data from the graph
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.node_labels.clear();
        self.edge_labels.clear();
        self.adjacency_out.clear();
        self.adjacency_in.clear();
    }
}

impl Default for GraphCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub node_label_count: usize,
    pub edge_label_count: usize,
}
