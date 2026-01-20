// // Copyright (c) 2024-2025 DeepGraph Inc.
// // SPDX-License-Identifier: Apache-2.0
// //
// //! Test suite for REDB storage backend

// #[cfg(feature = "redb-backend")]
// mod redb_tests {
//     use graphlite::{StorageManager, StorageMethod};
//     use tempfile::TempDir;

//     // Note: StorageType is not publicly exported, so we test through StorageManager
//     // which uses the storage type internally based on configuration

//     #[test]
//     fn test_redb_through_storage_manager() {
//         let temp_dir = TempDir::new().unwrap();

//         // Create a storage manager - this will use the default storage type (Sled)
//         // To test REDB, we'd need to expose StorageType publicly or test at a lower level
//         let storage_manager = StorageManager::new(
//             temp_dir.path(),
//             StorageMethod::DiskOnly,
//             graphlite::StorageType::Redb, // This won't work as StorageType isn't public
//         );

//         // For now, this test documents that we need to expose StorageType
//         // or create a different testing approach
//     }
// }

// // Direct unit tests for REDB implementation
// // These tests access the module directly rather than through the public API
// #[cfg(all(test, feature = "redb-backend"))]
// mod redb_unit_tests {
//     use graphlite::storage::persistent::redb::{RedbDriver, RedbTree};
//     use graphlite::storage::persistent::traits::{StorageDriver, StorageTree, IndexTreeOptions};
//     use tempfile::TempDir;

//     #[test]
//     fn test_redb_basic_operations() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();

//         // Open a tree
//         let tree = driver.open_tree("test_tree").unwrap();

//         // Test insert and get
//         tree.insert(b"key1", b"value1").unwrap();
//         let value = tree.get(b"key1").unwrap();
//         assert_eq!(value, Some(b"value1".to_vec()));

//         // Test contains_key
//         assert!(tree.contains_key(b"key1").unwrap());
//         assert!(!tree.contains_key(b"key2").unwrap());

//         // Test remove
//         tree.remove(b"key1").unwrap();
//         assert!(!tree.contains_key(b"key1").unwrap());
//     }

//     #[test]
//     fn test_redb_batch_operations() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();
//         let tree = driver.open_tree("batch_test").unwrap();

//         // Test batch insert
//         let entries = vec![
//             (b"key1" as &[u8], b"value1" as &[u8]),
//             (b"key2" as &[u8], b"value2" as &[u8]),
//             (b"key3" as &[u8], b"value3" as &[u8]),
//         ];
//         tree.batch_insert(&entries).unwrap();

//         // Test batch get
//         let keys = vec![b"key1" as &[u8], b"key2" as &[u8], b"key3" as &[u8]];
//         let values = tree.batch_get(&keys).unwrap();
//         assert_eq!(values.len(), 3);
//         assert_eq!(values[0], Some(b"value1".to_vec()));
//         assert_eq!(values[1], Some(b"value2".to_vec()));
//         assert_eq!(values[2], Some(b"value3".to_vec()));

//         // Test batch remove
//         let remove_keys = vec![b"key1" as &[u8], b"key3" as &[u8]];
//         tree.batch_remove(&remove_keys).unwrap();
//         assert!(!tree.contains_key(b"key1").unwrap());
//         assert!(tree.contains_key(b"key2").unwrap());
//         assert!(!tree.contains_key(b"key3").unwrap());
//     }

//     #[test]
//     fn test_redb_iteration() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();
//         let tree = driver.open_tree("iter_test").unwrap();

//         // Insert test data
//         tree.insert(b"a", b"1").unwrap();
//         tree.insert(b"b", b"2").unwrap();
//         tree.insert(b"c", b"3").unwrap();

//         // Test iteration
//         let items: Vec<_> = tree.iter().unwrap().collect::<Result<Vec<_>, _>>().unwrap();
//         assert_eq!(items.len(), 3);
//     }

//     #[test]
//     fn test_redb_prefix_scan() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();
//         let tree = driver.open_tree("prefix_test").unwrap();

//         // Insert test data with common prefix
//         tree.insert(b"user:1", b"alice").unwrap();
//         tree.insert(b"user:2", b"bob").unwrap();
//         tree.insert(b"user:3", b"charlie").unwrap();
//         tree.insert(b"post:1", b"hello").unwrap();

//         // Test prefix scan
//         let items: Vec<_> = tree
//             .scan_prefix(b"user:")
//             .unwrap()
//             .collect::<Result<Vec<_>, _>>()
//             .unwrap();
//         assert_eq!(items.len(), 3);
//     }

//     #[test]
//     fn test_redb_clear() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();
//         let tree = driver.open_tree("clear_test").unwrap();

//         // Insert data
//         tree.insert(b"key1", b"value1").unwrap();
//         tree.insert(b"key2", b"value2").unwrap();
//         assert!(!tree.is_empty().unwrap());

//         // Clear tree
//         tree.clear().unwrap();
//         assert!(tree.is_empty().unwrap());
//     }

//     #[test]
//     fn test_redb_multiple_trees() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();

//         // Create multiple trees
//         let tree1 = driver.open_tree("tree1").unwrap();
//         let tree2 = driver.open_tree("tree2").unwrap();

//         // Insert different data in each tree
//         tree1.insert(b"key", b"value1").unwrap();
//         tree2.insert(b"key", b"value2").unwrap();

//         // Verify isolation
//         assert_eq!(tree1.get(b"key").unwrap(), Some(b"value1".to_vec()));
//         assert_eq!(tree2.get(b"key").unwrap(), Some(b"value2".to_vec()));

//         // List trees
//         let trees = driver.list_trees().unwrap();
//         assert!(trees.contains(&"tree1".to_string()));
//         assert!(trees.contains(&"tree2".to_string()));
//     }

//     #[test]
//     fn test_redb_persistence() {
//         let temp_dir = TempDir::new().unwrap();
//         let db_path = temp_dir.path();

//         // Create driver and insert data
//         {
//             let driver = RedbDriver::open(db_path).unwrap();
//             let tree = driver.open_tree("persist_test").unwrap();
//             tree.insert(b"persistent_key", b"persistent_value")
//                 .unwrap();
//             tree.flush().unwrap();
//         }

//         // Reopen and verify data persisted
//         {
//             let driver = RedbDriver::open(db_path).unwrap();
//             let tree = driver.open_tree("persist_test").unwrap();
//             let value = tree.get(b"persistent_key").unwrap();
//             assert_eq!(value, Some(b"persistent_value".to_vec()));
//         }
//     }

//     #[test]
//     fn test_redb_index_operations() {
//         let temp_dir = TempDir::new().unwrap();
//         let driver = RedbDriver::open(temp_dir.path()).unwrap();

//         // Create an index tree
//         let index_options = IndexTreeOptions::default();
//         let index_tree = driver.open_index_tree("test_index", index_options).unwrap();

//         // Test basic index operations
//         index_tree.insert(b"index_key", b"index_value").unwrap();
//         assert_eq!(
//             index_tree.get(b"index_key").unwrap(),
//             Some(b"index_value".to_vec())
//         );

//         // List indexes
//         let indexes = driver.list_indexes().unwrap();
//         assert!(indexes.contains(&"test_index".to_string()));

//         // Drop index
//         driver.drop_index("test_index").unwrap();
//         let indexes_after = driver.list_indexes().unwrap();
//         assert!(!indexes_after.contains(&"test_index".to_string()));
//     }
// }
