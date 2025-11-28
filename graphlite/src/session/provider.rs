// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Session provider abstraction for flexible session management
//!
//! This module provides a trait-based abstraction for session management,
//! allowing different storage strategies (instance-based vs global).

use crate::catalog::manager::CatalogManager;
use crate::session::models::{SessionPermissionCache, UserSession};
use crate::storage::StorageManager;
use crate::txn::TransactionManager;
use std::any::Any;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Abstract session provider interface
///
/// This trait defines the contract for session management,
/// allowing different implementations:
/// - InstanceSessionProvider: Each QueryCoordinator owns its sessions (embedded mode)
/// - GlobalSessionProvider: Process-wide global sessions (server mode)
pub trait SessionProvider: Send + Sync {
    /// Create a new session
    ///
    /// # Arguments
    /// * `username` - Username for the session
    /// * `roles` - List of roles assigned to the user
    /// * `permissions` - Session permission cache
    ///
    /// # Returns
    /// * `Ok(session_id)` - Unique session identifier
    /// * `Err(msg)` - Error message if creation fails
    fn create_session(
        &self,
        username: String,
        roles: Vec<String>,
        permissions: SessionPermissionCache,
    ) -> Result<String, String>;

    /// Get an existing session by ID
    ///
    /// # Arguments
    /// * `session_id` - The session identifier
    ///
    /// # Returns
    /// * `Some(session)` - The session if found
    /// * `None` - If session doesn't exist
    fn get_session(&self, session_id: &str) -> Option<Arc<RwLock<UserSession>>>;

    /// Remove a session
    ///
    /// # Arguments
    /// * `session_id` - The session identifier to remove
    ///
    /// # Returns
    /// * `Ok(())` - Session removed successfully
    /// * `Err(msg)` - Error message if removal fails
    fn remove_session(&self, session_id: &str) -> Result<(), String>;

    /// Get all active session IDs
    ///
    /// # Returns
    /// Vector of active session IDs
    fn list_sessions(&self) -> Vec<String>;

    /// Clean up expired sessions based on idle time
    ///
    /// # Arguments
    /// * `max_idle` - Maximum idle duration before expiration
    ///
    /// # Returns
    /// Number of sessions cleaned up
    fn cleanup_expired(&self, max_idle: Duration) -> usize;

    /// Graceful shutdown - persist state and close all sessions
    ///
    /// # Returns
    /// * `Ok(())` - Shutdown successful
    /// * `Err(msg)` - Error message if shutdown fails
    fn shutdown(&self) -> Result<(), String>;

    /// Get session count
    ///
    /// # Returns
    /// Number of active sessions
    fn session_count(&self) -> usize {
        self.list_sessions().len()
    }

    /// Invalidate sessions using a specific graph
    ///
    /// # Arguments
    /// * `graph_name` - The graph that was dropped
    ///
    /// # Returns
    /// Number of sessions invalidated
    fn invalidate_sessions_for_graph(&self, graph_name: &str) -> usize;

    /// Get the storage manager
    ///
    /// # Returns
    /// Arc reference to the storage manager
    fn get_storage_manager(&self) -> Arc<StorageManager>;

    /// Get the catalog manager
    ///
    /// # Returns
    /// Arc reference to the catalog manager
    fn get_catalog_manager(&self) -> Arc<RwLock<CatalogManager>>;

    /// Get the transaction manager
    ///
    /// # Returns
    /// Arc reference to the transaction manager
    fn get_transaction_manager(&self) -> Arc<TransactionManager>;

    /// Downcast to concrete type (for backward compatibility)
    ///
    /// # Returns
    /// &dyn Any for downcasting
    fn as_any(&self) -> &dyn Any;
}
