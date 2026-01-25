// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Storage driver factory
//!
//! This module provides factory functions for creating storage drivers based on configuration.
//! It handles the instantiation and setup of different storage driver types.

use super::traits::{StorageDriver, StorageTree};
use super::types::{StorageResult, StorageType};
use std::path::Path;

/// Factory function to create a storage driver based on configuration
///
/// This is the main entry point for creating storage drivers. It takes a storage type
/// and path, then returns the appropriate driver implementation as a trait object.
///
/// # Arguments
/// * `storage_type` - The type of storage driver to create (Sled or Memory)
/// * `path` - The filesystem path where the database should be stored
///
/// # Returns
/// A boxed trait object that implements StorageDriver
///
/// # Examples
/// ```ignore
/// use crate::storage::drivers::{create_storage_driver, StorageType};
///
/// let driver = create_storage_driver(StorageType::Sled, "./data")?;
/// let tree = driver.open_tree("my_tree")?;
/// ```
pub fn create_storage_driver<P: AsRef<Path>>(
    storage_type: StorageType,
    path: P,
) -> StorageResult<Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>> {
    match storage_type {
        StorageType::Sled => {
            use crate::storage::persistent::sled::SledDriver;
            let driver = SledDriver::open(path)?;
            Ok(Box::new(driver) as Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>)
        }
        StorageType::Memory => {
            use crate::storage::persistent::memory::MemoryStorageDriver;
            let driver = MemoryStorageDriver::open(path)?;
            Ok(Box::new(driver) as Box<dyn StorageDriver<Tree = Box<dyn StorageTree>>>)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_sled_driver() {
        let temp_dir = TempDir::new().unwrap();
        let driver = create_storage_driver(StorageType::Sled, temp_dir.path()).unwrap();
        assert_eq!(driver.storage_type(), StorageType::Sled);
    }
}
