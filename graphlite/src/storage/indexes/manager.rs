// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Index manager for GraphLite
//!
//! Simplified index manager that supports only graph indexes.

use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use log::{debug, info, warn};

use super::{IndexError};
use crate::storage::GraphCache;

/// Manager for all indexes in the system
pub struct IndexManager {
    /// Index names storage
    index_names: Arc<RwLock<HashSet<String>>>,
}

impl IndexManager {
    /// Create a new index manager
    pub fn new() -> Self {
        Self {
            index_names: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create a new index
    pub async fn create_index(
        &self,
        name: String,
        _index_type: super::IndexType,
        _config: super::IndexConfig,
    ) -> Result<(), IndexError> {
        info!("Creating index '{}'", name);

        let mut index_names = self.index_names.write().map_err(|e| {
            IndexError::creation(format!("Failed to acquire lock: {}", e))
        })?;

        if index_names.contains(&name) {
            return Err(IndexError::AlreadyExists(name));
        }

        // Store index name
        index_names.insert(name.clone());

        debug!("Index '{}' created successfully", name);
        Ok(())
    }

    /// Delete an index
    pub async fn delete_index(&self, name: &str) -> Result<(), IndexError> {
        info!("Deleting index '{}'", name);

        let mut index_names = self.index_names.write().map_err(|e| {
            IndexError::creation(format!("Failed to acquire lock: {}", e))
        })?;

        if !index_names.remove(name) {
            return Err(IndexError::NotFound(name.to_string()));
        }

        debug!("Index '{}' deleted successfully", name);
        Ok(())
    }

    /// Check if an index exists
    pub fn index_exists(&self, name: &str) -> bool {
        self.index_names.read()
            .map(|names| names.contains(name))
            .unwrap_or(false)
    }

    /// List all index names
    pub fn list_indexes(&self) -> Vec<String> {
        self.index_names.read()
            .map(|names| names.iter().cloned().collect())
            .unwrap_or_else(|_| Vec::new())
    }

    /// Reindex a text index (stub for compatibility)
    pub fn reindex_text_index(&self, _name: &str, _graph: &Arc<GraphCache>) -> Result<usize, IndexError> {
        warn!("Text index reindexing not supported in GraphLite");
        Ok(0)
    }

    /// Search an index synchronously (stub for compatibility)
    pub fn search_index_sync(
        &self,
        _index_name: &str,
        _query_text: &str,
        _limit: usize,
    ) -> Result<Vec<(String, f32)>, IndexError> {
        warn!("Text search not supported in GraphLite");
        Ok(Vec::new())
    }

    /// Find indexes for a label (stub for compatibility)
    pub fn find_indexes_for_label(&self, _label: &str) -> Vec<String> {
        Vec::new()
    }

    /// Find index by label and property (stub for compatibility)
    pub fn find_index_by_label_and_property(&self, _label: &str, _property: &str) -> Option<String> {
        None
    }
}
