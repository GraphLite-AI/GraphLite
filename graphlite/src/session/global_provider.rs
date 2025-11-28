// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Global session provider for server/daemon mode
//!
//! This provider implements a process-wide session pool that can be shared across
//! multiple QueryCoordinator instances. It's designed for server scenarios where
//! multiple clients connect to the same database and sessions need to be shared.
//!
//! # When to Use
//!
//! Use GlobalSessionProvider when:
//! - Running GraphLite as a server or daemon
//! - Multiple clients connecting to the same database instance
//! - Sessions need to be shared across different parts of the application
//! - Long-running service with persistent sessions
//!
//! # Example
//!
//! ```rust,ignore
//! use graphlite::session::GlobalSessionProvider;
//! use graphlite::coordinator::QueryCoordinator;
//!
//! // Create coordinator in Global mode
//! let coordinator = QueryCoordinator::from_path_with_mode(
//!     "server_db.db",
//!     SessionMode::Global
//! )?;
//!
//! // Multiple clients can share sessions through the global pool
//! let session1 = coordinator.create_session("user1", vec![], permissions)?;
//! let session2 = coordinator.create_session("user2", vec![], permissions)?;
//! ```

use crate::catalog::manager::CatalogManager;
use crate::session::manager::SessionManager;
use crate::session::models::{SessionPermissionCache, UserSession};
use crate::session::provider::SessionProvider;
use crate::storage::StorageManager;
use crate::txn::TransactionManager;
use once_cell::sync::Lazy;
use std::any::Any;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Process-wide global session manager
///
/// This is initialized once per process and shared across all GlobalSessionProvider instances.
/// Using Lazy ensures thread-safe initialization on first access.
static GLOBAL_SESSION_MANAGER: Lazy<RwLock<Option<Arc<SessionManager>>>> =
    Lazy::new(|| RwLock::new(None));

/// Global session provider for server/daemon mode
///
/// Implements a process-wide session pool that is shared across all
/// QueryCoordinator instances that use this provider. Unlike InstanceSessionProvider,
/// which gives each coordinator its own isolated session storage, GlobalSessionProvider
/// maintains a single shared session pool.
///
/// # Thread Safety
///
/// This provider is thread-safe and can be safely cloned and shared across threads.
/// The underlying session manager uses Arc and RwLock for synchronization.
///
/// # Important
///
/// All GlobalSessionProvider instances share the SAME session manager. The first
/// instance to be created initializes the global session manager, and all subsequent
/// instances use that same manager.
pub struct GlobalSessionProvider {
    // Note: We don't actually store anything here - all state is in the global manager
    _marker: std::marker::PhantomData<()>,
}

impl GlobalSessionProvider {
    /// Create a new global session provider
    ///
    /// # Arguments
    ///
    /// * `transaction_manager` - Transaction manager for session transaction state
    /// * `storage_manager` - Storage manager for data access
    /// * `catalog_manager` - Catalog manager for schema and metadata access
    ///
    /// # Returns
    ///
    /// A new GlobalSessionProvider instance that shares its session pool with all
    /// other GlobalSessionProvider instances.
    ///
    /// # Note
    ///
    /// Only the FIRST call to `new()` will actually initialize the global session manager
    /// with the provided parameters. Subsequent calls will use the existing global manager
    /// and ignore the parameters. This is intentional for server mode where you want ONE
    /// session pool for the entire process.
    pub fn new(
        transaction_manager: Arc<TransactionManager>,
        storage_manager: Arc<StorageManager>,
        catalog_manager: Arc<RwLock<CatalogManager>>,
    ) -> Self {
        // Initialize the global session manager if not already initialized
        let mut global_manager = GLOBAL_SESSION_MANAGER.write().unwrap();
        if global_manager.is_none() {
            let manager = Arc::new(SessionManager::new(
                transaction_manager,
                storage_manager,
                catalog_manager,
            ));
            *global_manager = Some(manager);
        }
        drop(global_manager);

        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the global session manager
    ///
    /// Returns None if the global manager hasn't been initialized yet.
    fn get_manager(&self) -> Option<Arc<SessionManager>> {
        GLOBAL_SESSION_MANAGER.read().unwrap().clone()
    }

    /// Get access to the underlying session manager
    ///
    /// This method provides backward compatibility and direct access to the
    /// session manager for advanced use cases.
    pub fn manager(&self) -> Arc<SessionManager> {
        self.get_manager()
            .expect("Global session manager not initialized")
    }
}

impl SessionProvider for GlobalSessionProvider {
    fn create_session(
        &self,
        username: String,
        roles: Vec<String>,
        permissions: SessionPermissionCache,
    ) -> Result<String, String> {
        self.get_manager()
            .ok_or_else(|| "Global session manager not initialized".to_string())?
            .create_session(username, roles, permissions)
    }

    fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<UserSession>>> {
        self.get_manager()?.get_session(session_id)
    }

    fn remove_session(&self, session_id: &str) -> Result<(), String> {
        self.get_manager()
            .ok_or_else(|| "Global session manager not initialized".to_string())?
            .remove_session(session_id)
    }

