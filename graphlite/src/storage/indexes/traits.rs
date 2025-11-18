// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Core traits for the indexing system

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::{IndexError, IndexStatistics, IndexType, SearchQuery, SearchResult};
use crate::storage::Value;

/// Core trait that all indexes must implement
#[async_trait]
#[allow(dead_code)] // ROADMAP v0.4.0 - Core index trait for advanced indexing system
pub trait Index: Send + Sync {
    /// Insert or update an entry in the index
    async fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), IndexError>;

    /// Search the index with a query
    async fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, IndexError>;

    /// Delete an entry from the index
    async fn delete(&mut self, key: &[u8]) -> Result<bool, IndexError>;

    /// Check if a key exists in the index
    async fn contains(&self, key: &[u8]) -> Result<bool, IndexError>;

    /// Get the size of the index (number of entries)
    fn size(&self) -> usize;

    /// Get index statistics
    fn stats(&self) -> &IndexStatistics;

    /// Get the index name
    fn name(&self) -> &str;

    /// Get the index type
    fn index_type(&self) -> &IndexType;

    /// Perform maintenance operations (compaction, optimization, etc.)
    async fn maintenance(&mut self) -> Result<(), IndexError>;

    /// Flush any pending writes to storage
    async fn flush(&mut self) -> Result<(), IndexError>;

    /// Get memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Check if the index is ready for operations
    fn is_ready(&self) -> bool;

    /// Shutdown the index and clean up resources
    async fn shutdown(&mut self) -> Result<(), IndexError>;

    /// Downcast support for accessing concrete index types
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Trait for indexes that support batch operations
#[async_trait]
#[allow(dead_code)] // ROADMAP v0.4.0 - Batch index operations for bulk data loading
pub trait BatchIndex: Index {
    /// Insert multiple entries at once
    async fn batch_insert(&mut self, entries: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), IndexError>;

    /// Delete multiple entries at once
    async fn batch_delete(&mut self, keys: Vec<Vec<u8>>) -> Result<usize, IndexError>;

    /// Search with multiple queries
    async fn batch_search(&self, queries: Vec<SearchQuery>) -> Result<Vec<Vec<SearchResult>>, IndexError>;
}

/// Trait for indexes that support graph operations
#[async_trait]
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph-aware indexes for traversal optimization
pub trait GraphIndex: Index {
    /// Add an edge to the graph index
    async fn add_edge(&mut self, source: &str, target: &str, properties: Option<HashMap<String, Value>>) -> Result<(), IndexError>;

    /// Remove an edge from the graph index
    async fn remove_edge(&mut self, source: &str, target: &str) -> Result<bool, IndexError>;

    /// Get all neighbors of a node
    async fn get_neighbors(&self, node: &str, direction: super::Direction) -> Result<Vec<String>, IndexError>;

    /// Check if there's a path between two nodes
    async fn has_path(&self, source: &str, target: &str, max_hops: Option<usize>) -> Result<bool, IndexError>;

    /// Find shortest path between two nodes
    async fn shortest_path(&self, source: &str, target: &str) -> Result<Option<Vec<String>>, IndexError>;

    /// Get node degree
    async fn degree(&self, node: &str) -> Result<usize, IndexError>;
}

/// Trait for partition-aware indexes (for future distribution)
#[async_trait]
#[allow(dead_code)] // ROADMAP v0.4.0 - Partitioned indexes for scalability
pub trait PartitionedIndex: Index {
    /// Get partition ID for a given key
    fn get_partition(&self, key: &[u8]) -> String;

    /// Get all partitions
    fn get_partitions(&self) -> Vec<String>;

    /// Get statistics for a specific partition
    fn partition_stats(&self, partition_id: &str) -> Option<IndexStatistics>;

    /// Rebalance partitions
    async fn rebalance(&mut self) -> Result<(), IndexError>;
}

/// Index lifecycle management
#[async_trait]
#[allow(dead_code)] // ROADMAP v0.4.0 - Index lifecycle management (create, load, save, drop)
pub trait IndexLifecycle {
    /// Create a new index
    async fn create(&mut self) -> Result<(), IndexError>;

    /// Load existing index from storage
    async fn load(&mut self) -> Result<(), IndexError>;

    /// Save index to storage
    async fn save(&self) -> Result<(), IndexError>;

    /// Drop/delete the index
    async fn drop(&mut self) -> Result<(), IndexError>;

    /// Get index metadata
    fn metadata(&self) -> IndexMetadata;
}

/// Index metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMetadata {
    /// Index name
    pub name: String,
    /// Index type
    pub index_type: IndexType,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last modified timestamp
    pub modified_at: chrono::DateTime<chrono::Utc>,
    /// Index version
    pub version: String,
    /// Custom properties
    pub properties: HashMap<String, Value>,
}
