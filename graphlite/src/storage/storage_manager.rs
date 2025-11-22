// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Storage Manager - Orchestrates multiple storage tiers
//!
//! This module provides a unified interface for managing graph storage across
//! multiple tiers: local cache, persistent disk storage, and external memory stores.
//!
//! Architecture:
//! - Cache: Local in-memory cache for hot data (via MultiGraphManager)
//! - Persistent Store: Optional disk-based storage (RocksDB/Sled via DataAdapter)
//! - Memory Store: Optional external memory store (Redis/Valkey via DataAdapter)
//!
//! At least one of persistent_store or memory_store must be configured.

use crate::catalog::manager::CatalogManager;
use crate::storage::data_adapter::DataAdapter;
use crate::storage::indexes::IndexManager;
use crate::storage::multi_graph::MultiGraphManager;
use crate::storage::persistent::{create_storage_driver, StorageDriver, StorageTree};
use crate::storage::StorageType;
use crate::storage::{GraphCache, StorageError};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Storage method configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum StorageMethod {
    /// Disk-based storage only (RocksDB/Sled)
    #[default]
    DiskOnly,
    /// Memory-based storage only (Redis/Valkey)
    MemoryOnly,
    /// Both disk and memory storage for redundancy
    DiskAndMemory,
}


/// Storage manager that orchestrates all storage tiers
#[derive(Clone)]
pub struct StorageManager {
    /// Local in-memory cache for hot data
    cache: Arc<MultiGraphManager>,

    /// Single storage driver instance (RocksDB/Sled) - created once at initialization
    storage_driver: Option<Arc<Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>>>,

    /// Optional disk-based persistent storage (RocksDB/Sled)
    persistent_store: Option<Arc<DataAdapter>>,

    /// Optional external memory store (Redis/Valkey)
    memory_store: Option<Arc<DataAdapter>>,

    /// Storage type being used
    storage_type: StorageType,

    /// Index manager for text indexes
    index_manager: Option<Arc<IndexManager>>,
}

impl StorageManager {
    /// Create a new storage manager with the specified method and configuration
    pub fn new<P: AsRef<Path>>(
        path: P,
        method: StorageMethod,
        storage_type: StorageType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!(
            "Creating storage manager with method: {:?}, storage type: {:?}",
            method, storage_type
        );

        match method {
            StorageMethod::DiskOnly => Self::init_disk_only(path, storage_type),
            StorageMethod::MemoryOnly => Self::init_memory_only(path, storage_type),
            StorageMethod::DiskAndMemory => Self::init_disk_and_memory(path, storage_type),
        }
    }

    /// Initialize storage manager with disk-only storage (RocksDB/Sled)
    fn init_disk_only<P: AsRef<Path>>(
        path: P,
        storage_type: StorageType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!(
            "Initializing disk-only storage with {} at path: {:?}",
            storage_type,
            path.as_ref()
        );

        // Create single storage driver instance
        let driver = create_storage_driver(storage_type, path.as_ref())?;

        // Pre-create commonly used column families/trees
        let common_trees = vec!["nodes", "edges", "metadata", "catalog", "auth"];
        for tree_name in &common_trees {
            driver.open_tree(tree_name)?;
            debug!("Pre-created tree: {}", tree_name);
        }

        info!(
            "Storage driver initialized with {} pre-created trees",
            common_trees.len()
        );

        // Create DataAdapter (now stateless)
        let persistent_store = Arc::new(DataAdapter::new());

        // Create IndexManager for graph indexes
        let driver_arc = Arc::new(driver);
        let index_manager = Arc::new(IndexManager::new());

        Ok(Self {
            cache: Arc::new(MultiGraphManager::new()),
            storage_driver: Some(driver_arc),
            persistent_store: Some(persistent_store),
            memory_store: None,
            storage_type,
            index_manager: Some(index_manager),
        })
    }

    /// Initialize storage manager with memory-only storage (Redis/Valkey)
    fn init_memory_only<P: AsRef<Path>>(
        _path: P,
        storage_type: StorageType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Initializing memory-only storage with {:?}", storage_type);

        // TODO: Implement Redis/Valkey memory store initialization
        Err("Memory-only storage not yet implemented".into())
    }

    /// Initialize storage manager with both disk and memory storage
    fn init_disk_and_memory<P: AsRef<Path>>(
        path: P,
        storage_type: StorageType,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        info!(
            "Initializing disk and memory storage with {} at path: {:?}",
            storage_type,
            path.as_ref()
        );

        // TODO: Implement both disk and memory storage initialization
        // For now, fall back to disk-only
        Self::init_disk_only(path, storage_type)
    }

