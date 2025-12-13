// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! REDB storage driver implementation

use super::traits::{IndexTreeOptions, StorageDriver, StorageTree, TreeStatistics};
use super::types::{StorageDriverError, StorageResult, StorageType};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition, TableHandle};
use std::path::Path;
use std::sync::Arc;

/// REDB driver implementation
pub struct RedbDriver {
    db: Arc<Database>,
}

/// REDB tree wrapper that implements StorageTree trait
/// In REDB, each "tree" is actually a separate table in the database
pub struct RedbTree {
    db: Arc<Database>,
    table_name: String,
}

impl StorageTree for RedbTree {
    fn insert(&self, key: &[u8], value: &[u8]) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            // Create a table definition for this specific table
            let table_def: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new(&self.table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

            table
                .insert(key, value)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }

    fn get(&self, key: &[u8]) -> StorageResult<Option<Vec<u8>>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let result = table
            .get(key)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(result.map(|guard| guard.value().to_vec()))
    }

    fn remove(&self, key: &[u8]) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new(&self.table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

            table
                .remove(key)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }

    fn contains_key(&self, key: &[u8]) -> StorageResult<bool> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let result = table
            .get(key)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(result.is_some())
    }

    fn clear(&self) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new(&self.table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

            // Collect all keys first to avoid borrowing issues
            let keys: Vec<Vec<u8>> = table
                .iter()
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?
                .filter_map(|result| result.ok())
                .map(|(k, _)| k.value().to_vec())
                .collect();

            // Remove all keys
            for key in keys {
                table
                    .remove(key.as_slice())
                    .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }

    fn is_empty(&self) -> StorageResult<bool> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(table.is_empty().map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?)
    }

    fn iter(
        &self,
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        // REDB's iterators are tied to transactions, which makes this tricky
        // We need to collect all data into memory for now
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let items: Vec<StorageResult<(Vec<u8>, Vec<u8>)>> = table
            .iter()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?
            .map(|result| {
                result
                    .map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
                    .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
            })
            .collect();

        Ok(Box::new(items.into_iter()))
    }

    fn flush(&self) -> StorageResult<()> {
        // REDB is crash-safe by default and handles durability automatically
        // We can force a checkpoint if needed
        // For now, this is a no-op as REDB handles persistence
        Ok(())
    }

    fn scan_prefix(
        &self,
        prefix: &[u8],
    ) -> StorageResult<Box<dyn Iterator<Item = StorageResult<(Vec<u8>, Vec<u8>)>> + '_>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        // Collect items that match the prefix
        let prefix_vec = prefix.to_vec();
        let items: Vec<StorageResult<(Vec<u8>, Vec<u8>)>> = table
            .iter()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?
            .map(|result| {
                result
                    .map(|(k, v)| (k.value().to_vec(), v.value().to_vec()))
                    .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))
            })
            .filter(|result| {
                if let Ok((key, _)) = result {
                    key.starts_with(&prefix_vec)
                } else {
                    true // Keep errors
                }
            })
            .collect();

        Ok(Box::new(items.into_iter()))
    }

    fn batch_get(&self, keys: &[&[u8]]) -> StorageResult<Vec<Option<Vec<u8>>>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(&self.table_name);
        let table = read_txn
            .open_table(table_def)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let result = table
                .get(*key)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
            results.push(result.map(|guard| guard.value().to_vec()));
        }
        Ok(results)
    }

    fn batch_insert(&self, entries: &[(&[u8], &[u8])]) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new(&self.table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

            for (key, value) in entries {
                table
                    .insert(*key, *value)
                    .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }

    fn batch_remove(&self, keys: &[&[u8]]) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new(&self.table_name);
            let mut table = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

            for key in keys {
                table
                    .remove(*key)
                    .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
            }
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }
}

impl StorageDriver for RedbDriver {
    type Tree = Box<dyn StorageTree>;

    fn open<P: AsRef<Path>>(path: P) -> StorageResult<Self> {
        // REDB requires a file path, not a directory
        // We'll create a database file in the specified directory
        let db_path = if path.as_ref().is_dir() {
            path.as_ref().join("graphlite.redb")
        } else {
            path.as_ref().to_path_buf()
        };

        let db = Database::create(&db_path)
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(RedbDriver { db: Arc::new(db) })
    }

    fn open_tree(&self, name: &str) -> StorageResult<Self::Tree> {
        // In REDB, we create a table for each "tree"
        // We need to ensure the table exists by opening it in a write transaction
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(name);
            let _ = write_txn
                .open_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(Box::new(RedbTree {
            db: self.db.clone(),
            table_name: name.to_string(),
        }) as Box<dyn StorageTree>)
    }

    fn list_trees(&self) -> StorageResult<Vec<String>> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        let table_names: Vec<String> = read_txn
            .list_tables()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?
            .map(|handle| handle.name().to_string())
            .collect();

        Ok(table_names)
    }

    fn flush(&self) -> StorageResult<()> {
        // REDB handles durability automatically
        // We can force a checkpoint if needed, but it's not strictly necessary
        Ok(())
    }

    fn storage_type(&self) -> StorageType {
        StorageType::Redb
    }

    fn shutdown(&mut self) -> StorageResult<()> {
        // REDB handles cleanup automatically on drop
        // Just ensure any pending operations are complete
        Ok(())
    }

    fn open_index_tree(
        &self,
        name: &str,
        _index_options: IndexTreeOptions,
    ) -> StorageResult<Self::Tree> {
        // For now, just use regular tree - could optimize later based on index_options
        self.open_tree(name)
    }

    fn list_indexes(&self) -> StorageResult<Vec<String>> {
        // Return all trees for now - could filter by naming convention later
        self.list_trees()
    }

    fn drop_index(&self, name: &str) -> StorageResult<()> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        {
            let table_def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(name);
            write_txn
                .delete_table(table_def)
                .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;
        }

        write_txn
            .commit()
            .map_err(|e| StorageDriverError::BackendSpecific(e.to_string()))?;

        Ok(())
    }

    fn tree_stats(&self, _name: &str) -> StorageResult<Option<TreeStatistics>> {
        // REDB provides some stats, but we'll return None for now
        // Could be enhanced later with REDB's stats API
        Ok(None)
    }
}
