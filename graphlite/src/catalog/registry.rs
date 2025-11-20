// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Catalog registry implementation
//!
//! This module provides the CatalogRegistry that manages all registered catalog providers.
//! The registry is responsible for catalog initialization, storage integration, and
//! providing access to catalog instances.

use super::error::{CatalogError, CatalogResult};
use super::providers;
use super::traits::CatalogProvider;
use crate::storage::StorageManager;
use std::collections::HashMap;
use std::sync::Arc;

/// Central registry for all catalog providers
///
/// The CatalogRegistry manages all registered catalog types and provides a unified
/// interface for accessing and managing them. It handles initialization with storage
/// and maintains the catalog instances.
pub struct CatalogRegistry {
    /// Map of catalog name to catalog provider instance
    catalogs: HashMap<String, Box<dyn CatalogProvider>>,
    /// Reference to storage manager for persistence
    storage: Arc<StorageManager>,
}

impl CatalogRegistry {
    /// Create a new catalog registry with storage manager
    ///
    /// This automatically registers all available catalog providers and initializes
    /// them with the provided storage manager.
    ///
    /// # Arguments
    /// * `storage` - Arc reference to storage manager for catalog persistence
    ///
    /// # Returns
    /// * `Self` - Initialized catalog registry with all catalogs registered
    pub fn new(storage: Arc<StorageManager>) -> Self {
        let mut registry = Self {
            catalogs: HashMap::new(),
            storage,
        };

        // Register all catalogs at initialization
        providers::register_all_catalogs(&mut registry);
        registry
    }

    /// Register a new catalog provider
    ///
    /// Adds a catalog provider to the registry and initializes it with the storage manager.
    /// If initialization fails, the catalog is still registered but may not function properly.
    ///
    /// # Arguments
    /// * `name` - Unique name for the catalog
    /// * `catalog` - Boxed catalog provider implementation
    pub fn register(&mut self, name: &str, mut catalog: Box<dyn CatalogProvider>) {
        // Initialize the catalog with storage reference
        if let Err(e) = catalog.init(self.storage.clone()) {
            log::warn!("Failed to initialize catalog '{}': {}", name, e);
        }

        self.catalogs.insert(name.to_string(), catalog);
        log::info!("Registered catalog provider: {}", name);
    }

    /// Get immutable reference to a catalog provider
    ///
    /// # Arguments
    /// * `name` - Name of the catalog to retrieve
    ///
    /// # Returns
    /// * `Some(&Box<dyn CatalogProvider>)` if catalog exists
    /// * `None` if catalog not found
    pub fn get(&self, name: &str) -> Option<&Box<dyn CatalogProvider>> {
        self.catalogs.get(name)
    }

    /// Get mutable reference to a catalog provider
    ///
    /// # Arguments
    /// * `name` - Name of the catalog to retrieve
    ///
    /// # Returns
    /// * `Some(&mut Box<dyn CatalogProvider>)` if catalog exists
    /// * `None` if catalog not found
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Box<dyn CatalogProvider>> {
        self.catalogs.get_mut(name)
    }

    /// List all registered catalog names
    ///
    /// # Returns
    /// * `Vec<String>` - List of all registered catalog names
    pub fn list_catalog_names(&self) -> Vec<String> {
        self.catalogs.keys().cloned().collect()
    }

    /// Check if a catalog is registered
    ///
    /// # Arguments
    /// * `name` - Name of the catalog to check
    ///
    /// # Returns
    /// * `true` if catalog is registered, `false` otherwise
    pub fn has_catalog(&self, name: &str) -> bool {
        self.catalogs.contains_key(name)
    }

    /// Get the number of registered catalogs
    ///
    /// # Returns
    /// * `usize` - Number of registered catalogs
    pub fn catalog_count(&self) -> usize {
        self.catalogs.len()
    }

    /// Save a specific catalog to storage
    ///
    /// Saves the state of a single catalog to storage using the catalog's save method.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to save
    ///
    /// # Returns
    /// * `Ok(())` if catalog saved successfully
    /// * `Err(CatalogError::CatalogNotFound)` if catalog doesn't exist
    /// * `Err(CatalogError)` if save operation fails
    pub fn save_catalog(&self, catalog_name: &str) -> CatalogResult<()> {
        let catalog = self
            .catalogs
            .get(catalog_name)
            .ok_or_else(|| CatalogError::CatalogNotFound(catalog_name.to_string()))?;

        let data = catalog.save().map_err(|e| {
            CatalogError::OperationFailed(format!(
                "Failed to save catalog '{}': {}",
                catalog_name, e
            ))
        })?;

        // Save catalog provider data to storage
        self.storage
            .save_catalog_provider(catalog_name, &data)
            .map_err(|e| {
                CatalogError::OperationFailed(format!(
                    "Failed to persist catalog '{}': {}",
                    catalog_name, e
                ))
            })?;

        log::debug!("Saved catalog '{}': {} bytes", catalog_name, data.len());
        Ok(())
    }

    /// Save all catalogs to storage
    ///
    /// Iterates through all registered catalogs and saves their state to storage
    /// using the catalog's save method.
    ///
    /// # Returns
    /// * `Ok(())` if all catalogs saved successfully
    /// * `Err(CatalogError)` if any catalog failed to save
    pub fn save_all(&self) -> CatalogResult<()> {
        for (name, catalog) in &self.catalogs {
            let data = catalog.save().map_err(|e| {
                CatalogError::OperationFailed(format!("Failed to save catalog '{}': {}", name, e))
            })?;

            // Save catalog provider data to storage
            self.storage
                .save_catalog_provider(name, &data)
                .map_err(|e| {
                    CatalogError::OperationFailed(format!(
                        "Failed to persist catalog '{}': {}",
                        name, e
                    ))
                })?;

            log::debug!("Saved catalog '{}': {} bytes", name, data.len());
        }
        Ok(())
    }

    /// Load all catalogs from storage
    ///
    /// Iterates through all registered catalogs and loads their state from storage
    /// using the catalog's load method.
    ///
    /// # Returns
    /// * `Ok(())` if all catalogs loaded successfully
    /// * `Err(CatalogError)` if any catalog failed to load
    pub fn load_all(&mut self) -> CatalogResult<()> {
        for (name, catalog) in &mut self.catalogs {
            log::debug!("Loading catalog '{}'", name);

            // Load catalog provider data from storage if it exists
            if let Some(data) = self.storage.load_catalog_provider(name).map_err(|e| {
                CatalogError::OperationFailed(format!("Failed to load catalog '{}': {}", name, e))
            })? {
                catalog.load(&data).map_err(|e| {
                    CatalogError::OperationFailed(format!(
                        "Failed to deserialize catalog '{}': {}",
                        name, e
                    ))
                })?;

                log::debug!("Loaded catalog '{}': {} bytes", name, data.len());
            } else {
                log::debug!(
                    "No stored data found for catalog '{}', using empty state",
                    name
                );
            }
        }
        Ok(())
    }

    /// Get reference to the storage manager
    ///
    /// # Returns
    /// * `Arc<StorageManager>` - Reference to the storage manager
    #[allow(dead_code)] // ROADMAP v0.4.0 - Catalog provider storage accessor (see ROADMAP.md §4)
    pub fn storage(&self) -> Arc<StorageManager> {
        self.storage.clone()
    }
}
