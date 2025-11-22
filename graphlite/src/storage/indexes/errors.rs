// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Error types for the indexing system

use crate::storage::persistent::types::StorageDriverError;
use crate::storage::StorageError;
use thiserror::Error;

/// Errors that can occur during index operations
#[derive(Error, Debug)]
pub enum IndexError {
    #[error("Index '{0}' already exists")]
    AlreadyExists(String),

    #[error("Index '{0}' not found")]
    NotFound(String),

    #[error("Invalid index configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] bincode::Error),

    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    #[error("Storage driver error: {0}")]
    StorageDriverError(#[from] StorageDriverError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Query error: {0}")]
    QueryError(String),

    #[error("Index is not ready for operations")]
    NotReady,

    #[error("Operation not supported by this index type")]
    UnsupportedOperation,

    #[error("Index maintenance failed: {0}")]
    MaintenanceError(String),

    #[error("Concurrent modification error")]
    ConcurrentModification,

    #[error("Index corruption detected: {0}")]
    CorruptionError(String),
}

impl IndexError {
    /// Create a query error
    pub fn query<S: Into<String>>(msg: S) -> Self {
        Self::QueryError(msg.into())
    }

    /// Create a configuration error
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::InvalidConfiguration(msg.into())
    }

    /// Create a maintenance error
    pub fn maintenance<S: Into<String>>(msg: S) -> Self {
        Self::MaintenanceError(msg.into())
    }

    /// Create an IO error
    pub fn io<S: Into<String>>(msg: S) -> Self {
        Self::IoError(std::io::Error::other(msg.into()))
    }

    /// Create a creation error (alias for config)
    pub fn creation<S: Into<String>>(msg: S) -> Self {
        Self::InvalidConfiguration(msg.into())
    }

    /// Create a serialization error
    pub fn serialization<S: Into<String>>(msg: S) -> Self {
        Self::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            msg.into(),
        ))
    }

    /// Create a deserialization error
    pub fn deserialization<S: Into<String>>(msg: S) -> Self {
        Self::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            msg.into(),
        ))
    }
}