    fn list_sessions(&self) -> Vec<String> {
        self.get_manager()
            .map(|mgr| mgr.get_active_session_ids())
            .unwrap_or_default()
    }

    fn cleanup_expired(&self, _max_idle: Duration) -> usize {
        // SessionManager's cleanup_expired_sessions uses hardcoded 1 hour timeout
        // We ignore the max_idle parameter for now
        self.get_manager()
            .and_then(|mgr| mgr.cleanup_expired_sessions().ok())
            .unwrap_or(0)
    }

    fn shutdown(&self) -> Result<(), String> {
        self.get_manager()
            .ok_or_else(|| "Global session manager not initialized".to_string())?
            .shutdown()
    }

    fn session_count(&self) -> usize {
        self.get_manager()
            .map(|mgr| mgr.session_count())
            .unwrap_or(0)
    }

    fn invalidate_sessions_for_graph(&self, graph_name: &str) -> usize {
        self.get_manager()
            .map(|mgr| mgr.invalidate_sessions_for_graph(graph_name))
            .unwrap_or(0)
    }

    fn get_storage_manager(&self) -> Arc<StorageManager> {
        self.get_manager()
            .expect("Global session manager not initialized")
            .get_storage_manager()
    }

    fn get_catalog_manager(&self) -> Arc<RwLock<CatalogManager>> {
        self.get_manager()
            .expect("Global session manager not initialized")
            .get_catalog_manager()
    }

    fn get_transaction_manager(&self) -> Arc<TransactionManager> {
        self.get_manager()
            .expect("Global session manager not initialized")
            .get_transaction_manager()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// Make GlobalSessionProvider cloneable
// All clones share the same global session manager
impl Clone for GlobalSessionProvider {
    fn clone(&self) -> Self {
        Self {
            _marker: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{StorageManager, StorageMethod, StorageType};
    use std::sync::{Arc, RwLock};
    use tempfile::tempdir;

    /// Test helper to clear the global session manager between tests
    fn clear_global_manager() {
        let mut global_manager = GLOBAL_SESSION_MANAGER.write().unwrap();
        if let Some(manager) = global_manager.take() {
            let _ = manager.shutdown();
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_global_provider_session_sharing() {
        clear_global_manager();

        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // Create shared managers
        let storage = Arc::new(
            StorageManager::new(db_path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));
        let txn_mgr = Arc::new(TransactionManager::new(db_path.clone()).unwrap());

        // Create two global providers with same managers
        let provider1 = GlobalSessionProvider::new(txn_mgr.clone(), storage.clone(), catalog.clone());
        let provider2 = GlobalSessionProvider::new(txn_mgr.clone(), storage.clone(), catalog.clone());

        // Create session in provider1
        let permissions = SessionPermissionCache::default();
        let session_id = provider1
            .create_session("user1".to_string(), vec![], permissions)
            .unwrap();

        // CRITICAL: Provider2 should see the same session (shared pool)
        // This is the KEY difference from InstanceSessionProvider
        let session_from_provider2 = provider2.get_session(&session_id);
        assert!(
            session_from_provider2.is_some(),
            "Global providers should share sessions"
        );

        // Verify session data is the same
        if let Some(session_arc) = session_from_provider2 {
            let session = session_arc.read().unwrap();
            assert_eq!(session.username, "user1");
        }

        // Session count should be consistent across both providers
        assert_eq!(provider1.session_count(), 1);
        assert_eq!(provider2.session_count(), 1);

        // Remove session from provider2
        provider2.remove_session(&session_id).unwrap();

        // Provider1 should also not see the session anymore
        assert!(provider1.get_session(&session_id).is_none());
        assert_eq!(provider1.session_count(), 0);
    }

    #[test]
    #[serial_test::serial]
    fn test_global_provider_clone() {
        clear_global_manager();

        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let storage = Arc::new(
            StorageManager::new(db_path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));
        let txn_mgr = Arc::new(TransactionManager::new(db_path.clone()).unwrap());

        let provider1 = GlobalSessionProvider::new(txn_mgr, storage, catalog);
        let provider2 = provider1.clone();

        // Create session in provider1
        let permissions = SessionPermissionCache::default();
        let session_id = provider1
            .create_session("user1".to_string(), vec![], permissions)
            .unwrap();

        // Cloned provider should see the same session
        assert!(provider2.get_session(&session_id).is_some());
        assert_eq!(provider2.session_count(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn test_global_provider_manager_access() {
        clear_global_manager();

        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_db");

        let storage = Arc::new(
            StorageManager::new(db_path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .unwrap(),
        );
        let catalog = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));
        let txn_mgr = Arc::new(TransactionManager::new(db_path.clone()).unwrap());

        let provider = GlobalSessionProvider::new(txn_mgr, storage, catalog);

        // Should be able to access underlying manager
        let manager = provider.manager();
        assert_eq!(manager.session_count(), 0);
    }
}
