// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Unified session models for database session management
//!
//! This module provides a consolidated session management model that combines
//! authentication, authorization, and database session state management.

use crate::session::transaction_state::SessionTransactionState;
use crate::storage::{GraphCache, StorageManager, Value};
use crate::txn::TransactionManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Stub for session permission cache
#[derive(Clone, Default)]
pub struct SessionPermissionCache {}

impl SessionPermissionCache {
    pub fn new() -> Self {
        Self {}
    }

    /// Check if user can access a specific graph (stub)
    pub fn can_access_graph(&self, _graph_name: &str, _action: &str) -> bool {
        true // Stub: allow all access
    }

    /// Check if user can access a specific schema (stub)
    pub fn can_access_schema(&self, _schema_name: &str, _action: &str) -> bool {
        true // Stub: allow all access
    }

    /// Check if user can perform a system operation (stub)
    pub fn can_perform_operation(&self, _operation: &str) -> bool {
        true // Stub: allow all operations
    }
}

/// Per-session catalog metadata cache
///
/// Caches frequently accessed catalog metadata to reduce contention on the
/// global CatalogManager. Uses version-based invalidation to ensure consistency.
#[derive(Clone, Debug)]
pub struct SessionCatalogCache {
    /// Cached list of schema names
    schema_list: Option<Vec<String>>,
    /// Version of cached schema list
    schema_list_version: u64,

    /// Cached graph names by schema
    graphs_by_schema: HashMap<String, Vec<String>>,
    /// Version of cached graph lists
    graphs_version: u64,

    /// Last time schema list was accessed (for metrics)
    last_schema_access: Option<Instant>,
    /// Last time graph list was accessed (for metrics)
    last_graph_access: Option<Instant>,
}

impl SessionCatalogCache {
    /// Create a new empty catalog cache
    pub fn new() -> Self {
        Self {
            schema_list: None,
            schema_list_version: 0,
            graphs_by_schema: HashMap::new(),
            graphs_version: 0,
            last_schema_access: None,
            last_graph_access: None,
        }
    }

    /// Check if schema list cache is valid for the given version
    pub fn is_schema_list_valid(&self, current_version: u64) -> bool {
        self.schema_list.is_some() && self.schema_list_version == current_version
    }

    /// Get cached schema list if valid
    pub fn get_schema_list(&mut self, current_version: u64) -> Option<Vec<String>> {
        if self.is_schema_list_valid(current_version) {
            self.last_schema_access = Some(Instant::now());
            self.schema_list.clone()
        } else {
            None
        }
    }

    /// Update schema list cache
    pub fn set_schema_list(&mut self, schemas: Vec<String>, version: u64) {
        self.schema_list = Some(schemas);
        self.schema_list_version = version;
        self.last_schema_access = Some(Instant::now());
    }

    /// Invalidate schema list cache
    pub fn invalidate_schema_list(&mut self) {
        self.schema_list = None;
        self.schema_list_version = 0;
    }

    /// Check if graph list cache is valid for the given schema and version
    pub fn is_graph_list_valid(&self, schema_name: &str, current_version: u64) -> bool {
        self.graphs_by_schema.contains_key(schema_name) && self.graphs_version == current_version
    }

    /// Get cached graph list for a schema if valid
    pub fn get_graph_list(
        &mut self,
        schema_name: &str,
        current_version: u64,
    ) -> Option<Vec<String>> {
        if self.is_graph_list_valid(schema_name, current_version) {
            self.last_graph_access = Some(Instant::now());
            self.graphs_by_schema.get(schema_name).cloned()
        } else {
            None
        }
    }

    /// Update graph list cache for a schema
    pub fn set_graph_list(&mut self, schema_name: String, graphs: Vec<String>, version: u64) {
        self.graphs_by_schema.insert(schema_name, graphs);
        self.graphs_version = version;
        self.last_graph_access = Some(Instant::now());
    }

    /// Invalidate graph list cache (all schemas)
    pub fn invalidate_graph_lists(&mut self) {
        self.graphs_by_schema.clear();
        self.graphs_version = 0;
    }

