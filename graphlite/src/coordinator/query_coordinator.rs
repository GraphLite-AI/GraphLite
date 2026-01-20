// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query Coordinator - Simplified orchestration for GraphLite query execution
//!
//! This provides a clean API that wraps the session manager and properly
//! coordinates query execution through the standard GraphLite components.

use crate::ast::parser::parse_query;
use crate::cache::CacheManager;
use crate::catalog::manager::CatalogManager;
use crate::exec::{ExecutionRequest, QueryExecutor, QueryResult};
use crate::session::{
    GlobalSessionProvider, InstanceSessionProvider, SessionManager, SessionMode, SessionProvider,
};
use crate::storage::{StorageManager, StorageMethod, StorageType};
use crate::txn::TransactionManager;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

/// Query Coordinator - Orchestrates query execution with proper session management
///
/// This is the main entry point for executing queries in GraphLite. It handles:
/// - Session management and validation
/// - Query parsing
/// - Execution coordination
/// - Result processing
pub struct QueryCoordinator {
    /// Session provider (trait object for flexible session storage)
    session_provider: Arc<dyn SessionProvider>,
    /// Query executor
    executor: Arc<QueryExecutor>,
}

// Explicitly mark QueryCoordinator as UnwindSafe for FFI panic handling.
// This is safe because:
// 1. All mutable state is protected by Arc/RwLock which are unwind-safe
// 2. No destructors perform critical cleanup that must run
// 3. The database can recover from a panic via transaction rollback
impl UnwindSafe for QueryCoordinator {}
impl RefUnwindSafe for QueryCoordinator {}