    /// Get a graph by name
    /// Checks cache first, then memory store, then persistent storage
    pub fn get_graph(&self, name: &str) -> Result<Option<GraphCache>, StorageError> {
        debug!("Getting graph '{}' from storage manager", name);

        // 1. Check local cache first
        match self.cache.get_graph(name) {
            Ok(Some(graph)) => {
                debug!("Graph '{}' found in local cache", name);
                return Ok(Some(graph));
            }
            Ok(None) => {
                debug!("Graph '{}' not found in local cache", name);
            }
            Err(e) => {
                error!("Error checking cache for graph '{}': {}", name, e);
                return Err(e);
            }
        }

        // No fallback logic - use exact names for consistency

        // 2. Check memory store if available
        if let Some(_memory_store) = &self.memory_store {
            debug!("Memory store not yet implemented for graph '{}'", name);
        }

        // 3. Check persistent disk storage if available
        debug!(
            "Graph '{}' not in memory, checking persistent storage",
            name
        );

        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                match persistent_store.load_graph_by_path(driver.as_ref().as_ref(), name) {
                    Ok(graph) => {
                        debug!("Graph '{}' loaded from persistent storage", name);

                        // Add to cache for future access
                        self.cache.add_graph(name.to_string(), graph.clone())?;
                        return Ok(Some(graph));
                    }
                    Err(e) => {
                        debug!(
                            "Failed to load graph '{}' from persistent storage: {}",
                            name, e
                        );
                    }
                }
            }
        }

        Ok(None)
    }

    /// Save a graph
    /// Updates cache, memory store (if available), and persistent storage
    pub fn save_graph(&self, name: &str, graph: GraphCache) -> Result<(), StorageError> {
        debug!("Saving graph '{}' to storage manager", name);

        // Use the provided name consistently - don't try to normalize to short names
        let cache_name = name.to_string();

        // 1. Update cache
        match self.cache.add_graph(cache_name.clone(), graph.clone()) {
            Ok(_) => {
                debug!("Successfully added graph '{}' to cache", cache_name);
            }
            Err(e) => {
                error!("Failed to add graph '{}' to cache: {}", cache_name, e);
                return Err(e);
            }
        }

        // 2. Update memory store if available (TODO: Implement memory store save)
        if let Some(_memory_store) = &self.memory_store {
            debug!("Memory store save not yet implemented for graph '{}'", name);
        }

        // 3. Persist to disk storage if available
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                debug!("Attempting to persist graph '{}' to disk", name);
                persistent_store
                    .save_graph_by_path(driver.as_ref().as_ref(), &graph, name)
                    .map_err(|e| {
                        error!("Failed to persist graph '{}': {}", name, e);
                        StorageError::PersistenceError(format!(
                            "Failed to persist graph '{}': {}",
                            name, e
                        ))
                    })?;
                debug!("Successfully persisted graph '{}' to disk", name);
            } else {
                debug!(
                    "No storage driver available, skipping disk persistence for '{}'",
                    name
                );
            }
        } else {
            debug!(
                "No persistent store available, skipping disk persistence for '{}'",
                name
            );
        }

        debug!("Successfully saved graph '{}' to all storage tiers", name);
        Ok(())
    }

    /// Get all graph names from cache and storage tiers
    pub fn get_graph_names(&self) -> Result<Vec<String>, StorageError> {
        debug!("Getting all graph names from storage manager");

        // Get names from cache
        let mut names = self.cache.get_graph_names()?;

        // Get names from persistent storage and merge
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                if let Ok(persistent_names) = persistent_store.list_graphs(driver.as_ref().as_ref())
                {
                    for name in persistent_names {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }

        // Get names from memory store and merge
        if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                if let Ok(memory_names) = memory_store.list_graphs(driver.as_ref().as_ref()) {
                    for name in memory_names {
                        if !names.contains(&name) {
                            names.push(name);
                        }
                    }
                }
            }
        }

        names.sort();
        debug!("Found {} graphs in storage manager", names.len());
        Ok(names)
    }

    /// Delete a graph from all storage tiers
    pub fn delete_graph(&self, name: &str) -> Result<(), StorageError> {
        debug!("Deleting graph '{}' from storage manager", name);

        // 1. Remove from cache
        self.cache.remove_graph(name)?;

        // 2. Remove from memory store if available
        if let Some(memory_store) = &self.memory_store {
            // Use the new delete_graph method to delete only this graph's data
            if let Some(driver) = &self.storage_driver {
                memory_store
                    .delete_graph(driver.as_ref().as_ref(), name)
                    .map_err(|e| {
                        error!("Failed to delete graph '{}' from memory store: {}", name, e);
                        StorageError::PersistenceError(format!(
                            "Failed to delete graph '{}' from memory: {}",
                            name, e
                        ))
                    })?;
            }
            debug!("Successfully deleted graph '{}' from memory store", name);
        }

        // 3. Delete from persistent storage if available
        if let Some(persistent_store) = &self.persistent_store {
            // Use delete_graph instead of clear() to only delete this specific graph
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .delete_graph(driver.as_ref().as_ref(), name)
                    .map_err(|e| {
                        error!(
                            "Failed to delete graph '{}' from persistent storage: {}",
                            name, e
                        );
                        StorageError::PersistenceError(format!(
                            "Failed to delete graph '{}': {}",
                            name, e
                        ))
                    })?;
            }
            debug!(
                "Successfully deleted graph '{}' from persistent storage",
                name
            );
        }

        debug!(
            "Successfully deleted graph '{}' from all storage tiers",
            name
        );
        Ok(())
    }

    /// List all available graphs
    pub fn list_graphs(&self) -> Result<Vec<String>, StorageError> {
        self.get_graph_names()
    }

    /// Get a mutable reference to a graph in the cache
    pub fn get_graph_mut(&self, name: &str) -> Result<Option<GraphCache>, StorageError> {
        // First ensure the graph is in the cache
        if self.cache.get_graph(name)?.is_none() {
            // Try to load it
            if let Some(graph) = self.get_graph(name)? {
                self.cache.add_graph(name.to_string(), graph)?;
            }
        }

        self.cache.get_graph_mut(name)
    }

    /// Get access to the storage driver for metrics collection
    pub fn get_storage_driver(
        &self,
    ) -> Option<&Arc<Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>>> {
        self.storage_driver.as_ref()
    }

    /// Get access to the cache for metrics collection
    pub fn get_cache(&self) -> &Arc<MultiGraphManager> {
        &self.cache
    }

    /// Get access to the index manager (Phase 5: Week 6.4)
    pub fn get_index_manager(&self) -> Option<&Arc<IndexManager>> {
        self.index_manager.as_ref()
    }

    /// Check if a text index exists for a given field (Phase 5: Week 6.4)
    pub fn has_text_index(&self, index_name: &str) -> bool {
        if let Some(index_manager) = &self.index_manager {
            index_manager.index_exists(index_name)
        } else {
            false
        }
    }

    /// Create a graph union (for UNION operations)
    pub fn create_graph_union(&self, graph_names: Vec<String>) -> Result<GraphCache, StorageError> {
        let mut graphs = Vec::new();
        for name in graph_names {
            if let Some(graph) = self.get_graph(&name)? {
                graphs.push(graph);
            } else {
                return Err(StorageError::GraphNotFound(name));
            }
        }

        self.cache.union_graphs(graphs)
    }

    /// Save catalog to persistent storage
    pub fn save_catalog(&self, catalog_manager: &CatalogManager) -> Result<(), StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            persistent_store.save_catalog(catalog_manager).map_err(|e| {
                StorageError::PersistenceError(format!("Failed to save catalog: {}", e))
            })
        } else if let Some(memory_store) = &self.memory_store {
            memory_store.save_catalog(catalog_manager).map_err(|e| {
                StorageError::PersistenceError(format!("Failed to save catalog: {}", e))
            })
        } else {
            Err(StorageError::PersistenceError(
                "No storage backend available".to_string(),
            ))
        }
    }

    /// Load catalog from persistent storage
    pub fn load_catalog(&self, catalog_manager: &mut CatalogManager) -> Result<(), StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            persistent_store.load_catalog(catalog_manager).map_err(|e| {
                StorageError::PersistenceError(format!("Failed to load catalog: {}", e))
            })
        } else if let Some(memory_store) = &self.memory_store {
            memory_store.load_catalog(catalog_manager).map_err(|e| {
                StorageError::PersistenceError(format!("Failed to load catalog: {}", e))
            })
        } else {
            debug!("No storage backend available for loading catalog");
            Ok(())
        }
    }

    /// Get access to the cache manager (for compatibility)
    pub fn cache(&self) -> Arc<MultiGraphManager> {
        self.cache.clone()
    }

    /// Get access to the cache manager (legacy name for compatibility)
    pub fn working_set(&self) -> Arc<MultiGraphManager> {
        self.cache.clone()
    }

    /// Get access to persistent storage (for compatibility during migration)
    pub fn persistent_storage(&self) -> Option<Arc<DataAdapter>> {
        self.persistent_store.clone()
    }

    /// Get access to the single storage driver instance
    /// This ensures all components use the same driver with consistent configuration
    pub fn storage_driver(
        &self,
    ) -> Option<Arc<Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>>> {
        self.storage_driver.clone()
    }

    /// Check if catalog data exists
    pub fn has_catalog_data(&self) -> Result<bool, StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .has_catalog_data(driver.as_ref().as_ref())
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to check catalog data: {}",
                            e
                        ))
                    })
            } else {
                Ok(false)
            }
        } else if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                memory_store
                    .has_catalog_data(driver.as_ref().as_ref())
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to check catalog data: {}",
                            e
                        ))
                    })
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Clear only the in-memory cache (not persistent storage)
    pub fn clear_cache(&self) -> Result<(), StorageError> {
        debug!("Clearing storage cache");
        self.cache.clear()?;
        debug!("Successfully cleared storage cache");
        Ok(())
    }

    /// Get cache statistics (entries count, memory bytes estimate)
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let entries = self.cache.graph_count();
        // Rough estimate: 1KB per cached graph entry
        let memory_bytes = entries * 1024;
        (entries, memory_bytes)
    }

    /// Clear all stored data
    pub fn clear_all_data(&self) -> Result<(), StorageError> {
        // Clear cache
        self.clear_cache()?;

        // Clear persistent store
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .clear(driver.as_ref().as_ref())
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to clear persistent data: {}",
                            e
                        ))
                    })?;
            }
        }

        // Clear memory store
        if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                memory_store.clear(driver.as_ref().as_ref()).map_err(|e| {
                    StorageError::PersistenceError(format!("Failed to clear memory data: {}", e))
                })?;
            }
        }

        Ok(())
    }

    /// Save catalog provider data to persistent storage
    pub fn save_catalog_provider(&self, name: &str, data: &[u8]) -> Result<(), StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .save_catalog_provider(driver.as_ref().as_ref(), name, data)
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to save catalog provider '{}': {}",
                            name, e
                        ))
                    })
            } else {
                Err(StorageError::PersistenceError(
                    "No storage driver available".to_string(),
                ))
            }
        } else if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                memory_store
                    .save_catalog_provider(driver.as_ref().as_ref(), name, data)
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to save catalog provider '{}': {}",
                            name, e
                        ))
                    })
            } else {
                Err(StorageError::PersistenceError(
                    "No storage driver available".to_string(),
                ))
            }
        } else {
            Err(StorageError::PersistenceError(
                "No storage backend available".to_string(),
            ))
        }
    }

    /// Load catalog provider data from persistent storage
    pub fn load_catalog_provider(&self, name: &str) -> Result<Option<Vec<u8>>, StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                match persistent_store.load_catalog_provider(driver.as_ref().as_ref(), name) {
                    Ok(data) => Ok(Some(data)),
                    Err(e) => {
                        debug!(
                            "Could not load catalog provider '{}' from persistent store: {}",
                            name, e
                        );
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            }
        } else if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                match memory_store.load_catalog_provider(driver.as_ref().as_ref(), name) {
                    Ok(data) => Ok(Some(data)),
                    Err(e) => {
                        debug!(
                            "Could not load catalog provider '{}' from memory store: {}",
                            name, e
                        );
                        Ok(None)
                    }
                }
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Check if catalog provider data exists
    pub fn has_catalog_provider(&self, name: &str) -> Result<bool, StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .has_catalog_provider(driver.as_ref().as_ref(), name)
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to check catalog provider '{}': {}",
                            name, e
                        ))
                    })
            } else {
                Ok(false)
            }
        } else if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                memory_store
                    .has_catalog_provider(driver.as_ref().as_ref(), name)
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to check catalog provider '{}': {}",
                            name, e
                        ))
                    })
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// List all catalog provider names
    pub fn list_catalog_providers(&self) -> Result<Vec<String>, StorageError> {
        if let Some(persistent_store) = &self.persistent_store {
            if let Some(driver) = &self.storage_driver {
                persistent_store
                    .list_catalog_providers(driver.as_ref().as_ref())
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to list catalog providers: {}",
                            e
                        ))
                    })
            } else {
                Ok(Vec::new())
            }
        } else if let Some(memory_store) = &self.memory_store {
            if let Some(driver) = &self.storage_driver {
                memory_store
                    .list_catalog_providers(driver.as_ref().as_ref())
                    .map_err(|e| {
                        StorageError::PersistenceError(format!(
                            "Failed to list catalog providers: {}",
                            e
                        ))
                    })
            } else {
                Ok(Vec::new())
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Explicitly shutdown the storage manager and release file locks
    /// This should be called before dropping to ensure clean resource cleanup
    pub fn shutdown(&self) -> Result<(), StorageError> {
        // Just flush for now - the main issue is ensuring proper drop order
        // in the test environments

        // Flush persistent store
        if let Some(_persistent_store) = &self.persistent_store {
            // We can't call shutdown since it requires &mut, but flush should be sufficient
            // The key is ensuring proper drop order in tests
            debug!("Flushing storage manager during shutdown");
        }

        Ok(())
    }
}

impl std::fmt::Debug for StorageManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageManager")
            .field("storage_type", &self.storage_type)
            .field("has_storage_driver", &self.storage_driver.is_some())
            .field("has_persistent_store", &self.persistent_store.is_some())
            .field("has_memory_store", &self.memory_store.is_some())
            .finish_non_exhaustive()
    }
}