    /// Invalidate graph list cache for a specific schema
    pub fn invalidate_graph_list_for_schema(&mut self, schema_name: &str) {
        self.graphs_by_schema.remove(schema_name);
    }

    /// Clear all cached data
    pub fn clear(&mut self) {
        self.schema_list = None;
        self.schema_list_version = 0;
        self.graphs_by_schema.clear();
        self.graphs_version = 0;
        self.last_schema_access = None;
        self.last_graph_access = None;
    }

    /// Get cache statistics (for monitoring)
    pub fn get_stats(&self) -> CatalogCacheStats {
        CatalogCacheStats {
            schema_list_cached: self.schema_list.is_some(),
            graph_schemas_cached: self.graphs_by_schema.len(),
            last_schema_access: self.last_schema_access,
            last_graph_access: self.last_graph_access,
        }
    }
}

impl Default for SessionCatalogCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for monitoring
#[derive(Debug, Clone)]
pub struct CatalogCacheStats {
    pub schema_list_cached: bool,
    pub graph_schemas_cached: usize,
    pub last_schema_access: Option<Instant>,
    pub last_graph_access: Option<Instant>,
}

/// Unified user session that combines authentication and database session management
#[derive(Clone)]
pub struct UserSession {
    // === Identity Information ===
    /// Unique session identifier
    pub session_id: String,
    /// User identifier (unique across system)
    pub user_id: String,
    /// User display name
    pub username: String,
    /// User roles for authorization
    pub roles: Vec<String>,

    // === Database Session Context ===
    /// Current graph being used (equivalent to current database/schema)
    pub current_graph: Option<String>,
    /// Current schema context
    pub current_schema: Option<String>,
    /// Current timezone setting
    pub current_timezone: Option<String>,

    // === Session State ===
    /// Session parameters (SET commands, user variables)
    pub parameters: HashMap<String, Value>,
    /// Authorization permission cache
    pub permissions: SessionPermissionCache,
    /// Transaction state for this session (shared reference)
    pub transaction_state: Arc<SessionTransactionState>,
    /// Catalog metadata cache (reduces contention on CatalogManager)
    pub catalog_cache: SessionCatalogCache,

    // === Session Lifecycle ===
    /// When the session was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Whether the session is currently active
    pub active: bool,
}

impl UserSession {
    /// Create a new session from authentication information
    pub fn new(
        username: String,
        roles: Vec<String>,
        permissions: SessionPermissionCache,
        transaction_manager: Arc<TransactionManager>,
    ) -> Self {
        let now = chrono::Utc::now();
        let session_id = uuid::Uuid::new_v4().to_string();
        let transaction_state = Arc::new(SessionTransactionState::new(transaction_manager));

        Self {
            session_id,
            user_id: username.clone(), // Using username as user_id for now
            username,
            roles,
            current_graph: None,
            current_schema: None,
            current_timezone: None,
            parameters: HashMap::new(),
            permissions,
            transaction_state,
            catalog_cache: SessionCatalogCache::new(),
            created_at: now,
            last_activity: now,
            active: true,
        }
    }

    // === Graph Context Management ===

    /// Set the current graph for this session
    pub fn set_current_graph(&mut self, graph_path: Option<String>) {
        self.current_graph = graph_path;
        self.update_activity();
    }

    /// Get the current graph for this session
    pub fn get_current_graph(&self) -> Option<&String> {
        self.current_graph.as_ref()
    }

    /// Clear the current graph
    pub fn clear_current_graph(&mut self) {
        self.current_graph = None;
        self.update_activity();
    }

    /// Set the current schema for this session
    pub fn set_current_schema(&mut self, schema: Option<String>) {
        self.current_schema = schema;
        self.update_activity();
    }

    /// Get the current schema for this session
    pub fn get_current_schema(&self) -> Option<&String> {
        self.current_schema.as_ref()
    }

    /// Set the current timezone for this session
    pub fn set_current_timezone(&mut self, timezone: Option<String>) {
        self.current_timezone = timezone;
        self.update_activity();
    }

    /// Get the current timezone for this session
    pub fn get_current_timezone(&self) -> Option<&String> {
        self.current_timezone.as_ref()
    }

