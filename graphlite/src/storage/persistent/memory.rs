// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! In-memory storage driver implementation for testing

use super::traits::{IndexTreeOptions, StorageDriver, StorageTree, TreeStatistics};
use super::types::{StorageResult, StorageType};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// In-memory storage driver for testing
pub struct MemoryStorageDriver {
    trees: Arc<RwLock<HashMap<String, Arc<MemoryTree>>>>,
}

/// In-memory tree implementation
pub struct MemoryTree {
    data: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl MemoryStorageDriver {
    /// Create a new memory storage driver
    pub fn new() -> Self {
        Self {
            trees: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl StorageTree for MemoryTree {
    fn insert(&self, key: &[u8], value: &[u8]) -> StorageResult<()> {
        self.data.write().insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> StorageResult<Option<Vec<u8>>> {
        Ok(self.data.read().get(key).cloned())
    }

    fn remove(&self, key: &[u8]) -> StorageResult<()> {
        self.data.write().remove(key);
        Ok(())
    }

    fn contains_key(&self, key: &[u8]) -> StorageResult<bool> {
        Ok(self.data.read().contains_key(key))
    }

    fn clear(&self) -> StorageResult<()> {
        self.data.write().clear();
        Ok(())
    }

    fn is_empty(&self) -> StorageResult<bool> {
        Ok(self.data.read().is_empty())
    }

    fn iter(
        &self,
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        let data = self.data.read();
        let items: Vec<_> = data
            .iter()
            .map(|(k, v)| Ok((k.clone(), v.clone())))
            .collect();
        Ok(Box::new(items.into_iter()))
    }

    fn scan_prefix(
        &self,
        prefix: &[u8],
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        let data = self.data.read();
        let items: Vec<_> = data
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| Ok((k.clone(), v.clone())))
            .collect();
        Ok(Box::new(items.into_iter()))
    }

    fn batch_get(&self, keys: &[&[u8]]) -> StorageResult<Vec<Option<Vec<u8>>>> {
        let data = self.data.read();
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            results.push(data.get(*key).cloned());
        }
        Ok(results)
    }

    fn batch_insert(&self, entries: &[(&[u8], &[u8])]) -> StorageResult<()> {
        let mut data = self.data.write();
        for (key, value) in entries {
            data.insert(key.to_vec(), value.to_vec());
        }
        Ok(())
    }

    fn batch_remove(&self, keys: &[&[u8]]) -> StorageResult<()> {
        let mut data = self.data.write();
        for key in keys {
            data.remove(*key);
        }
        Ok(())
    }

    fn flush(&self) -> StorageResult<()> {
        // No-op for memory storage
        Ok(())
    }
}

impl StorageDriver for MemoryStorageDriver {
    type Tree = Box<dyn StorageTree>;

    fn open<P: AsRef<Path>>(_path: P) -> StorageResult<Self> {
        Ok(Self::new())
    }

    fn open_tree(&self, name: &str) -> StorageResult<Self::Tree> {
        let mut trees = self.trees.write();

        if let Some(tree) = trees.get(name) {
            Ok(Box::new(MemoryTree {
                data: tree.data.clone(),
            }) as Box<dyn StorageTree>)
        } else {
            let tree = Arc::new(MemoryTree {
                data: Arc::new(RwLock::new(HashMap::new())),
            });
            trees.insert(name.to_string(), tree.clone());

            Ok(Box::new(MemoryTree {
                data: tree.data.clone(),
            }) as Box<dyn StorageTree>)
        }
    }

    fn list_trees(&self) -> StorageResult<Vec<String>> {
        Ok(self.trees.read().keys().cloned().collect())
    }

    fn flush(&self) -> StorageResult<()> {
        // No-op for memory storage
        Ok(())
    }

    fn storage_type(&self) -> StorageType {
        StorageType::Memory
    }

    fn open_index_tree(
        &self,
        name: &str,
        _index_options: IndexTreeOptions,
    ) -> StorageResult<Self::Tree> {
        // For memory storage, index options don't matter
        self.open_tree(name)
    }

    fn list_indexes(&self) -> StorageResult<Vec<String>> {
        self.list_trees()
    }

    fn drop_index(&self, name: &str) -> StorageResult<()> {
        self.trees.write().remove(name);
        Ok(())
    }

    fn tree_stats(&self, name: &str) -> StorageResult<Option<TreeStatistics>> {
        let trees = self.trees.read();
        if let Some(tree) = trees.get(name) {
            let data = tree.data.read();
            let entry_count = data.len() as u64;
            let size_bytes = data.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() as u64;

            Ok(Some(TreeStatistics {
                entry_count,
                size_bytes,
                memory_bytes: size_bytes, // Same as size for memory storage
                levels: None,
                compaction_stats: None,
            }))
        } else {
            Ok(None)
        }
    }

    fn shutdown(&mut self) -> StorageResult<()> {
        // No-op for memory storage
        Ok(())
    }
}
