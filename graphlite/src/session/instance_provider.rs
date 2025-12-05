// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Instance-based session provider for embedded mode
//!
//! This provider maintains sessions in instance-owned storage,
//! allowing multiple independent database instances in one process.

use crate::catalog::manager::CatalogManager;
use crate::session::manager::SessionManager;
use crate::session::models::{SessionPermissionCache, UserSession};
use crate::session::provider::SessionProvider;
use crate::storage::StorageManager;
use crate::txn::TransactionManager;
use std::any::Any;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Instance-based session provider
///
/// Each QueryCoordinator instance owns its own session storage.
/// This enables multiple independent database instances in one process.
pub struct InstanceSessionProvider {
    /// The underlying session manager (instance-owned)
    manager: Arc<SessionManager>,
}

impl InstanceSessionProvider {
    /// Create a new instance session provider
    ///
    /// # Arguments
    /// * `transaction_manager` - Transaction manager for this instance
    /// * `storage_manager` - Storage manager for this instance
    /// * `catalog_manager` - Catalog manager for this instance
    pub fn new(
        transaction_manager: Arc<TransactionManager>,
        storage_manager: Arc<StorageManager>,
        catalog_manager: Arc<RwLock<CatalogManager>>,
    ) -> Self {
        let manager = Arc::new(SessionManager::new(
            transaction_manager,
            storage_manager,
            catalog_manager,
        ));

        Self { manager }
    }

    /// Get the underlying session manager (for compatibility)
    pub fn manager(&self) -> Arc<SessionManager> {
        self.manager.clone()
    }
}

impl SessionProvider for InstanceSessionProvider {
    fn create_session(
        &self,
        username: String,
        roles: Vec<String>,
        permissions: SessionPermissionCache,
    ) -> Result<String, String> {
        self.manager.create_session(username, roles, permissions)
    }

    fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<UserSession>>> {
        self.manager.get_session(session_id)
    }

    fn remove_session(&self, session_id: &str) -> Result<(), String> {
        self.manager.remove_session(session_id)
    }

    fn list_sessions(&self) -> Vec<String> {
        self.manager.get_active_session_ids()
    }

    fn cleanup_expired(&self, _max_idle: Duration) -> usize {
        // Use the manager's existing cleanup logic
        self.manager.cleanup_expired_sessions().unwrap_or(0)
    }

    fn shutdown(&self) -> Result<(), String> {
        self.manager.shutdown()
    }

    fn session_count(&self) -> usize {
        self.manager.session_count()
    }

    fn invalidate_sessions_for_graph(&self, graph_name: &str) -> usize {
        self.manager.invalidate_sessions_for_graph(graph_name)
    }

    fn get_storage_manager(&self) -> Arc<StorageManager> {
        self.manager.get_storage_manager()
    }

    fn get_catalog_manager(&self) -> Arc<RwLock<CatalogManager>> {
        self.manager.get_catalog_manager()
    }

    fn get_transaction_manager(&self) -> Arc<TransactionManager> {
        self.manager.get_transaction_manager()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::manager::CatalogManager;
    use crate::storage::{StorageManager, StorageMethod, StorageType};
    use crate::txn::TransactionManager;
    use std::sync::{Arc, RwLock};
    use tempfile::tempdir;

    #[test]
    fn test_instance_provider_create_session() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let storage = Arc::new(
            StorageManager::new(db_path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));
        let txn_mgr = Arc::new(TransactionManager::new(db_path.clone()).unwrap());

        let provider = InstanceSessionProvider::new(txn_mgr, storage, catalog);

        let session_id = provider
            .create_session(
                "test_user".to_string(),
                vec!["admin".to_string()],
                SessionPermissionCache::new(),
            )
            .unwrap();

        assert!(!session_id.is_empty());
        assert!(provider.get_session(&session_id).is_some());
        assert_eq!(provider.session_count(), 1);
    }

    #[test]
    fn test_instance_provider_remove_session() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let storage = Arc::new(
            StorageManager::new(db_path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));
        let txn_mgr = Arc::new(TransactionManager::new(db_path.clone()).unwrap());

        let provider = InstanceSessionProvider::new(txn_mgr, storage, catalog);

        let session_id = provider
            .create_session(
                "test_user".to_string(),
                vec!["admin".to_string()],
                SessionPermissionCache::new(),
            )
            .unwrap();

        assert_eq!(provider.session_count(), 1);

        provider.remove_session(&session_id).unwrap();

        assert_eq!(provider.session_count(), 0);
        assert!(provider.get_session(&session_id).is_none());
    }

    #[test]
    fn test_multiple_instance_providers_isolated() {
        let temp_dir1 = tempdir().unwrap();
        let temp_dir2 = tempdir().unwrap();
        let db_path1 = temp_dir1.path().join("test_db1");
        let db_path2 = temp_dir2.path().join("test_db2");

        // Create two independent providers
        let storage1 = Arc::new(
            StorageManager::new(db_path1.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog1 = Arc::new(RwLock::new(CatalogManager::new(storage1.clone())));
        let txn_mgr1 = Arc::new(TransactionManager::new(db_path1.clone()).unwrap());
        let provider1 = InstanceSessionProvider::new(txn_mgr1, storage1, catalog1);

        let storage2 = Arc::new(
            StorageManager::new(db_path2.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog2 = Arc::new(RwLock::new(CatalogManager::new(storage2.clone())));
        let txn_mgr2 = Arc::new(TransactionManager::new(db_path2.clone()).unwrap());
        let provider2 = InstanceSessionProvider::new(txn_mgr2, storage2, catalog2);

        // Create sessions in each provider
        let session1 = provider1
            .create_session(
                "user1".to_string(),
                vec!["admin".to_string()],
                SessionPermissionCache::new(),
            )
            .unwrap();

        let session2 = provider2
            .create_session(
                "user2".to_string(),
                vec!["admin".to_string()],
                SessionPermissionCache::new(),
            )
            .unwrap();

        // Verify isolation - each provider only knows about its own session
        assert!(provider1.get_session(&session1).is_some());
        assert!(provider1.get_session(&session2).is_none()); // Isolated!

        assert!(provider2.get_session(&session2).is_some());
        assert!(provider2.get_session(&session1).is_none()); // Isolated!

        assert_eq!(provider1.session_count(), 1);
        assert_eq!(provider2.session_count(), 1);
    }
}
