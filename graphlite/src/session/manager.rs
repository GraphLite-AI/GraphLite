// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Global session management with persistent session registry
//!
//! This module provides centralized session management similar to Oracle/PostgreSQL
//! where sessions are looked up by ID from a global registry.

use crate::catalog::manager::CatalogManager;
use crate::session::models::{SessionPermissionCache, UserSession};
use crate::storage::StorageManager;
use crate::txn::TransactionManager;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Safe to use block_on here as they're not called from within async contexts
thread_local! {
    pub(crate) static SESSION_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime for session operations");
}

/// Global session manager that maintains all active sessions
pub struct SessionManager {
    /// Registry of all active sessions indexed by session_id
    sessions: RwLock<HashMap<String, Arc<RwLock<UserSession>>>>,
    /// Transaction manager for creating new sessions
    transaction_manager: Arc<TransactionManager>,
    /// Storage manager - singleton shared across all sessions
    storage_manager: Arc<StorageManager>,
    /// Catalog manager - singleton shared across all sessions
    catalog_manager: Arc<RwLock<CatalogManager>>,
}

impl SessionManager {
    /// Create a new session manager with all required singleton components
    pub fn new(
        transaction_manager: Arc<TransactionManager>,
        storage_manager: Arc<StorageManager>,
        catalog_manager: Arc<RwLock<CatalogManager>>,
    ) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            transaction_manager,
            storage_manager,
            catalog_manager,
        }
    }

    /// Get the storage manager
    pub fn get_storage_manager(&self) -> Arc<StorageManager> {
        self.storage_manager.clone()
    }

    /// Get the catalog manager
    pub fn get_catalog_manager(&self) -> Arc<RwLock<CatalogManager>> {
        self.catalog_manager.clone()
    }

    /// Get the transaction manager
    pub fn get_transaction_manager(&self) -> Arc<TransactionManager> {
        self.transaction_manager.clone()
    }

    /// Create a new session and return its ID
    pub fn create_session(
        &self,
        username: String,
        roles: Vec<String>,
        permissions: SessionPermissionCache,
    ) -> Result<String, String> {
        let user_session = UserSession::new(
            username,
            roles,
            permissions,
            self.transaction_manager.clone(),
        );

        let session_id = user_session.session_id.clone();
        let session_arc = Arc::new(RwLock::new(user_session));

        {
            let mut sessions = self
                .sessions
                .write()
                .map_err(|_| "Failed to acquire sessions write lock")?;
            sessions.insert(session_id.clone(), session_arc);
        }

        Ok(session_id)
    }

    /// Get a session by ID
    pub fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<UserSession>>> {
        let sessions = self.sessions.read().ok()?;
        sessions.get(session_id).cloned()
    }

    /// Remove a session from the registry
    pub fn remove_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| "Failed to acquire sessions write lock")?;

        if let Some(session_arc) = sessions.remove(session_id) {
            // Mark session as inactive
            if let Ok(mut session) = session_arc.write() {
                session.deactivate();
            }
        }

        // Persist catalogs when session is removed to ensure data is saved
        if let Ok(catalog_manager) = self.catalog_manager.write() {
            let persist_result =
                SESSION_RUNTIME.with(|rt| rt.block_on(catalog_manager.persist_all()));
            if let Err(e) = persist_result {
                log::warn!("Failed to persist catalogs during session removal: {}", e);
            }
        }

        Ok(())
    }

    /// Get all active session IDs
    pub fn get_active_session_ids(&self) -> Vec<String> {
        let sessions = match self.sessions.read() {
            Ok(sessions) => sessions,
            Err(_) => return Vec::new(), // In case of poisoned lock, return empty vec
        };
        sessions.keys().cloned().collect()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired_sessions(&self) -> Result<usize, String> {
        let expired_ids: Vec<String> = {
            let sessions = self
                .sessions
                .read()
                .map_err(|_| "Failed to acquire sessions read lock")?;

            sessions
                .iter()
                .filter_map(|(id, session_arc)| {
                    if let Ok(session) = session_arc.read() {
                        if session.is_expired() {
                            Some(id.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect()
        };

        let count = expired_ids.len();
        for session_id in expired_ids {
            self.remove_session(&session_id)?;
        }

        Ok(count)
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions
            .read()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    /// Create an anonymous session for internal/testing use
    pub fn create_anonymous_session(&self) -> Result<String, String> {
        let permissions = SessionPermissionCache::new();
        self.create_session(
            "anonymous".to_string(),
            vec!["user".to_string()],
            permissions,
        )
    }

    /// Invalidate all sessions currently using the specified graph
    /// This is called when a graph is dropped to prevent stale data access
    /// Returns the number of sessions that were invalidated
    pub fn invalidate_sessions_for_graph(&self, graph_name: &str) -> usize {
        let sessions_guard = match self.sessions.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!("Failed to acquire sessions read lock for graph invalidation");
                return 0;
            }
        };

        let mut invalidated_count = 0;

        // Iterate through all sessions and invalidate those using the dropped graph
        for (session_id, session_arc) in sessions_guard.iter() {
            let mut session = match session_arc.write() {
                Ok(guard) => guard,
                Err(_) => {
                    log::warn!("Failed to acquire write lock for session {}", session_id);
                    continue;
                }
            };

            // Check if this session is currently using the dropped graph
            if let Some(current_graph) = &session.current_graph {
                let current_graph_clone = current_graph.clone(); // Clone to avoid borrowing issues

                // Handle both "/graph_name" and "graph_name" formats
                let matches = current_graph == graph_name
                    || current_graph == &format!("/{}", graph_name)
                    || current_graph.strip_prefix('/') == Some(graph_name);

                if matches {
                    // Clear only the graph context to prevent stale data access
                    // Keep the schema since it's independent of any particular graph
                    session.current_graph = None;

                    log::info!(
                        "Invalidated session {} using dropped graph '{}' (was: '{}')",
                        session_id,
                        graph_name,
                        current_graph_clone
                    );
                    invalidated_count += 1;
                } else {
                    log::debug!(
                        "Session {} using different graph '{}', not invalidating for '{}'",
                        session_id,
                        current_graph_clone,
                        graph_name
                    );
                }
            }
        }

        if invalidated_count > 0 {
            log::info!(
                "Successfully invalidated {} sessions using dropped graph '{}'",
                invalidated_count,
                graph_name
            );
        }

        invalidated_count
    }

    /// Graceful shutdown - persist all catalogs and close all sessions
    pub fn shutdown(&self) -> Result<(), String> {
        log::info!("SessionManager shutting down gracefully...");

        // Persist all catalogs before shutdown
        if let Ok(catalog_manager) = self.catalog_manager.write() {
            let persist_result =
                SESSION_RUNTIME.with(|rt| rt.block_on(catalog_manager.persist_all()));
            if let Err(e) = persist_result {
                log::error!("Failed to persist catalogs during shutdown: {}", e);
                return Err(format!("Failed to persist catalogs during shutdown: {}", e));
            }
            log::info!("Successfully persisted all catalogs during shutdown");
        }

        // Close all active sessions
        let session_ids: Vec<String> = {
            if let Ok(sessions) = self.sessions.read() {
                sessions.keys().cloned().collect()
            } else {
                Vec::new()
            }
        };

        for session_id in session_ids {
            if let Ok(mut sessions) = self.sessions.write() {
                if let Some(session_arc) = sessions.remove(&session_id) {
                    if let Ok(mut session) = session_arc.write() {
                        session.deactivate();
                    }
                }
            }
        }

        // Shutdown storage manager to release file locks
        if let Err(e) = self.storage_manager.shutdown() {
            log::error!("Failed to shutdown storage manager: {}", e);
            return Err(format!("Failed to shutdown storage manager: {}", e));
        }

        log::info!("SessionManager shutdown completed");
        Ok(())
    }
}
