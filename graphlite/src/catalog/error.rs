// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Error types for the pluggable catalog system

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum CatalogError {
    #[error("Catalog not found: {0}")]
    CatalogNotFound(String),
    
    #[error("Catalog operation failed: {0}")]
    OperationFailed(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Entity not found: {0}")]
    EntityNotFound(String),
    
    #[error("Entity already exists: {0}")]
    EntityAlreadyExists(String),
    
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("IO error: {0}")]
    IoError(String),
    
    #[error("Not supported: {0}")]
    NotSupported(String),
    
    #[error("Duplicate entry: {0}")]
    DuplicateEntry(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

impl From<std::io::Error> for CatalogError {
    fn from(err: std::io::Error) -> Self {
        CatalogError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for CatalogError {
    fn from(err: serde_json::Error) -> Self {
        CatalogError::SerializationError(err.to_string())
    }
}

impl From<bincode::Error> for CatalogError {
    fn from(err: bincode::Error) -> Self {
        CatalogError::SerializationError(err.to_string())
    }
}

impl From<crate::storage::StorageError> for CatalogError {
    fn from(err: crate::storage::StorageError) -> Self {
        CatalogError::StorageError(err.to_string())
    }
}

pub type CatalogResult<T> = Result<T, CatalogError>;
