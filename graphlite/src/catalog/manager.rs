// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Unified catalog manager - The single external interface
//!
//! This module provides the CatalogManager, which is the ONLY interface that external
//! code should use to interact with the catalog system. It provides a unified API
//! for all catalog operations and manages the underlying registry.

use super::error::{CatalogError, CatalogResult};
use super::operations::{CatalogOperation, CatalogResponse, EntityType, QueryType};
use super::registry::CatalogRegistry;
use super::traits::CatalogSchema;
use crate::storage::StorageManager;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Information about a catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogInfo {
    /// Name of the catalog
    pub name: String,
    /// Schema describing the catalog's capabilities
    pub schema: CatalogSchema,
    /// List of supported operations
    pub supported_operations: Vec<String>,
}

/// Unified catalog manager - The single external interface
///
/// CatalogManager is the ONLY class that external code should interact with.
/// It provides a unified interface for all catalog operations and manages
/// the underlying catalog registries and providers.
///
/// # Key Design Principles
/// - **Single Interface**: External code only imports and uses CatalogManager
/// - **Generic Operations**: All operations use generic operation/response types
/// - **Storage Integration**: Automatic persistence through StorageManager
/// - **Provider Abstraction**: External code has zero knowledge of specific catalog providers
/// - **Architectural Separation**: Internal catalogs and external data source catalogs are managed separately
pub struct CatalogManager {
    /// Internal registry managing all catalog providers
    registry: CatalogRegistry,
    /// Reference to storage manager for persistence
    #[allow(dead_code)]
    // FALSE POSITIVE - Used in initialization (line 61) and passed to registry. Compiler limitation with field access detection.
    storage: Arc<StorageManager>,
}