impl QueryCoordinator {
    /// Create a new QueryCoordinator from a database path (Simplified API)
    ///
    /// This is the recommended way to create a QueryCoordinator for embedding GraphLite.
    /// It handles all internal component initialization automatically.
    ///
    /// # Arguments
    /// * `db_path` - Path to the database directory
    ///
    /// # Returns
    /// * `Ok(Arc<QueryCoordinator>)` - Initialized coordinator ready for use
    /// * `Err(String)` - Error message if initialization fails
    ///
    /// # Example
    /// ```no_run
    /// use graphlite::{QueryCoordinator, StorageType};
    ///
    /// let coordinator = QueryCoordinator::from_path("./mydb", StorageType::Sled)
    ///     .expect("Failed to initialize database");
    ///
    /// let session_id = coordinator.create_simple_session("user")
    ///     .expect("Failed to create session");
    ///
    /// let result = coordinator.process_query("MATCH (n) RETURN n", &session_id)
    ///     .expect("Failed to execute query");
    /// ```
    pub fn from_path(db_path: impl AsRef<Path>, storage_type: StorageType) -> Result<Arc<Self>, String> {
        let path = db_path.as_ref().to_path_buf();

        // Initialize storage
        let storage = Arc::new(
            StorageManager::new(path.clone(), StorageMethod::DiskOnly, storage_type)
                .map_err(|e| format!("Failed to initialize storage: {}", e))?,
        );

        // Initialize catalog manager
        let catalog_manager = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));

        // Initialize transaction manager with database path
        let transaction_manager = Arc::new(
            TransactionManager::new(path.clone())
                .map_err(|e| format!("Failed to initialize transaction manager: {}", e))?,
        );

        // Initialize cache manager
        let cache_config = crate::cache::CacheConfig::default();
        let cache_manager =
            Some(Arc::new(CacheManager::new(cache_config).map_err(|e| {
                format!("Failed to initialize cache manager: {}", e)
            })?));

        // Create instance-based session provider (embedded mode)
        let session_provider: Arc<dyn SessionProvider> = Arc::new(InstanceSessionProvider::new(
            transaction_manager.clone(),
            storage.clone(),
            catalog_manager.clone(),
        ));

        // Create query executor
        let executor = Arc::new(
            QueryExecutor::new(
                storage.clone(),
                catalog_manager.clone(),
                transaction_manager.clone(),
                session_provider.clone(),
                cache_manager,
            )
            .map_err(|e| format!("Failed to initialize query executor: {}", e))?,
        );

        Ok(Arc::new(Self::new(executor, session_provider)))
    }

    /// Create a new QueryCoordinator with specified session mode
    ///
    /// This method allows you to choose between Instance and Global session management.
    ///
    /// # Arguments
    /// * `db_path` - Path to the database directory
    /// * `mode` - Session management mode (Instance or Global)
    ///
    /// # Returns
    /// * `Ok(Arc<QueryCoordinator>)` - Initialized coordinator ready for use
    /// * `Err(String)` - Error message if initialization fails
    ///
    /// # Session Modes
    ///
    /// - **SessionMode::Instance** (default): Each QueryCoordinator instance has its own
    ///   isolated session pool. Sessions are not shared between instances. Use this mode
    ///   for embedded databases where each application instance needs complete isolation.
    ///
    /// - **SessionMode::Global**: All QueryCoordinator instances share a process-wide
    ///   session pool. Sessions created in one coordinator are visible to all coordinators
    ///   in the same process. Use this mode for server/daemon applications where multiple
    ///   coordinators need to access the same user sessions.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use graphlite::QueryCoordinator;
    /// use graphlite::SessionMode;
    ///
    /// // Embedded mode - each instance isolated (default behavior)
    /// let coord1 = QueryCoordinator::from_path_with_mode("db1.graphlite", SessionMode::Instance)?;
    /// let coord2 = QueryCoordinator::from_path_with_mode("db2.graphlite", SessionMode::Instance)?;
    /// // coord1 and coord2 have separate session pools
    ///
    /// // Server mode - shared session pool
    /// let coord1 = QueryCoordinator::from_path_with_mode("db.graphlite", SessionMode::Global)?;
    /// let coord2 = QueryCoordinator::from_path_with_mode("db.graphlite", SessionMode::Global)?;
    /// // coord1 and coord2 share the same session pool
    /// # Ok::<(), String>(())
    /// ```
    pub fn from_path_with_mode(
        db_path: impl AsRef<Path>,
        mode: SessionMode,
    ) -> Result<Arc<Self>, String> {
        let path = db_path.as_ref().to_path_buf();

        // Initialize storage
        let storage = Arc::new(
            StorageManager::new(path.clone(), StorageMethod::DiskOnly, StorageType::Sled)
                .map_err(|e| format!("Failed to initialize storage: {}", e))?,
        );

        // Initialize catalog manager
        let catalog_manager = Arc::new(RwLock::new(CatalogManager::new(storage.clone())));

        // Initialize transaction manager with database path
        let transaction_manager = Arc::new(
            TransactionManager::new(path.clone())
                .map_err(|e| format!("Failed to initialize transaction manager: {}", e))?,
        );

        // Initialize cache manager
        let cache_config = crate::cache::CacheConfig::default();
        let cache_manager =
            Some(Arc::new(CacheManager::new(cache_config).map_err(|e| {
                format!("Failed to initialize cache manager: {}", e)
            })?));

        // Create session provider based on mode
        let session_provider: Arc<dyn SessionProvider> = match mode {
            SessionMode::Instance => {
                // Instance mode: Each coordinator has isolated sessions
                Arc::new(InstanceSessionProvider::new(
                    transaction_manager.clone(),
                    storage.clone(),
                    catalog_manager.clone(),
                ))
            }
            SessionMode::Global => {
                // Global mode: All coordinators share process-wide session pool
                Arc::new(GlobalSessionProvider::new(
                    transaction_manager.clone(),
                    storage.clone(),
                    catalog_manager.clone(),
                ))
            }
        };

        // Create query executor
        let executor = Arc::new(
            QueryExecutor::new(
                storage.clone(),
                catalog_manager.clone(),
                transaction_manager.clone(),
                session_provider.clone(),
                cache_manager,
            )
            .map_err(|e| format!("Failed to initialize query executor: {}", e))?,
        );

        Ok(Arc::new(Self::new(executor, session_provider)))
    }

    /// Create a new QueryCoordinator (Advanced API)
    ///
    /// This is an advanced constructor for cases where you need fine-grained control
    /// over component initialization. Most users should use `from_path()` instead.
    ///
    /// # Arguments
    /// * `executor` - Query executor
    /// * `session_provider` - Session provider (trait object)
    pub fn new(executor: Arc<QueryExecutor>, session_provider: Arc<dyn SessionProvider>) -> Self {
        Self {
            session_provider,
            executor,
        }
    }

    /// Execute a query with session ID
    ///
    /// This is the main entry point for query execution.
    ///
    /// # Arguments
    /// * `query_text` - The GQL query string to execute
    /// * `session_id` - Session ID for the query
    ///
    /// # Returns
    /// * `Ok(QueryResult)` - Query result on success
    /// * `Err(String)` - Error message on failure
    pub fn process_query(&self, query_text: &str, session_id: &str) -> Result<QueryResult, String> {
        // Parse query
        let document = parse_query(query_text).map_err(|e| format!("Parse error: {:?}", e))?;

        // Get session
        let session = self.session_provider.get_session(session_id);

        // Create execution request
        let request = ExecutionRequest::new(document.statement)
            .with_session(session)
            .with_query_text(Some(query_text.to_string()));

        // Execute query
        let result = self
            .executor
            .execute_query(request)
            .map_err(|e| format!("Execution error: {:?}", e))?;

        // Process any session results (SET GRAPH, SET SCHEMA, etc.)
        if let Some(ref session_result) = result.session_result {
            self.handle_session_result(session_result, session_id)?;
        }

        Ok(result)
    }

    /// Handle session-modifying results (SET GRAPH, SET SCHEMA)
    fn handle_session_result(
        &self,
        session_result: &crate::exec::SessionResult,
        session_id: &str,
    ) -> Result<(), String> {
        use crate::ast::GraphExpression;

        match session_result {
            crate::exec::SessionResult::SetGraph {
                graph_expression,
                validated: _,
            } => {
                // Get session
                let session_arc = self
                    .session_provider
                    .get_session(session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;

                let mut session = session_arc
                    .write()
                    .map_err(|e| format!("Failed to acquire session write lock: {}", e))?;

                // Extract the graph path from the expression
                let graph_path = match graph_expression {
                    GraphExpression::Reference(catalog_path) => {
                        match catalog_path.segments.len() {
                            2 => {
                                // Full path: /schema_name/graph_name
                                format!("/{}", catalog_path.segments.join("/"))
                            }
                            1 => {
                                // Relative path: graph_name only - use session schema
                                let graph_name = &catalog_path.segments[0];
                                match &session.current_schema {
                                    Some(session_schema) => {
                                        let schema_name = session_schema
                                            .strip_prefix('/')
                                            .unwrap_or(session_schema);
                                        format!("/{}/{}", schema_name, graph_name)
                                    }
                                    None => {
                                        return Err(
                                            "Cannot use relative graph path without current schema set. Use SESSION SET SCHEMA or provide full path /schema_name/graph_name".to_string()
                                        );
                                    }
                                }
                            }
                            _ => {
                                return Err("Invalid graph path format".to_string());
                            }
                        }
                    }
                    GraphExpression::CurrentGraph => {
                        return Err("CURRENT_GRAPH cannot be used in SESSION SET GRAPH".to_string());
                    }
                    GraphExpression::Union { .. } => {
                        return Err(
                            "UNION expressions cannot be used in SESSION SET GRAPH".to_string()
                        );
                    }
                };

                // Update session's current graph
                session.current_graph = Some(graph_path.clone());
                log::debug!("Session {} graph set to: {}", session_id, graph_path);

                Ok(())
            }
            crate::exec::SessionResult::SetSchema {
                schema_reference,
                validated: _,
            } => {
                // Get session
                let session_arc = self
                    .session_provider
                    .get_session(session_id)
                    .ok_or_else(|| format!("Session not found: {}", session_id))?;

                let mut session = session_arc
                    .write()
                    .map_err(|e| format!("Failed to acquire session write lock: {}", e))?;

                // Update session's current schema
                let schema_path = format!("/{}", schema_reference.segments.join("/"));
                session.current_schema = Some(schema_path.clone());
                log::debug!("Session {} schema set to: {}", session_id, schema_path);

                Ok(())
            }
            _ => Ok(()), // Other session results don't need special handling
        }
    }

    /// Create a simple session with default permissions (Simplified API)
    ///
    /// This is the recommended way to create a session for most use cases.
    /// It creates a session with full permissions for the given username.
    ///
    /// # Arguments
    /// * `username` - Username for the session
    ///
    /// # Returns
    /// * `Ok(String)` - Session ID for use with `process_query()`
    /// * `Err(String)` - Error message if session creation fails
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// let session_id = coordinator.create_simple_session("user")
    ///     .expect("Failed to create session");
    /// ```
    pub fn create_simple_session(&self, username: impl Into<String>) -> Result<String, String> {
        use crate::session::SessionPermissionCache;

        // Create default permissions with full access
        let permissions = SessionPermissionCache::default();

        self.session_provider
            .create_session(username.into(), vec![], permissions)
    }

    /// Create a new session with custom permissions (Advanced API)
    ///
    /// This is an advanced method for cases where you need fine-grained control
    /// over session permissions and roles. Most users should use `create_simple_session()` instead.
    ///
    /// # Arguments
    /// * `username` - Username for the session
    /// * `roles` - List of role names for the user
    /// * `permissions` - Permission cache for the session
    ///
    /// # Returns
    /// * `Ok(String)` - Session ID for use with `process_query()`
    /// * `Err(String)` - Error message if session creation fails
    pub fn create_session(
        &self,
        username: String,
        roles: Vec<String>,
        permissions: crate::session::SessionPermissionCache,
    ) -> Result<String, String> {
        self.session_provider
            .create_session(username, roles, permissions)
    }

    /// Authenticate a user and create a session
    ///
    /// Authenticates the user against the security catalog and creates a session
    /// with appropriate roles if authentication succeeds.
    ///
    /// # Arguments
    /// * `username` - Username to authenticate
    /// * `password` - Password for authentication
    ///
    /// # Returns
    /// * `Ok(String)` - Session ID for the authenticated user
    /// * `Err(String)` - Error message if authentication fails
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// let session_id = coordinator.authenticate_and_create_session("admin", "password")
    ///     .expect("Authentication failed");
    /// ```
    pub fn authenticate_and_create_session(
        &self,
        username: &str,
        password: &str,
    ) -> Result<String, String> {
        use crate::catalog::operations::{CatalogResponse, QueryType};
        use crate::session::SessionPermissionCache;

        // Get catalog manager and authenticate
        let catalog_manager = self.session_provider.get_catalog_manager();
        let catalog_lock = catalog_manager
            .read()
            .map_err(|_| "Failed to acquire catalog lock".to_string())?;

        let auth_result = catalog_lock
            .query_read_only(
                "security",
                QueryType::Authenticate,
                serde_json::json!({
                    "username": username,
                    "password": password,
                }),
            )
            .map_err(|e| format!("Authentication query failed: {}", e))?;

        // Parse authentication result
        let (authenticated, user_roles) = match auth_result {
            CatalogResponse::Query { results } => {
                let authenticated = results
                    .get("authenticated")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let roles = if authenticated {
                    results
                        .get("roles")
                        .and_then(|v| v.as_object())
                        .map(|obj| obj.keys().cloned().collect())
                        .unwrap_or_else(|| vec!["user".to_string()])
                } else {
                    vec![]
                };

                (authenticated, roles)
            }
            _ => (false, vec![]),
        };

        drop(catalog_lock); // Release lock before creating session

        if !authenticated {
            return Err("Authentication failed: Invalid credentials".to_string());
        }

        // Create session with authenticated user's roles
        self.session_provider.create_session(
            username.to_string(),
            user_roles,
            SessionPermissionCache::new(),
        )
    }

    /// Set a user's password (for installation and admin operations)
    ///
    /// Updates the password for an existing user in the security catalog.
    /// This is primarily used during database initialization.
    ///
    /// # Arguments
    /// * `username` - Username to update
    /// * `password` - New password for the user
    ///
    /// # Returns
    /// * `Ok(())` - Password updated successfully
    /// * `Err(String)` - Error message if update fails
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// coordinator.set_user_password("admin", "newpassword")
    ///     .expect("Failed to set password");
    /// ```
    pub fn set_user_password(&self, username: &str, password: &str) -> Result<(), String> {
        use crate::catalog::operations::{CatalogOperation, EntityType};

        let catalog_manager = self.session_provider.get_catalog_manager();
        let mut catalog_lock = catalog_manager
            .write()
            .map_err(|_| "Failed to acquire catalog write lock".to_string())?;

        // Update user password
        catalog_lock
            .execute(
                "security",
                CatalogOperation::Update {
                    entity_type: EntityType::User,
                    name: username.to_string(),
                    updates: serde_json::json!({
                        "password": password,
                        "enabled": true,
                    }),
                },
            )
            .map_err(|e| format!("Failed to update user password: {}", e))?;

        // Persist changes to disk
        catalog_lock
            .persist_catalog("security")
            .map_err(|e| format!("Failed to persist security catalog: {}", e))?;

        Ok(())
    }

    /// Close a session
    ///
    /// Removes the session from the session manager.
    pub fn close_session(&self, session_id: &str) -> Result<(), String> {
        self.session_provider.remove_session(session_id)
    }

    /// Get the session manager reference (for backward compatibility)
    ///
    /// Note: This returns the underlying SessionManager from the InstanceSessionProvider.
    /// For new code, prefer using session_provider methods directly through QueryCoordinator.
    pub fn session_manager(&self) -> Arc<SessionManager> {
        // For Phase 0, we know the provider is InstanceSessionProvider
        // This method provides backward compatibility
        if let Some(instance_provider) = self
            .session_provider
            .as_any()
            .downcast_ref::<InstanceSessionProvider>()
        {
            instance_provider.manager()
        } else {
            panic!("session_manager() is only supported with InstanceSessionProvider")
        }
    }

    /// Get the executor reference
    pub fn executor(&self) -> &Arc<QueryExecutor> {
        &self.executor
    }

    /// Validate query syntax without executing it
    ///
    /// This is useful for query builders, IDEs, or validating user input
    /// before attempting execution.
    ///
    /// # Arguments
    /// * `query` - The GQL query string to validate
    ///
    /// # Returns
    /// * `Ok(())` - Query is syntactically valid
    /// * `Err(String)` - Error message describing the syntax error
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// // Valid query
    /// assert!(coordinator.validate_query("MATCH (n) RETURN n").is_ok());
    ///
    /// // Invalid query
    /// assert!(coordinator.validate_query("MATCH (n RETURN n").is_err());
    /// ```
    pub fn validate_query(&self, query: &str) -> Result<(), String> {
        // Parse the query
        let document = parse_query(query).map_err(|e| format!("Parse error: {:?}", e))?;

        // Validate the parsed query (pass false for has_graph_context since we're just validating syntax)
        crate::ast::validator::validate_query(&document, false)
            .map_err(|e| format!("Validation error: {:?}", e))?;

        Ok(())
    }

    /// Check if a query is syntactically valid
    ///
    /// This is a convenience method that returns a boolean instead of an error.
    ///
    /// # Arguments
    /// * `query` - The GQL query string to check
    ///
    /// # Returns
    /// * `true` - Query is syntactically valid
    /// * `false` - Query has syntax errors
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// if coordinator.is_valid_query("MATCH (n) RETURN n") {
    ///     println!("Query is valid!");
    /// }
    /// ```
    pub fn is_valid_query(&self, query: &str) -> bool {
        self.validate_query(query).is_ok()
    }

    /// Analyze a query and return metadata without executing it
    ///
    /// This is useful for understanding query characteristics, estimating
    /// resource usage, or implementing query analysis tools.
    ///
    /// # Arguments
    /// * `query` - The GQL query string to analyze
    ///
    /// # Returns
    /// * `Ok(QueryInfo)` - Query metadata
    /// * `Err(String)` - Error if query cannot be parsed
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// let info = coordinator.analyze_query("MATCH (n:Person) RETURN n.name")
    ///     .expect("Failed to analyze query");
    ///
    /// println!("Query type: {:?}", info.query_type);
    /// println!("Read-only: {}", info.is_read_only);
    /// ```
    pub fn analyze_query(&self, query: &str) -> Result<QueryInfo, String> {
        // Parse the query
        let document = parse_query(query).map_err(|e| format!("Parse error: {:?}", e))?;

        // Analyze the statement type
        let query_type = match &document.statement {
            crate::ast::Statement::Query(_) => QueryType::Match,
            crate::ast::Statement::Select(_) => QueryType::Select,
            crate::ast::Statement::Call(_) => QueryType::Call,
            crate::ast::Statement::CatalogStatement(cat) => {
                use crate::ast::CatalogStatement;
                match cat {
                    CatalogStatement::CreateSchema { .. } => QueryType::CreateSchema,
                    CatalogStatement::DropSchema { .. } => QueryType::DropSchema,
                    CatalogStatement::CreateGraph { .. } => QueryType::CreateGraph,
                    CatalogStatement::DropGraph { .. } => QueryType::DropGraph,
                    CatalogStatement::CreateGraphType { .. } => QueryType::CreateGraphType,
                    CatalogStatement::DropGraphType { .. } => QueryType::DropGraphType,
                    CatalogStatement::AlterGraphType(_) => QueryType::AlterGraphType,
                    CatalogStatement::CreateProcedure(_) => QueryType::CreateProcedure,
                    CatalogStatement::DropProcedure(_) => QueryType::DropProcedure,
                    CatalogStatement::CreateUser { .. } => QueryType::CreateUser,
                    CatalogStatement::DropUser { .. } => QueryType::DropUser,
                    CatalogStatement::CreateRole { .. } => QueryType::CreateRole,
                    CatalogStatement::DropRole { .. } => QueryType::DropRole,
                    CatalogStatement::GrantRole { .. } => QueryType::GrantRole,
                    CatalogStatement::RevokeRole { .. } => QueryType::RevokeRole,
                    CatalogStatement::ClearGraph { .. } => QueryType::ClearGraph,
                    CatalogStatement::TruncateGraph { .. } => QueryType::TruncateGraph,
                }
            }
            crate::ast::Statement::DataStatement(data) => {
                use crate::ast::DataStatement;
                match data {
                    DataStatement::Insert { .. } => QueryType::Insert,
                    DataStatement::Set { .. } => QueryType::Set,
                    DataStatement::Remove { .. } => QueryType::Remove,
                    DataStatement::Delete { .. } => QueryType::Delete,
                    DataStatement::MatchInsert { .. } => QueryType::MatchInsert,
                    DataStatement::MatchSet { .. } => QueryType::MatchSet,
                    DataStatement::MatchRemove { .. } => QueryType::MatchRemove,
                    DataStatement::MatchDelete { .. } => QueryType::MatchDelete,
                }
            }
            crate::ast::Statement::SessionStatement(session) => {
                use crate::ast::{SessionSetClause, SessionStatement};
                match session {
                    SessionStatement::Set(set_stmt) => match &set_stmt.clause {
                        SessionSetClause::Graph { .. } => QueryType::SessionSetGraph,
                        SessionSetClause::Schema { .. } => QueryType::SessionSetSchema,
                        _ => QueryType::SessionSet,
                    },
                    SessionStatement::Reset(_) => QueryType::SessionReset,
                    SessionStatement::Close(_) => QueryType::SessionClose,
                }
            }
            crate::ast::Statement::TransactionStatement(txn) => {
                use crate::ast::TransactionStatement;
                match txn {
                    TransactionStatement::StartTransaction(_) => QueryType::StartTransaction,
                    TransactionStatement::Commit(_) => QueryType::Commit,
                    TransactionStatement::Rollback(_) => QueryType::Rollback,
                    TransactionStatement::SetTransactionCharacteristics(_) => {
                        QueryType::SetTransactionCharacteristics
                    }
                }
            }
            crate::ast::Statement::IndexStatement(idx) => {
                use crate::ast::IndexStatement;
                match idx {
                    IndexStatement::Create { .. } => QueryType::CreateIndex,
                    IndexStatement::Drop { .. } => QueryType::DropIndex,
                    IndexStatement::Alter(_) => QueryType::AlterIndex,
                    IndexStatement::Optimize(_) => QueryType::OptimizeIndex,
                    IndexStatement::Reindex(_) => QueryType::ReindexIndex,
                }
            }
            crate::ast::Statement::Declare(_) => QueryType::Declare,
            crate::ast::Statement::Let(_) => QueryType::Let,
            crate::ast::Statement::Next(_) => QueryType::Next,
            crate::ast::Statement::AtLocation(_) => QueryType::AtLocation,
            crate::ast::Statement::ProcedureBody(_) => QueryType::ProcedureBody,
        };

        // Determine if query is read-only
        let is_read_only = matches!(
            query_type,
            QueryType::Match
                | QueryType::Select
                | QueryType::Call
                | QueryType::SessionSetGraph
                | QueryType::SessionSetSchema
                | QueryType::SessionReset
        );

        Ok(QueryInfo {
            query_type,
            is_read_only,
        })
    }

    /// Explain the query execution plan without executing the query
    ///
    /// This generates a detailed query plan showing how the query will be executed,
    /// including optimization steps, cost estimates, and operator tree.
    ///
    /// # Arguments
    /// * `query` - The GQL query string to explain
    ///
    /// # Returns
    /// * `Ok(QueryPlan)` - Detailed query execution plan
    /// * `Err(String)` - Error if query cannot be planned
    ///
    /// # Example
    /// ```no_run
    /// # use graphlite::QueryCoordinator;
    /// # let coordinator = QueryCoordinator::from_path("./mydb", graphlite::StorageType::Sled).unwrap();
    /// let plan = coordinator.explain_query("MATCH (n:Person) RETURN n.name")
    ///     .expect("Failed to explain query");
    ///
    /// println!("Query Plan:\n{}", plan.format_tree());
    /// println!("Estimated cost: {}", plan.estimated_cost);
    /// ```
    pub fn explain_query(&self, query: &str) -> Result<QueryPlan, String> {
        // Parse the query
        let document = parse_query(query).map_err(|e| format!("Parse error: {:?}", e))?;

        // Only MATCH/SELECT queries can be explained (not DDL/DML)
        match &document.statement {
            crate::ast::Statement::Query(_) | crate::ast::Statement::Select(_) => {
                // Good - these can be explained
            }
            _ => {
                return Err("EXPLAIN is only supported for MATCH and SELECT queries".to_string());
            }
        }

        // Create a query planner
        let mut planner = crate::plan::optimizer::QueryPlanner::new();

        // Plan the query with tracing
        let trace = planner
            .plan_query_with_trace(&document)
            .map_err(|e| format!("Planning error: {:?}", e))?;

        // Use the cost and row estimates from the physical plan
        let estimated_cost = trace.physical_plan.estimated_cost;
        let estimated_rows = trace.physical_plan.estimated_rows;

        Ok(QueryPlan {
            logical_plan: trace.logical_plan,
            physical_plan: trace.physical_plan,
            planning_steps: trace.steps,
            total_planning_time_ms: trace.total_duration.as_millis() as u64,
            estimated_cost,
            estimated_rows,
        })
    }
}

