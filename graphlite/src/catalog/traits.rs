// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Catalog provider trait definition
//!
//! This module defines the core trait that all catalog providers must implement.
//! The trait provides a generic interface for catalog operations, storage integration,
//! and metadata access.

use std::sync::Arc;
use crate::storage::StorageManager;
use super::error::CatalogResult;
use super::operations::{CatalogOperation, CatalogResponse};
use serde::{Serialize, Deserialize};

/// Schema information for a catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSchema {
    /// Name of the catalog type
    pub name: String,
    /// Version of the catalog implementation
    pub version: String,
    /// List of entity types this catalog can manage
    pub entities: Vec<String>,
    /// List of supported operations
    pub operations: Vec<String>,
}

/// Core trait that all catalog providers must implement
///
/// This trait defines the generic interface for all catalog types in the system.
/// Implementing this trait allows a catalog to be automatically integrated into
/// the catalog registry and managed through the unified CatalogManager interface.
pub trait CatalogProvider: Send + Sync {
    /// Initialize the catalog with a storage manager reference
    ///
    /// This method is called once when the catalog is registered. The catalog
    /// should store the storage reference for persistence operations.
    ///
    /// # Arguments
    /// * `storage` - Arc reference to the storage manager for persistence
    ///
    /// # Returns
    /// * `Ok(())` on successful initialization
    /// * `Err(CatalogError)` if initialization fails
    fn init(&mut self, storage: Arc<StorageManager>) -> CatalogResult<()>;
    
    /// Handle catalog operations
    ///
    /// This is the main entry point for all catalog operations. The catalog
    /// should match on the operation type and execute the appropriate logic.
    ///
    /// # Arguments
    /// * `op` - The catalog operation to execute
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with the operation result
    /// * `Err(CatalogError)` if the operation fails
    fn execute(&mut self, op: CatalogOperation) -> CatalogResult<CatalogResponse>;

    /// Handle read-only catalog operations (queries)
    ///
    /// This method handles operations that don't modify catalog state, allowing
    /// for concurrent read access without requiring mutable references.
    ///
    /// # Arguments
    /// * `op` - The catalog operation to execute (should be read-only)
    ///
    /// # Returns
    /// * `Ok(CatalogResponse)` with the operation result
    /// * `Err(CatalogError)` if the operation fails or is not read-only
    fn execute_read_only(&self, op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        // Default implementation: For most catalogs, query operations can be safely
        // executed in read-only mode since they don't modify state. This is a
        // temporary bridge until all catalogs implement proper read-only methods.
        use super::error::CatalogError;
        match op {
            CatalogOperation::Query { .. } => {
                // For now, return a temporary error encouraging migration
                Err(CatalogError::NotSupported(
                    "Read-only queries not yet implemented for this catalog. \
                     Consider implementing execute_read_only() for better concurrency.".to_string()
                ))
            }
            _ => Err(CatalogError::NotSupported(
                "Only query operations are supported in read-only mode".to_string()
            ))
        }
    }
    
    /// Persist catalog state to storage
    ///
    /// Serialize the catalog's current state to bytes for persistence.
    /// This is called by the catalog manager during save operations.
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` with the serialized catalog state
    /// * `Err(CatalogError)` if serialization fails
    fn save(&self) -> CatalogResult<Vec<u8>>;
    
    /// Load catalog state from storage
    ///
    /// Deserialize catalog state from bytes loaded from storage.
    /// This is called by the catalog manager during load operations.
    ///
    /// # Arguments
    /// * `data` - Serialized catalog state as bytes
    ///
    /// # Returns
    /// * `Ok(())` on successful deserialization
    /// * `Err(CatalogError)` if deserialization fails
    fn load(&mut self, data: &[u8]) -> CatalogResult<()>;
    
    /// Get catalog schema and metadata
    ///
    /// Returns information about this catalog's capabilities, supported
    /// entities, and operations.
    ///
    /// # Returns
    /// * `CatalogSchema` describing this catalog's capabilities
    fn schema(&self) -> CatalogSchema;
    
    /// Get list of supported operations
    ///
    /// Returns a list of operation names that this catalog supports.
    /// This is used for capability discovery and validation.
    ///
    /// # Returns
    /// * `Vec<String>` of supported operation names
    fn supported_operations(&self) -> Vec<String>;
}