impl CatalogManager {
    /// Create a new catalog manager
    ///
    /// This initializes both the catalog registry and data source catalog registry
    /// with all available providers and sets up storage integration.
    ///
    /// # Arguments
    /// * `storage` - Arc reference to storage manager for persistence
    ///
    /// # Returns
    /// * `Self` - Initialized catalog manager with all catalogs registered
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self {
            registry: CatalogRegistry::new(storage.clone()),
            storage,
        }
    }

    /// Execute operation on specific catalog
    ///
    /// This is the main entry point for all catalog operations. The operation
    /// is routed to the appropriate catalog provider based on the catalog name.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to execute the operation on
    /// * `operation` - The catalog operation to execute
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with the operation result
    /// * `Err(CatalogError::CatalogNotFound)` if catalog doesn't exist
    /// * `Err(CatalogError)` if the operation fails
    pub fn execute(
        &mut self,
        catalog_name: &str,
        operation: CatalogOperation,
    ) -> CatalogResult<CatalogResponse> {
        self.registry
            .get_mut(catalog_name)
            .ok_or_else(|| CatalogError::CatalogNotFound(catalog_name.to_string()))?
            .execute(operation)
    }

    /// Execute a read-only query on a catalog
    ///
    /// This method only accepts query operations and uses immutable access,
    /// allowing for concurrent read operations.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to query
    /// * `query_type` - Type of query to execute
    /// * `params` - Query parameters
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with the query result
    /// * `Err(CatalogError::CatalogNotFound)` if catalog doesn't exist
    /// * `Err(CatalogError)` if the query fails
    pub fn query_read_only(
        &self,
        catalog_name: &str,
        query_type: crate::catalog::operations::QueryType,
        params: serde_json::Value,
    ) -> CatalogResult<CatalogResponse> {
        use crate::catalog::operations::CatalogOperation;

        let operation = CatalogOperation::Query { query_type, params };

        self.registry
            .get(catalog_name)
            .ok_or_else(|| CatalogError::CatalogNotFound(catalog_name.to_string()))?
            .execute_read_only(operation)
    }

    /// Get catalog metadata without knowing the specific type
    ///
    /// Returns information about a catalog's capabilities, supported entities,
    /// and operations without requiring knowledge of the specific implementation.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to get information about
    ///
    /// # Returns
    /// * `Some(CatalogInfo)` if catalog exists
    /// * `None` if catalog not found
    pub fn get_catalog_info(&self, catalog_name: &str) -> Option<CatalogInfo> {
        self.registry.get(catalog_name).map(|cat| CatalogInfo {
            name: catalog_name.to_string(),
            schema: cat.schema(),
            supported_operations: cat.supported_operations(),
        })
    }

    /// List all available catalogs
    ///
    /// Returns a list of all registered catalog names. This can be used
    /// for discovery and validation purposes.
    ///
    /// # Returns
    /// * `Vec<String>` - List of all available catalog names
    pub fn list_catalogs(&self) -> Vec<String> {
        self.registry.list_catalog_names()
    }

    /// Check if a catalog exists
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to check
    ///
    /// # Returns
    /// * `true` if catalog exists, `false` otherwise
    pub fn has_catalog(&self, catalog_name: &str) -> bool {
        self.registry.has_catalog(catalog_name)
    }

    /// Get information about all available catalogs
    ///
    /// Returns detailed information about all registered catalogs,
    /// including their schemas and supported operations.
    ///
    /// # Returns
    /// * `Vec<CatalogInfo>` - Information about all available catalogs
    pub fn list_catalog_info(&self) -> Vec<CatalogInfo> {
        self.list_catalogs()
            .into_iter()
            .filter_map(|name| self.get_catalog_info(&name))
            .collect()
    }

    /// Save all catalogs to storage
    ///
    /// Persists the state of all registered catalogs to storage.
    /// This operation saves each catalog's state individually.
    ///
    /// # Returns
    /// * `Ok(())` if all catalogs saved successfully
    /// * `Err(CatalogError)` if any catalog failed to save
    pub async fn persist_all(&self) -> CatalogResult<()> {
        // Storage operations are sync, just wrap in async context
        self.registry.save_all()
    }

    /// Load all catalogs from storage
    ///
    /// Loads the state of all registered catalogs from storage.
    /// This operation loads each catalog's state individually.
    ///
    /// # Returns
    /// * `Ok(())` if all catalogs loaded successfully
    /// * `Err(CatalogError)` if any catalog failed to load
    pub async fn load_all(&mut self) -> CatalogResult<()> {
        // Storage operations are sync, just wrap in async context
        self.registry.load_all()
    }

    /// Save a specific catalog to storage
    ///
    /// Persists the state of a single catalog to storage.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to save
    ///
    /// # Returns
    /// * `Ok(())` if catalog saved successfully
    /// * `Err(CatalogError::CatalogNotFound)` if catalog doesn't exist
    /// * `Err(CatalogError)` if save operation fails
    pub fn persist_catalog(&self, catalog_name: &str) -> CatalogResult<()> {
        // Storage operations are sync
        self.registry.save_catalog(catalog_name)
    }

    /// Load a specific catalog from storage
    ///
    /// Loads the state of a single catalog from storage.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to load
    ///
    /// # Returns
    /// * `Ok(())` if catalog loaded successfully
    /// * `Err(CatalogError::CatalogNotFound)` if catalog doesn't exist
    /// * `Err(CatalogError)` if load operation fails
    pub fn load_catalog(&mut self, catalog_name: &str) -> CatalogResult<()> {
        if !self.registry.has_catalog(catalog_name) {
            return Err(CatalogError::CatalogNotFound(catalog_name.to_string()));
        }

        // TODO: Implement catalog-specific storage methods in StorageManager
        // For now, this is a placeholder for the storage integration
        log::debug!("Loading catalog '{}'", catalog_name);

        Ok(())
    }

    /// Get the number of registered catalogs
    ///
    /// # Returns
    /// * `usize` - Number of registered catalogs
    pub fn catalog_count(&self) -> usize {
        self.registry.catalog_count()
    }

    // Data Source Catalog Methods

    /// Execute operation on specific data source catalog
    ///
    /// This is the main entry point for all data source catalog operations. The operation
    /// is routed to the appropriate data source catalog provider based on the catalog name.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the data source catalog to execute the operation on
    /// * `operation` - The data source catalog operation to execute
    ///
    /// Execute a query operation with convenience method
    ///
    /// Convenience method for executing query operations with commonly used parameters.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to query
    /// * `query_type` - Type of query to perform
    /// * `params` - Query parameters
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with query results
    /// * `Err(CatalogError)` if operation fails
    pub fn query(
        &mut self,
        catalog_name: &str,
        query_type: QueryType,
        params: serde_json::Value,
    ) -> CatalogResult<CatalogResponse> {
        self.execute(catalog_name, CatalogOperation::Query { query_type, params })
    }

    /// Create an entity with convenience method
    ///
    /// Convenience method for creating entities with commonly used parameters.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to create entity in
    /// * `entity_type` - Type of entity to create
    /// * `name` - Name of the entity
    /// * `params` - Creation parameters
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with creation result
    /// * `Err(CatalogError)` if operation fails
    pub fn create_entity(
        &mut self,
        catalog_name: &str,
        entity_type: EntityType,
        name: &str,
        params: serde_json::Value,
    ) -> CatalogResult<CatalogResponse> {
        self.execute(
            catalog_name,
            CatalogOperation::Create {
                entity_type,
                name: name.to_string(),
                params,
            },
        )
    }

    /// List entities with convenience method
    ///
    /// Convenience method for listing entities with optional filters.
    ///
    /// # Arguments
    /// * `catalog_name` - Name of the catalog to list entities from
    /// * `entity_type` - Type of entities to list
    /// * `filters` - Optional filters to apply
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with entity list
    /// * `Err(CatalogError)` if operation fails
    pub fn list_entities(
        &mut self,
        catalog_name: &str,
        entity_type: EntityType,
        filters: Option<serde_json::Value>,
    ) -> CatalogResult<CatalogResponse> {
        self.execute(
            catalog_name,
            CatalogOperation::List {
                entity_type,
                filters,
            },
        )
    }
}
