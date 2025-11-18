// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Sled storage driver implementation

use super::traits::{StorageDriver, StorageTree, IndexTreeOptions, TreeStatistics};
use super::types::{StorageType, StorageDriverError, StorageResult};
use std::path::Path;

/// Sled driver implementation
pub struct SledDriver {
    db: sled::Db,
}

/// Sled tree wrapper that implements StorageTree trait
pub struct SledTree {
    tree: sled::Tree,
}

impl StorageTree for SledTree {
    fn insert(&self, key: &[u8], value: &[u8]) -> StorageResult<()> {
        self.tree.insert(key, value)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn get(&self, key: &[u8]) -> StorageResult<Option<Vec<u8>>> {
        self.tree.get(key)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
            .map(|opt| opt.map(|v| v.to_vec()))
    }
    
    fn remove(&self, key: &[u8]) -> StorageResult<()> {
        self.tree.remove(key)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn contains_key(&self, key: &[u8]) -> StorageResult<bool> {
        self.tree.contains_key(key)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
    }
    
    fn clear(&self) -> StorageResult<()> {
        self.tree.clear()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
    }
    
    fn is_empty(&self) -> StorageResult<bool> {
        Ok(self.tree.is_empty())
    }
    
    fn iter(&self) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        let iter = self.tree.iter().map(|result| {
            result
                .map(|(k, v)| (k.to_vec(), v.to_vec()))
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
        });
        Ok(Box::new(iter))
    }
    
    fn flush(&self) -> StorageResult<()> {
        self.tree.flush()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn scan_prefix(&self, prefix: &[u8]) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        let iter = self.tree.scan_prefix(prefix).map(|result| {
            result
                .map(|(k, v)| (k.to_vec(), v.to_vec()))
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
        });
        Ok(Box::new(iter))
    }
    
    fn batch_get(&self, keys: &[&[u8]]) -> StorageResult<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let value = self.get(key)?;
            results.push(value);
        }
        Ok(results)
    }
    
    fn batch_insert(&self, entries: &[(&[u8], &[u8])]) -> StorageResult<()> {
        for (key, value) in entries {
            self.insert(key, value)?;
        }
        Ok(())
    }
    
    fn batch_remove(&self, keys: &[&[u8]]) -> StorageResult<()> {
        for key in keys {
            self.remove(key)?;
        }
        Ok(())
    }
}


impl StorageDriver for SledDriver {
    type Tree = Box<dyn StorageTree>;
    
    fn open<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
        let db = sled::open(path)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(SledDriver { db })
    }
    
    fn open_tree(&self, name: &str) -> StorageResult<Self::Tree> {
        let tree = self.db.open_tree(name)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(Box::new(SledTree { tree }) as Box<dyn StorageTree>)
    }
    
    fn list_trees(&self) -> StorageResult<Vec<String>> {
        let tree_names = self.db.tree_names()
            .into_iter()
            .map(|name| String::from_utf8_lossy(&name).to_string())
            .collect();
        Ok(tree_names)
    }
    
    fn flush(&self) -> StorageResult<()> {
        self.db.flush()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn storage_type(&self) -> StorageType {
        StorageType::Sled
    }
    
    fn shutdown(&mut self) -> StorageResult<()> {
        // Just flush to ensure data is persisted
        // The database will remain open but file locks should be reduced
        self.db.flush()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn open_index_tree(&self, name: &str, _index_options: IndexTreeOptions) -> StorageResult<Self::Tree> {
        // For now, just use regular tree - could optimize later based on index_options
        self.open_tree(name)
    }
    
    fn list_indexes(&self) -> StorageResult<Vec<String>> {
        // Return all trees for now - could filter by naming convention later
        self.list_trees()
    }
    
    fn drop_index(&self, name: &str) -> StorageResult<()> {
        self.db.drop_tree(name.as_bytes())
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        Ok(())
    }
    
    fn tree_stats(&self, _name: &str) -> StorageResult<Option<TreeStatistics>> {
        // Sled doesn't provide detailed statistics, return None for now
        Ok(None)
    }
}