    // === Session Parameter Management ===

    /// Set a session parameter
    pub fn set_parameter(&mut self, key: String, value: Value) {
        self.parameters.insert(key, value);
        self.update_activity();
    }

    /// Get a session parameter
    pub fn get_parameter(&self, key: &str) -> Option<&Value> {
        self.parameters.get(key)
    }

    /// Remove a session parameter
    pub fn remove_parameter(&mut self, key: &str) -> Option<Value> {
        self.update_activity();
        self.parameters.remove(key)
    }

    /// Clear all session parameters
    pub fn clear_parameters(&mut self) {
        self.parameters.clear();
        self.update_activity();
    }

    // === Schema Enforcement Configuration ===

    /// Set the schema enforcement mode for this session
    /// Valid values: "strict", "advisory", "disabled"
    pub fn set_schema_enforcement_mode(&mut self, mode: &str) {
        self.set_parameter(
            "schema_enforcement_mode".to_string(),
            Value::String(mode.to_string()),
        );
    }

    /// Get the current schema enforcement mode
    /// Returns "advisory" as default if not set
    pub fn get_schema_enforcement_mode(&self) -> &str {
        self.get_parameter("schema_enforcement_mode")
            .and_then(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .unwrap_or("advisory")
    }

    /// Set whether to validate on write operations
    pub fn set_validate_on_write(&mut self, enabled: bool) {
        self.set_parameter(
            "schema_validate_on_write".to_string(),
            Value::Boolean(enabled),
        );
    }

    /// Get whether to validate on write operations (default: true)
    pub fn get_validate_on_write(&self) -> bool {
        self.get_parameter("schema_validate_on_write")
            .and_then(|v| match v {
                Value::Boolean(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(true)
    }

    /// Set whether to allow unknown properties not defined in schema
    pub fn set_allow_unknown_properties(&mut self, allow: bool) {
        self.set_parameter(
            "schema_allow_unknown_properties".to_string(),
            Value::Boolean(allow),
        );
    }

    /// Get whether to allow unknown properties (default: false in strict mode, true otherwise)
    pub fn get_allow_unknown_properties(&self) -> bool {
        self.get_parameter("schema_allow_unknown_properties")
            .and_then(|v| match v {
                Value::Boolean(b) => Some(*b),
                _ => None,
            })
            .unwrap_or_else(|| self.get_schema_enforcement_mode() != "strict")
    }

    // === Session Lifecycle Management ===

    /// Update the last activity timestamp
    pub fn update_activity(&mut self) {
        self.last_activity = chrono::Utc::now();
    }

    /// Check if the session is expired (1 hour timeout)
    pub fn is_expired(&self) -> bool {
        let timeout = chrono::Duration::hours(1);
        chrono::Utc::now() - self.last_activity > timeout
    }

    /// Mark session as inactive
    pub fn deactivate(&mut self) {
        self.active = false;
        self.update_activity();
    }

    /// Reset session to defaults (clears graph context and parameters)
    pub fn reset(&mut self, reset_target: Option<SessionResetTarget>) {
        match reset_target {
            Some(SessionResetTarget::Graph) => {
                self.current_graph = None;
            }
            Some(SessionResetTarget::Schema) => {
                self.current_schema = None;
            }
            Some(SessionResetTarget::TimeZone) => {
                self.current_timezone = None;
            }
            Some(SessionResetTarget::AllParameters) => {
                self.parameters.clear();
            }
            Some(SessionResetTarget::AllCharacteristics) => {
                self.current_graph = None;
                self.current_schema = None;
                self.current_timezone = None;
            }
            Some(SessionResetTarget::Parameter(param_name)) => {
                self.parameters.remove(&param_name);
            }
            None => {
                // Reset everything except identity and permissions
                self.current_graph = None;
                self.current_schema = None;
                self.current_timezone = None;
                self.parameters.clear();
            }
        }
        self.update_activity();
    }

    // === Catalog Cache Management ===

    /// Get mutable reference to catalog cache
    pub fn catalog_cache_mut(&mut self) -> &mut SessionCatalogCache {
        &mut self.catalog_cache
    }

    /// Get immutable reference to catalog cache
    pub fn catalog_cache_ref(&self) -> &SessionCatalogCache {
        &self.catalog_cache
    }

    /// Clear catalog cache (called on schema/graph changes)
    pub fn clear_catalog_cache(&mut self) {
        self.catalog_cache.clear();
        self.update_activity();
    }

    // === Authorization ===

    /// Check if user has a specific role
    pub fn has_role(&self, role_name: &str) -> bool {
        self.roles.contains(&role_name.to_string())
    }

    /// Check if user can access a specific graph
    pub fn can_access_graph(&self, graph_name: &str, action: &str) -> bool {
        self.permissions.can_access_graph(graph_name, action)
    }

    /// Check if user can access a specific schema
    pub fn can_access_schema(&self, schema_name: &str, action: &str) -> bool {
        self.permissions.can_access_schema(schema_name, action)
    }

    /// Check if user can perform a system operation
    pub fn can_perform_operation(&self, operation: &str) -> bool {
        self.permissions.can_perform_operation(operation)
    }
}

/// Session reset targets (from exec::context)
#[derive(Debug, Clone)]
pub enum SessionResetTarget {
    Schema,
    Graph,
    TimeZone,
    AllParameters,
    AllCharacteristics,
    Parameter(String),
}

/// Simplified session for metadata tracking integration
///
/// This is a streamlined session model specifically designed for metadata
/// tracking and testing purposes. It focuses on graph management and user
/// context without the full complexity of UserSession.
#[derive(Debug, Clone)]
pub struct Session {
    /// Username for metadata tracking
    pub username: String,

    /// Storage manager reference
    pub storage: Arc<StorageManager>,

    /// Current graph name
    #[allow(dead_code)]
    // FALSE POSITIVE - Used in set_current_graph() (line 346), get_current_graph() (line 365), get_current_graph_mut() (line 356). Compiler cannot detect field usage across module boundaries when accessed through public API methods.
    pub current_graph: Option<String>,

    /// Graph caches by name
    #[allow(dead_code)]
    // FALSE POSITIVE - Used in set_current_graph() (line 350), get_current_graph() (line 366), get_current_graph_mut() (line 357). Compiler cannot detect field usage across module boundaries when accessed through public API methods.
    pub graphs: HashMap<String, GraphCache>,
}

impl Session {
    /// Create a new session with user and storage
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn new_with_user(username: &str, storage: Arc<StorageManager>) -> Self {
        Self {
            username: username.to_string(),
            storage,
            current_graph: None,
            graphs: HashMap::new(),
        }
    }

    /// Set the current graph for this session
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn set_current_graph(&mut self, name: &str) {
        self.current_graph = Some(name.to_string());

        // Ensure the graph exists in our cache
        if !self.graphs.contains_key(name) {
            self.graphs.insert(name.to_string(), GraphCache::new());
        }
    }

    /// Get a mutable reference to the current graph
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn get_current_graph_mut(&mut self) -> Option<&mut GraphCache> {
        if let Some(graph_name) = &self.current_graph {
            self.graphs.get_mut(graph_name)
        } else {
            None
        }
    }

    /// Get a reference to the current graph
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn get_current_graph(&self) -> Option<&GraphCache> {
        if let Some(graph_name) = &self.current_graph {
            self.graphs.get(graph_name)
        } else {
            None
        }
    }

    /// Get a mutable reference to a specific graph
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn get_graph_mut(&mut self, name: &str) -> Option<&mut GraphCache> {
        self.graphs.get_mut(name)
    }

    /// Get a reference to a specific graph
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn get_graph(&self, name: &str) -> Option<&GraphCache> {
        self.graphs.get(name)
    }

    /// Create or get a graph by name
    #[allow(dead_code)] // ROADMAP v0.2.0 - Session management for multi-user support (see ROADMAP.md §2)
    pub fn get_or_create_graph(&mut self, name: &str) -> &mut GraphCache {
        self.graphs.entry(name.to_string()).or_default()
    }
}
