// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Graph data structures and error types
//!
//! Defines Node and Edge structures for the in-memory graph,
//! along with error types for graph operations.

use crate::storage::value::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Error types for graph operations
#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    
    #[error("Edge not found: {0}")]
    EdgeNotFound(String),
    
    #[error("Node already exists: {0}")]
    NodeAlreadyExists(String),
    
    #[error("Edge already exists: {0}")]
    EdgeAlreadyExists(String),
    
    #[error("Invalid edge: from node {from} to node {to} - one or both nodes don't exist")]
    InvalidEdge { from: String, to: String },
    
    #[error("Property error: {0}")]
    PropertyError(String),
    
    #[error("Index error: {0}")]
    IndexError(String),
}

/// Error types for storage operations (including multi-graph operations)
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),
    
    #[error("Graph not found: {0}")]
    GraphNotFound(String),
    
    #[error("Lock error: {0}")]
    LockError(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    
    #[error("Session error: {0}")]
    SessionError(String),
    
    #[error("User not found: {0}")]
    UserNotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Persistence error: {0}")]
    PersistenceError(String),
}

/// Graph node with id, labels, and properties
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Node {
    pub id: String,
    pub labels: Vec<String>,
    pub properties: HashMap<String, Value>,
}

impl Node {
    /// Create a new node with the given id
    pub fn new(id: String) -> Self {
        Self {
            id,
            labels: Vec::new(),
            properties: HashMap::new(),
        }
    }
    
    /// Create a new node with id and labels
    pub fn with_labels(id: String, labels: Vec<String>) -> Self {
        Self {
            id,
            labels,
            properties: HashMap::new(),
        }
    }
    
    /// Add a label to this node
    pub fn add_label(&mut self, label: String) {
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }
    
    /// Check if node has a specific label
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.iter().any(|l| l == label)
    }
    
    /// Set a property value
    pub fn set_property(&mut self, key: String, value: Value) {
        self.properties.insert(key, value);
    }
    
    /// Get a property value
    pub fn get_property(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }
    
    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<Value> {
        self.properties.remove(key)
    }
    
    /// Check if node has a specific property
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }
}

/// Graph edge with id, from/to nodes, label, and properties
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Edge {
    pub id: String,
    pub from_node: String,
    pub to_node: String,
    pub label: String,
    pub properties: HashMap<String, Value>,
}

impl Edge {
    /// Create a new edge
    pub fn new(id: String, from_node: String, to_node: String, label: String) -> Self {
        Self {
            id,
            from_node,
            to_node,
            label,
            properties: HashMap::new(),
        }
    }
    
    /// Set a property value
    pub fn set_property(&mut self, key: String, value: Value) {
        self.properties.insert(key, value);
    }
    
    /// Get a property value
    pub fn get_property(&self, key: &str) -> Option<&Value> {
        self.properties.get(key)
    }
    
    /// Remove a property
    pub fn remove_property(&mut self, key: &str) -> Option<Value> {
        self.properties.remove(key)
    }
    
    /// Check if edge has a specific property
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }
    
    /// Check if this edge connects the given nodes (in either direction)
    pub fn connects(&self, node1: &str, node2: &str) -> bool {
        (self.from_node == node1 && self.to_node == node2) ||
        (self.from_node == node2 && self.to_node == node1)
    }
    
    /// Check if this edge goes from node1 to node2
    pub fn goes_from_to(&self, from: &str, to: &str) -> bool {
        self.from_node == from && self.to_node == to
    }
}
