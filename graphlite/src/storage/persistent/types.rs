// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Storage driver types and error handling
//!
//! This module defines the types, enums, and error handling used throughout
//! the storage driver system.

use serde::{Serialize, Deserialize};
use std::fmt::Debug;

/// Storage driver type configuration
/// 
/// Specifies which underlying storage technology to use.
/// Each type has different performance characteristics and use cases.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum StorageType {
    /// RocksDB - High-performance persistent key-value store
    /// Best for: High write throughput, large datasets, production use
    RocksDB,
    
    /// Sled - Pure Rust embedded database
    /// Best for: Development, testing, pure Rust environments
    Sled,
    
    /// Memory - In-memory storage for testing
    /// Best for: Unit testing, development
    Memory,
}

impl Default for StorageType {
    fn default() -> Self {
        // Sled is default for development convenience
        StorageType::Sled
    }
}

impl std::str::FromStr for StorageType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rocksdb" => Ok(StorageType::RocksDB),
            "sled" => Ok(StorageType::Sled),
            "memory" => Ok(StorageType::Memory),
            _ => Err(format!("Unknown storage type: {}. Valid options: rocksdb, sled, memory", s))
        }
    }
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            StorageType::RocksDB => "rocksdb",
            StorageType::Sled => "sled",
            StorageType::Memory => "memory",
        };
        write!(f, "{}", name)
    }
}

/// Error type for storage driver operations
/// 
/// Comprehensive error type covering all possible failure modes in storage operations.
/// Designed to be easily converted from underlying storage engine errors.
#[derive(Debug)]
pub enum StorageDriverError {
    /// I/O related errors (file system, network, etc.)
    IoError(std::io::Error),
    
    /// Data serialization failed
    SerializationError(String),
    
    /// Data deserialization failed  
    _DeserializationError(String),
    
    /// Requested key was not found
    _NotFound(String),
    
    /// Invalid key format or content
    _InvalidKey(String),
    
    /// Driver-specific error (RocksDB, Sled, etc.)
    BackendSpecific(String),
}

impl std::fmt::Display for StorageDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageDriverError::IoError(e) => write!(f, "I/O error: {}", e),
            StorageDriverError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            StorageDriverError::_DeserializationError(e) => write!(f, "Deserialization error: {}", e),
            StorageDriverError::_NotFound(key) => write!(f, "Key not found: {}", key),
            StorageDriverError::_InvalidKey(key) => write!(f, "Invalid key: {}", key),
            StorageDriverError::BackendSpecific(e) => write!(f, "Storage driver error: {}", e),
        }
    }
}

impl std::error::Error for StorageDriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorageDriverError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

// Automatic conversions from common error types
impl From<std::io::Error> for StorageDriverError {
    fn from(e: std::io::Error) -> Self {
        StorageDriverError::IoError(e)
    }
}

impl From<bincode::Error> for StorageDriverError {
    fn from(e: bincode::Error) -> Self {
        StorageDriverError::SerializationError(e.to_string())
    }
}

impl From<serde_json::Error> for StorageDriverError {
    fn from(e: serde_json::Error) -> Self {
        StorageDriverError::SerializationError(e.to_string())
    }
}

/// Result type for storage driver operations
/// 
/// Standard Result type used throughout the storage driver system.
/// Provides consistent error handling across all drivers.
pub type StorageResult<T> = Result<T, StorageDriverError>;