/// Query execution plan information
#[derive(Debug, Clone)]
pub struct QueryPlan {
    /// Logical plan (high-level query structure)
    pub logical_plan: crate::plan::logical::LogicalPlan,
    /// Physical plan (actual execution operators)
    pub physical_plan: crate::plan::physical::PhysicalPlan,
    /// Planning steps and optimizations applied
    pub planning_steps: Vec<crate::plan::trace::TraceStep>,
    /// Total time spent planning (milliseconds)
    pub total_planning_time_ms: u64,
    /// Estimated total cost
    pub estimated_cost: f64,
    /// Estimated number of rows returned
    pub estimated_rows: usize,
}

impl QueryPlan {
    /// Format the physical plan as a tree for display
    pub fn format_tree(&self) -> String {
        format!("{:#?}", self.physical_plan.root)
    }

    /// Get a summary of the plan
    pub fn summary(&self) -> String {
        format!(
            "Planning time: {}ms | Estimated cost: {:.2} | Estimated rows: {}",
            self.total_planning_time_ms, self.estimated_cost, self.estimated_rows
        )
    }
}

/// Information about a parsed query
#[derive(Debug, Clone)]
pub struct QueryInfo {
    /// The type of query operation
    pub query_type: QueryType,
    /// Whether the query only reads data (no modifications)
    pub is_read_only: bool,
}

/// Types of query operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    // Read operations
    Match,
    Select,
    Call,

    // DDL operations
    CreateSchema,
    DropSchema,
    CreateGraph,
    DropGraph,
    CreateGraphType,
    DropGraphType,
    AlterGraphType,
    CreateProcedure,
    DropProcedure,
    CreateUser,
    DropUser,
    CreateRole,
    DropRole,
    GrantRole,
    RevokeRole,
    ClearGraph,
    TruncateGraph,
    CreateIndex,
    DropIndex,
    AlterIndex,
    OptimizeIndex,
    ReindexIndex,

    // DML operations
    Insert,
    Set,
    Remove,
    Delete,
    MatchInsert,
    MatchSet,
    MatchRemove,
    MatchDelete,

    // Session operations
    SessionSet,
    SessionSetGraph,
    SessionSetSchema,
    SessionReset,
    SessionClose,

    // Transaction operations
    StartTransaction,
    Commit,
    Rollback,
    SetTransactionCharacteristics,

    // Other operations
    Declare,
    Let,
    Next,
    AtLocation,
    ProcedureBody,
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_coordinator_creation() {
        // This is a basic structure test
        // Real tests should use TestFixture
    }
}
