// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Storage driver traits
//!
//! This module defines the core traits for storage drivers and trees.
//! All storage drivers must implement these traits to provide a consistent interface.

use super::types::{StorageResult, StorageType};
use std::collections::HashMap;
use std::path::Path;

/// Trait for a tree/column family in the storage driver
///
/// Represents a named collection of key-value pairs within a storage driver.
/// Similar to a table in SQL databases or a column family in NoSQL databases.
pub trait StorageTree: Send + Sync {
    /// Insert a key-value pair
    fn insert(&self, key: &[u8], value: &[u8]) -> StorageResult<()>;

    /// Get a value by key
    fn get(&self, key: &[u8]) -> StorageResult<Option<Vec<u8>>>;

    /// Remove a key-value pair
    fn remove(&self, key: &[u8]) -> StorageResult<()>;

    /// Check if a key exists
    fn contains_key(&self, key: &[u8]) -> StorageResult<bool>;

    /// Clear all data in the tree
    fn clear(&self) -> StorageResult<()>;

    /// Check if the tree is empty
    fn is_empty(&self) -> StorageResult<bool>;

    /// Iterate over all key-value pairs
    fn iter(
        &self,
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>>;

    /// Scan with a key prefix
    fn scan_prefix(
        &self,
        prefix: &[u8],
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>>;

    /// Get multiple values by keys (batch get)
    fn batch_get(&self, keys: &[&[u8]]) -> StorageResult<Vec<Option<Vec<u8>>>>;

    /// Insert multiple key-value pairs (batch insert)
    fn batch_insert(&self, entries: &[(&[u8], &[u8])]) -> StorageResult<()>;

    /// Remove multiple keys (batch remove)
    fn batch_remove(&self, keys: &[&[u8]]) -> StorageResult<()>;

    /// Flush any pending writes to disk
    fn flush(&self) -> StorageResult<()>;
}

/// Main storage driver trait
///
/// Defines the interface that all storage drivers must implement.
/// Provides methods for opening databases, managing trees, and basic operations.
pub trait StorageDriver: Send + Sync {
    /// Type of tree/column family used by this driver
    type Tree: StorageTree;

    /// Open or create a storage driver at the given path
    fn open<P: AsRef<Path>>(path: P) -> StorageResult<Self>
    where
        Self: Sized;

    /// Open or create a named tree/column family
    fn open_tree(&self, name: &str) -> StorageResult<Self::Tree>;

    /// List all available trees/column families
    fn list_trees(&self) -> StorageResult<Vec<String>>;

    /// Flush all pending writes to disk
    fn flush(&self) -> StorageResult<()>;

    /// Get storage type
    fn storage_type(&self) -> StorageType;

    /// Open or create a tree with specific options for indexes
    fn open_index_tree(
        &self,
        name: &str,
        index_options: IndexTreeOptions,
    ) -> StorageResult<Self::Tree>;

    /// List all available indexes
    fn list_indexes(&self) -> StorageResult<Vec<String>>;

    /// Drop an index tree
    fn drop_index(&self, name: &str) -> StorageResult<()>;

    /// Get statistics for a tree
    fn tree_stats(&self, name: &str) -> StorageResult<Option<TreeStatistics>>;

    /// Explicitly close the storage driver and release any file locks
    /// This is called before dropping to ensure clean shutdown
    fn shutdown(&mut self) -> StorageResult<()> {
        // Default implementation just flushes
        self.flush()
    }
}

// Helper implementation for Box<dyn StorageTree>
// This allows us to use boxed trait objects seamlessly
impl StorageTree for Box<dyn StorageTree> {
    fn insert(&self, key: &[u8], value: &[u8]) -> StorageResult<()> {
        (**self).insert(key, value)
    }

    fn get(&self, key: &[u8]) -> StorageResult<Option<Vec<u8>>> {
        (**self).get(key)
    }

    fn remove(&self, key: &[u8]) -> StorageResult<()> {
        (**self).remove(key)
    }

    fn contains_key(&self, key: &[u8]) -> StorageResult<bool> {
        (**self).contains_key(key)
    }

    fn clear(&self) -> StorageResult<()> {
        (**self).clear()
    }

    fn is_empty(&self) -> StorageResult<bool> {
        (**self).is_empty()
    }

    fn iter(
        &self,
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        (**self).iter()
    }

    fn flush(&self) -> StorageResult<()> {
        (**self).flush()
    }

    fn scan_prefix(
        &self,
        prefix: &[u8],
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        (**self).scan_prefix(prefix)
    }

    fn batch_get(&self, keys: &[&[u8]]) -> StorageResult<Vec<Option<Vec<u8>>>> {
        (**self).batch_get(keys)
    }

    fn batch_insert(&self, entries: &[(&[u8], &[u8])]) -> StorageResult<()> {
        (**self).batch_insert(entries)
    }

    fn batch_remove(&self, keys: &[&[u8]]) -> StorageResult<()> {
        (**self).batch_remove(keys)
    }
}

/// Options for creating index trees
#[derive(Debug, Clone)]
pub struct IndexTreeOptions {
    /// Index type for optimization hints
    pub index_type: String,
    /// Compression enabled
    pub compression: bool,
    /// Block cache size in bytes
    pub block_cache_size: Option<usize>,
    /// Write buffer size in bytes
    pub write_buffer_size: Option<usize>,
    /// Bloom filter bits per key
    pub bloom_filter_bits: Option<u32>,
    /// Custom options
    pub custom_options: HashMap<String, String>,
}

impl Default for IndexTreeOptions {
    fn default() -> Self {
        Self {
            index_type: "generic".to_string(),
            compression: true,
            block_cache_size: Some(64 * 1024 * 1024), // 64MB
            write_buffer_size: Some(16 * 1024 * 1024), // 16MB
            bloom_filter_bits: Some(10),
            custom_options: HashMap::new(),
        }
    }
}

impl IndexTreeOptions {
    /// Create options optimized for graph indexes
    pub fn for_graph_index() -> Self {
        Self {
            index_type: "graph".to_string(),
            compression: false, // Graph data often doesn't compress well
            block_cache_size: Some(64 * 1024 * 1024), // 64MB
            write_buffer_size: Some(16 * 1024 * 1024), // 16MB
            bloom_filter_bits: Some(12),
            custom_options: HashMap::new(),
        }
    }
}

/// Tree statistics for monitoring
#[derive(Debug, Clone)]
pub struct TreeStatistics {
    /// Number of entries
    pub entry_count: u64,
    /// Total size in bytes
    pub size_bytes: u64,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// Number of levels (for LSM trees)
    pub levels: Option<u32>,
    /// Compaction statistics
    pub compaction_stats: Option<CompactionStats>,
}

/// Compaction statistics
#[derive(Debug, Clone)]
pub struct CompactionStats {
    /// Number of compactions
    pub compaction_count: u64,
    /// Last compaction timestamp
    pub last_compaction: Option<chrono::DateTime<chrono::Utc>>,
    /// Bytes written during compaction
    pub bytes_written: u64,
    /// Bytes read during compaction
    pub bytes_read: u64,
}
