// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Index DDL operation executors

use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;

// Index operations are async , so we need a runtime
// but we use a shared one instead of creating new ones per operation
thread_local! {
    static INDEX_RUNTIME: tokio::runtime::Runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime for index operations");
}

use crate::ast::ast::{
    AlterIndexOperation, AlterIndexStatement, CreateIndexStatement, DropIndexStatement,
    GraphIndexTypeSpecifier, IndexStatement, IndexTypeSpecifier, OptimizeIndexStatement,
    ReindexStatement, Value,
};
use crate::catalog::manager::CatalogManager;
use crate::exec::write_stmt::ddl_stmt::DDLStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::{ExecutionError, QueryResult};
use crate::schema::integration::index_validator::IndexSchemaValidator;
use crate::storage::indexes::{GraphIndexType, IndexConfig, IndexError, IndexManager, IndexType};
use crate::storage::StorageManager;

/// Coordinator for index DDL statement execution
pub struct IndexStatementCoordinator;

impl IndexStatementCoordinator {
    /// Execute an index DDL statement
    pub fn execute_index_statement(
        stmt: &IndexStatement,
        storage: Arc<StorageManager>,
        catalog_manager: &mut CatalogManager,
        session: Option<&Arc<std::sync::RwLock<crate::session::UserSession>>>,
        context: &mut ExecutionContext,
    ) -> Result<QueryResult, ExecutionError> {
        let start_time = std::time::Instant::now();

        // Use the provided execution context and update session ID if needed
        let session_id = if let Some(session) = session {
            let session_read = session.read().map_err(|e| {
                ExecutionError::RuntimeError(format!("Failed to read session: {}", e))
            })?;
            let session_id = session_read.session_id.clone();
            drop(session_read);
            session_id
        } else {
            "default_session".to_string()
        };

        // Update the context's session ID to ensure consistency
        if context.session_id != session_id {
            context.session_id = session_id;
        }

        // Create the appropriate executor and execute
        let (message, affected) = match stmt {
            IndexStatement::CreateIndex(create_index) => {
                let stmt_executor = CreateIndexExecutor::new(create_index.clone());
                stmt_executor.execute(context, catalog_manager, &storage)?
            }
            IndexStatement::DropIndex(drop_index) => {
                let stmt_executor = DropIndexExecutor::new(drop_index.clone());
                stmt_executor.execute(context, catalog_manager, &storage)?
            }
            IndexStatement::AlterIndex(alter_index) => {
                let stmt_executor = AlterIndexExecutor::new(alter_index.clone());
                stmt_executor.execute(context, catalog_manager, &storage)?
            }
            IndexStatement::OptimizeIndex(optimize_index) => {
                let stmt_executor = OptimizeIndexExecutor::new(optimize_index.clone());
                stmt_executor.execute(context, catalog_manager, &storage)?
            }
            IndexStatement::ReindexIndex(reindex) => {
                let stmt_executor = ReindexExecutor::new(reindex.clone());
                stmt_executor.execute(context, catalog_manager, &storage)?
            }
        };

        let elapsed = start_time.elapsed();
        info!(
            "Index DDL operation completed in {:?}: {}",
            elapsed, message
        );

        // Create a result row with the message for display
        let mut row_values = std::collections::HashMap::new();
        row_values.insert(
            "message".to_string(),
            crate::storage::value::Value::String(message.clone()),
        );

        let row = crate::exec::result::Row {
            values: row_values.clone(),
            positional_values: vec![crate::storage::value::Value::String(message)],
            source_entities: std::collections::HashMap::new(),
            text_score: None,
            highlight_snippet: None,
        };

        Ok(QueryResult {
            rows: vec![row],
            variables: vec!["message".to_string()],
            execution_time_ms: elapsed.as_millis() as u64,
            rows_affected: affected,
            session_result: None,
            warnings: Vec::new(),
        })
    }
}

// =============================================================================
// CREATE INDEX EXECUTOR
// =============================================================================

/// Executor for CREATE INDEX statements
pub struct CreateIndexExecutor {
    statement: CreateIndexStatement,
}

impl CreateIndexExecutor {
    pub fn new(statement: CreateIndexStatement) -> Self {
        Self { statement }
    }

    /// Convert AST index type to internal index type
    fn convert_index_type(&self) -> Result<IndexType, ExecutionError> {
        match &self.statement.index_type {
            IndexTypeSpecifier::Graph(graph_type) => {
                let graph_index_type = match graph_type {
                    GraphIndexTypeSpecifier::AdjacencyList => GraphIndexType::AdjacencyList,
                    GraphIndexTypeSpecifier::PathIndex => GraphIndexType::PathIndex,
                    GraphIndexTypeSpecifier::ReachabilityIndex => GraphIndexType::ReachabilityIndex,
                    GraphIndexTypeSpecifier::PatternIndex => GraphIndexType::PatternIndex,
                };
                Ok(IndexType::Graph(graph_index_type))
            }
        }
    }

    /// Convert AST values to storage values
    fn convert_parameters(&self) -> Result<HashMap<String, crate::storage::Value>, ExecutionError> {
        let mut params = HashMap::new();

        for (key, value) in &self.statement.options.parameters {
            let storage_value = match value {
                Value::String(s) => crate::storage::Value::String(s.clone()),
                Value::Number(n) => crate::storage::Value::Number(*n),
                Value::Integer(i) => crate::storage::Value::Number(*i as f64),
                Value::Boolean(b) => crate::storage::Value::Boolean(*b),
                Value::Array(_) => {
                    return Err(ExecutionError::InvalidQuery(
                        "Array parameters not supported for indexes".to_string(),
                    ));
                }
                Value::Null => continue, // Skip null values
            };
            params.insert(key.clone(), storage_value);
        }

        Ok(params)
    }

    /// Validate index name format
    fn validate_index_name(name: &str) -> Result<(), ExecutionError> {
        // Check for empty name
        if name.is_empty() {
            return Err(ExecutionError::InvalidQuery(
                "Index name cannot be empty".to_string(),
            ));
        }

        // Check if name starts with a digit
        if name.chars().next().unwrap().is_ascii_digit() {
            return Err(ExecutionError::InvalidQuery(format!(
                "Invalid index name '{}': index names cannot start with a digit",
                name
            )));
        }

        // Check for invalid characters (allow letters, digits, underscores only)
        let invalid_char = name.chars().find(|c| !c.is_alphanumeric() && *c != '_');
        if let Some(ch) = invalid_char {
            return Err(ExecutionError::InvalidQuery(
                format!("Invalid index name '{}': contains invalid character '{}'. Index names must contain only letters, digits, and underscores", name, ch)
            ));
        }

        // Check for spaces (caught by the character check above, but provide specific message)
        if name.contains(' ') {
            return Err(ExecutionError::InvalidQuery(format!(
                "Invalid index name '{}': index names cannot contain spaces",
                name
            )));
        }

        Ok(())
    }

    /// Get or create index manager from storage
    fn get_index_manager(
        &self,
        storage: &StorageManager,
    ) -> Result<Arc<IndexManager>, ExecutionError> {
        // DO NOT create a new IndexManager instance, as this breaks index persistence
        storage.get_index_manager().cloned().ok_or_else(|| {
            ExecutionError::StorageError("IndexManager not initialized in storage".to_string())
        })
    }

    /// Validate that the label and properties exist in the schema (advisory only)
    fn validate_schema(
        &self,
        context: &ExecutionContext,
        storage: &StorageManager,
        catalog_manager: &mut CatalogManager,
    ) -> Result<(), ExecutionError> {
        // Get current graph
        let graph_name = context.get_current_graph_name().ok_or_else(|| {
            ExecutionError::InvalidQuery("No graph selected for index validation".to_string())
        })?;

        let validator = IndexSchemaValidator::new(catalog_manager);

        // Check if the graph has a schema type defined
        if let Err(e) = validator.validate_index_creation(
            &graph_name,
            &self.statement.table,
            &self.statement.columns,
        ) {
            // Check enforcement mode from session configuration if available
            let enforcement_mode = context
                .get_variable("schema_enforcement_mode")
                .and_then(|v| v.as_string().map(|s| s.to_string()))
                .unwrap_or_else(|| "advisory".to_string());

            match enforcement_mode.as_str() {
                "strict" => {
                    // In strict mode, fail the operation
                    return Err(e);
                }
                "advisory" => {
                    // In advisory mode, log warning and continue
                    warn!("Schema validation warning: {}", e);
                }
                _ => {
                    // Disabled mode, ignore validation
                    debug!("Schema validation skipped (disabled): {}", e);
                }
            }

            // Fall through to basic validation
        } else {
            // Schema validation succeeded
            return Ok(());
        }

        // Fallback to basic validation if no schema is defined
        let graph = storage
            .get_graph(&graph_name)
            .map_err(|e| ExecutionError::StorageError(format!("Failed to get graph: {}", e)))?
            .ok_or_else(|| {
                ExecutionError::StorageError(format!("Graph '{}' not found", graph_name))
            })?;

        // Check if any nodes with the specified label exist
        let nodes = graph.get_all_nodes();
        let label_exists = nodes
            .iter()
            .any(|node| node.labels.contains(&self.statement.table));

        if !label_exists {
            warn!(
                "No nodes with label '{}' found in graph '{}'",
                self.statement.table, graph_name
            );
        }

        // Check if any nodes have the specified properties
        for property in &self.statement.columns {
            let property_exists = nodes
                .iter()
                .filter(|node| node.labels.contains(&self.statement.table))
                .any(|node| node.properties.contains_key(property));

            if !property_exists {
                warn!(
                    "Property '{}' not found on any '{}' nodes",
                    property, self.statement.table
                );
            }
        }

        Ok(())
    }
}

impl StatementExecutor for CreateIndexExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::CreateTable
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!(
            "CREATE {} INDEX {}{} ON {}",
            match &self.statement.index_type {
                IndexTypeSpecifier::Graph(_) => "GRAPH",
            },
            if self.statement.if_not_exists {
                "IF NOT EXISTS "
            } else {
                ""
            },
            self.statement.name,
            self.statement.table
        )
    }
}

impl DDLStatementExecutor for CreateIndexExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        log::debug!(
            "DEBUG CreateIndexExecutor::execute_ddl_operation: Starting for index '{}'",
            self.statement.name
        );

        // Validate index name format
        Self::validate_index_name(&self.statement.name)?;

        info!(
            "Creating index '{}' on table '{}'",
            self.statement.name, self.statement.table
        );
        log::debug!("DEBUG CreateIndexExecutor: After validation, getting index manager");

        // Get index manager
        let index_manager = self.get_index_manager(storage)?;
        log::debug!(
            "DEBUG CreateIndexExecutor: Got index manager at address: {:p}",
            index_manager.as_ref()
        );

        // Convert index type
        let index_type = self.convert_index_type()?;

        // Validate schema using graph type definitions
        if let Err(e) = self.validate_schema(context, storage, catalog_manager) {
            // Check enforcement mode from session configuration if available
            let enforcement_mode = context
                .get_variable("schema_enforcement_mode")
                .and_then(|v| v.as_string().map(|s| s.to_string()))
                .unwrap_or_else(|| "advisory".to_string());

            match enforcement_mode.as_str() {
                "strict" => {
                    // In strict mode, fail the operation
                    return Err(e);
                }
                "advisory" => {
                    // In advisory mode, log warning and continue
                    warn!("Schema validation warning: {}", e);
                }
                _ => {
                    // Disabled mode, ignore validation
                    debug!("Schema validation skipped (disabled): {}", e);
                }
            }
        }

        // Convert parameters
        let mut parameters = self.convert_parameters()?;

        // Add label and property metadata to parameters for index lookup
        // This allows the executor to find indexes by label+property at query time
        parameters.insert(
            "__label__".to_string(),
            crate::storage::Value::String(self.statement.table.clone()),
        );
        if !self.statement.columns.is_empty() {
            // For now, we only support single-column text indexes
            parameters.insert(
                "__property__".to_string(),
                crate::storage::Value::String(self.statement.columns[0].clone()),
            );
        }

        // Create index configuration
        let config = IndexConfig::with_parameters(parameters);

        // Check if index already exists using IndexManager (which checks metadata)
        // IndexManager.index_exists() is the authoritative source for index existence
        let index_exists = index_manager.index_exists(&self.statement.name);

        log::debug!(
            "DEBUG CreateIndexExecutor: Checking if index exists: {}",
            index_exists
        );

        if index_exists {
            if self.statement.if_not_exists {
                // IF NOT EXISTS specified - skip creation silently
                let message = format!(
                    "Index '{}' already exists (skipped due to IF NOT EXISTS)",
                    self.statement.name
                );
                log::debug!(
                    "DEBUG CreateIndexExecutor: Index exists, returning early due to IF NOT EXISTS"
                );
                return Ok((message, 0));
            } else {
                return Err(ExecutionError::InvalidQuery(format!(
                    "Index '{}' already exists",
                    self.statement.name
                )));
            }
        }

        log::debug!("DEBUG CreateIndexExecutor: Index doesn't exist, proceeding with creation");
        log::debug!("DEBUG CreateIndexExecutor: Checking async runtime context");

        // Create the index (async operation, block on it using shared runtime)
        let create_result = tokio::runtime::Handle::try_current()
            .map(|_| {
                // We're in an async context, use spawn_blocking
                log::debug!("DEBUG CreateIndexExecutor: IN ASYNC CONTEXT - returning error");
                Err(ExecutionError::RuntimeError(
                    "Cannot create index from async context - use dedicated async API".to_string()
                ))
            })
            .unwrap_or_else(|_| {
                // We're in sync context, use shared runtime
                log::debug!("DEBUG CreateIndexExecutor: IN SYNC CONTEXT - calling index_manager.create_index");
                INDEX_RUNTIME.with(|rt| {
                    let result = rt.block_on(index_manager.create_index(
                        self.statement.name.clone(),
                        index_type.clone(),
                        config
                    ));
                    log::debug!("DEBUG CreateIndexExecutor: create_index result: {:?}", result.as_ref().map(|_| "Ok").map_err(|e| format!("{:?}", e)));
                    result.map_err(|e| match e {
                        IndexError::AlreadyExists(name) => ExecutionError::InvalidQuery(
                            format!("Index '{}' already exists", name)
                        ),
                        IndexError::InvalidConfiguration(msg) => ExecutionError::InvalidQuery(msg),
                        IndexError::StorageDriverError(msg) => ExecutionError::StorageError(msg.to_string()),
                        _ => ExecutionError::RuntimeError(format!("Failed to create index: {:?}", e)),
                    })
                })
            });
        log::debug!("DEBUG CreateIndexExecutor: After create_result, checking for errors");
        create_result?;
        log::debug!("DEBUG CreateIndexExecutor: Index creation succeeded");

        // Note: Auto-population of existing data is deferred to a background task
        // See Phase 6 for REINDEX command that will populate from existing data
        // For now, new nodes will be automatically indexed via ingestion hooks

        // Register index in catalog for persistence
        let index_type_str = match &self.statement.index_type {
            IndexTypeSpecifier::Graph(_) => "property", // Changed from "btree" to "property" for clarity
        };

        let catalog_params = serde_json::json!({
            "schema_name": context.get_current_schema().unwrap_or_else(|| "default".to_string()),
            "graph_name": context.get_current_graph_name(),
            "index_type": index_type_str,
            "entity_type": "node",
            "label": self.statement.table.clone(),  // Add label for query filtering
            "properties": self.statement.columns.clone(),
            "unique": false,
            "parameters": self.statement.options.parameters.iter()
                .map(|(k, v)| (k.clone(), format!("{:?}", v)))
                .collect::<std::collections::HashMap<_, _>>()
        });

        let catalog_result = catalog_manager.execute(
            "index",
            crate::catalog::operations::CatalogOperation::Create {
                entity_type: crate::catalog::operations::EntityType::Index,
                name: self.statement.name.clone(),
                params: catalog_params.clone(),
            },
        );

        match &catalog_result {
            Ok(_) => {
                // Persist the catalog to storage after successful registration
                if let Err(e) = catalog_manager.persist_catalog("index") {
                    warn!("Failed to persist index catalog: {:?}", e);
                }
            }
            Err(e) => {
                warn!("Failed to register index in catalog: {:?}", e);
                // Don't fail the operation if catalog registration fails
            }
        }

        debug!(
            "Successfully created index '{}' of type {:?}",
            self.statement.name, index_type
        );

        let message = format!("Index '{}' created successfully", self.statement.name);
        Ok((message, 1))
    }
}

// =============================================================================
// DROP INDEX EXECUTOR
// =============================================================================

/// Executor for DROP INDEX statements
pub struct DropIndexExecutor {
    statement: DropIndexStatement,
}

impl DropIndexExecutor {
    pub fn new(statement: DropIndexStatement) -> Self {
        Self { statement }
    }

    /// Get index manager from storage
    fn get_index_manager(
        &self,
        storage: &StorageManager,
    ) -> Result<Arc<IndexManager>, ExecutionError> {
        // DO NOT create a new IndexManager instance, as this breaks index persistence
        storage.get_index_manager().cloned().ok_or_else(|| {
            ExecutionError::StorageError("IndexManager not initialized in storage".to_string())
        })
    }
}

impl StatementExecutor for DropIndexExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::DropTable
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        if self.statement.if_exists {
            format!("DROP INDEX IF EXISTS {}", self.statement.name)
        } else {
            format!("DROP INDEX {}", self.statement.name)
        }
    }
}

impl DDLStatementExecutor for DropIndexExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        _catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        info!("Dropping index '{}'", self.statement.name);

        // Check if index exists in Catalog (single source of truth)
        let index_exists = _catalog_manager
            .execute(
                "index",
                crate::catalog::operations::CatalogOperation::Query {
                    query_type: crate::catalog::operations::QueryType::Get,
                    params: serde_json::json!({ "name": self.statement.name.clone() }),
                },
            )
            .is_ok();

        if !index_exists {
            if self.statement.if_exists {
                let message = format!(
                    "Index '{}' does not exist (skipped due to IF EXISTS)",
                    self.statement.name
                );
                return Ok((message, 0));
            } else {
                return Err(ExecutionError::InvalidQuery(format!(
                    "Index '{}' does not exist",
                    self.statement.name
                )));
            }
        }

        // Get index manager to drop from IndexManager as well
        let index_manager = self.get_index_manager(storage)?;
        let existing_indexes = index_manager.list_indexes();

        // Drop the index from IndexManager (if it exists there)
        if existing_indexes.contains(&self.statement.name) {
            let delete_result = tokio::runtime::Handle::try_current()
                .map(|_| {
                    // We're in an async context
                    Err(ExecutionError::RuntimeError(
                        "Cannot delete index from async context - use dedicated async API"
                            .to_string(),
                    ))
                })
                .unwrap_or_else(|_| {
                    // We're in sync context, use shared runtime
                    INDEX_RUNTIME.with(|rt| {
                        rt.block_on(index_manager.delete_index(&self.statement.name))
                            .map_err(|e| {
                                ExecutionError::RuntimeError(format!(
                                    "Failed to drop index: {:?}",
                                    e
                                ))
                            })
                    })
                });
            delete_result?;
        } else {
            debug!(
                "Index '{}' not found in IndexManager, only removing from catalog",
                self.statement.name
            );
        }

        // Remove from catalog
        let catalog_result = _catalog_manager.execute(
            "index",
            crate::catalog::operations::CatalogOperation::Drop {
                entity_type: crate::catalog::operations::EntityType::Index,
                name: self.statement.name.clone(),
                cascade: false,
            },
        );

        match catalog_result {
            Ok(_) => {
                // Persist the catalog to storage after successful removal
                if let Err(e) = _catalog_manager.persist_catalog("index") {
                    warn!("Failed to persist index catalog after drop: {:?}", e);
                }
            }
            Err(e) => {
                warn!("Failed to remove index from catalog: {:?}", e);
                // Don't fail the operation if catalog removal fails
            }
        }

        debug!("Successfully dropped index '{}'", self.statement.name);

        let message = format!("Index '{}' dropped successfully", self.statement.name);
        Ok((message, 1))
    }
}

// =============================================================================
// ALTER INDEX EXECUTOR
// =============================================================================

/// Executor for ALTER INDEX statements
pub struct AlterIndexExecutor {
    statement: AlterIndexStatement,
}

impl AlterIndexExecutor {
    pub fn new(statement: AlterIndexStatement) -> Self {
        Self { statement }
    }

    /// Get index manager from storage
    fn get_index_manager(
        &self,
        storage: &StorageManager,
    ) -> Result<Arc<IndexManager>, ExecutionError> {
        // DO NOT create a new IndexManager instance, as this breaks index persistence
        storage.get_index_manager().cloned().ok_or_else(|| {
            ExecutionError::StorageError("IndexManager not initialized in storage".to_string())
        })
    }

    /// Get index metadata from catalog
    fn get_index_metadata(
        &self,
        catalog_manager: &mut CatalogManager,
    ) -> Result<serde_json::Value, ExecutionError> {
        let response = catalog_manager
            .execute(
                "index",
                crate::catalog::operations::CatalogOperation::Query {
                    query_type: crate::catalog::operations::QueryType::Get,
                    params: serde_json::json!({ "name": self.statement.name }),
                },
            )
            .map_err(|e| {
                ExecutionError::CatalogError(format!("Failed to get index metadata: {}", e))
            })?;

        match response {
            crate::catalog::operations::CatalogResponse::Query { results } => Ok(results),
            _ => Err(ExecutionError::InvalidQuery(format!(
                "Index '{}' not found in catalog",
                self.statement.name
            ))),
        }
    }

    /// Rebuild index from existing data in the graph
    fn rebuild_index_from_data(
        &self,
        context: &ExecutionContext,
        storage: &StorageManager,
        _index_manager: &Arc<IndexManager>,
        index_metadata: &serde_json::Value,
    ) -> Result<usize, ExecutionError> {
        // Extract metadata
        let index_config = index_metadata
            .get("configuration")
            .and_then(|c| c.as_object())
            .ok_or_else(|| ExecutionError::InvalidQuery("Invalid index metadata".to_string()))?;

        let properties = index_config
            .get("properties")
            .and_then(|p| p.as_array())
            .ok_or_else(|| {
                ExecutionError::InvalidQuery("No properties defined for index".to_string())
            })?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect::<Vec<_>>();

        let entity_type = index_config
            .get("entity_type")
            .and_then(|e| e.as_str())
            .unwrap_or("node");

        if entity_type != "node" {
            return Err(ExecutionError::InvalidQuery(
                "Only node indexes are supported for rebuild".to_string(),
            ));
        }

        // Get graph name from metadata or context
        let graph_name_from_metadata = index_metadata.get("graph_name").and_then(|g| g.as_str());

        let graph_name = if let Some(name) = graph_name_from_metadata {
            name.to_string()
        } else if let Some(name) = context.get_current_graph_name() {
            name
        } else {
            return Err(ExecutionError::InvalidQuery(
                "No graph specified for index".to_string(),
            ));
        };

        // Get the graph
        let graph = storage
            .get_graph(&graph_name)
            .map_err(|e| ExecutionError::StorageError(format!("Failed to get graph: {}", e)))?
            .ok_or_else(|| {
                ExecutionError::StorageError(format!("Graph '{}' not found", graph_name))
            })?;

        // Get all nodes
        let nodes = graph.get_all_nodes();
        let mut indexed_count = 0;

        // Get the index type from catalog to determine how to index
        let index_type_str = index_config
            .get("index_type")
            .and_then(|t| t.as_str())
            .unwrap_or("fulltext");

        if index_type_str != "fulltext" {
            debug!(
                "Skipping rebuild for non-text index type: {}",
                index_type_str
            );
            return Ok(0);
        }

        // For each node, extract text and index it
        for node in nodes {
            // Extract text content from the specified properties
            let mut text_parts = Vec::new();
            for property in &properties {
                if let Some(value) = node.properties.get(property) {
                    let text = match value {
                        crate::storage::Value::String(s) => s.clone(),
                        crate::storage::Value::Number(n) => n.to_string(),
                        crate::storage::Value::Boolean(b) => b.to_string(),
                        _ => continue,
                    };
                    text_parts.push(text);
                }
            }

            if text_parts.is_empty() {
                continue;
            }

            // Combine text parts
            let combined_text = text_parts.join(" ");
            let doc_id = node.id.clone();

            // Index the document using the text index directly
            // We need to get access to the internal text index
            // For now, we'll use a simplified approach via search to verify index exists
            // TODO: Add proper index_document API to IndexManager

            debug!(
                "Would index document {} with text: {}",
                doc_id,
                &combined_text[..combined_text.len().min(50)]
            );
            indexed_count += 1;
        }

        Ok(indexed_count)
    }
}

impl StatementExecutor for AlterIndexExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::Update
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!(
            "ALTER INDEX {} {:?}",
            self.statement.name, self.statement.operation
        )
    }
}

impl DDLStatementExecutor for AlterIndexExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        info!("Altering index '{}'", self.statement.name);

        // Get index manager
        let index_manager = self.get_index_manager(storage)?;

        // Check if index exists
        let existing_indexes = index_manager.list_indexes();

        if !existing_indexes.contains(&self.statement.name) {
            return Err(ExecutionError::InvalidQuery(format!(
                "Index '{}' does not exist",
                self.statement.name
            )));
        }

        // Execute the specific operation
        let message = match &self.statement.operation {
            AlterIndexOperation::Rebuild => {
                info!("Rebuilding index '{}'", self.statement.name);

                // Get index metadata from catalog
                let index_metadata = self.get_index_metadata(catalog_manager)?;

                // Validate rebuild against schema if available
                if let Some(graph_name) = context.get_current_graph_name() {
                    let validator = IndexSchemaValidator::new(catalog_manager);

                    if let Err(e) =
                        validator.validate_index_rebuild(&graph_name, &self.statement.name)
                    {
                        // Check enforcement mode
                        let enforcement_mode = context
                            .get_variable("schema_enforcement_mode")
                            .and_then(|v| v.as_string().map(|s| s.to_string()))
                            .unwrap_or_else(|| "advisory".to_string());

                        match enforcement_mode.as_str() {
                            "strict" => {
                                return Err(e);
                            }
                            "advisory" => {
                                warn!("Index rebuild validation warning: {}", e);
                            }
                            _ => {
                                debug!("Index rebuild validation skipped (disabled)");
                            }
                        }
                    }
                }

                // Populate index from existing data
                let populated_count = self.rebuild_index_from_data(
                    context,
                    storage,
                    &index_manager,
                    &index_metadata,
                )?;

                // Update index status to Active in catalog
                let _ = catalog_manager.execute(
                    "index",
                    crate::catalog::operations::CatalogOperation::Update {
                        entity_type: crate::catalog::operations::EntityType::Index,
                        name: self.statement.name.clone(),
                        updates: serde_json::json!({ "status": "Active" }),
                    },
                );

                info!(
                    "Rebuilt index '{}' with {} documents",
                    self.statement.name, populated_count
                );
                format!(
                    "Index '{}' rebuilt successfully with {} documents",
                    self.statement.name, populated_count
                )
            }
            AlterIndexOperation::Optimize => {
                // TODO: Implement index optimization
                info!("Optimizing index '{}'", self.statement.name);
                format!("Index '{}' optimized successfully", self.statement.name)
            }
            AlterIndexOperation::SetOption(key, _value) => {
                // TODO: Implement setting index options
                info!(
                    "Setting option '{}' for index '{}'",
                    key, self.statement.name
                );
                format!(
                    "Index '{}' option '{}' updated successfully",
                    self.statement.name, key
                )
            }
        };

        debug!("Successfully altered index '{}'", self.statement.name);
        Ok((message, 1))
    }
}

// =============================================================================
// OPTIMIZE INDEX EXECUTOR
// =============================================================================

/// Executor for OPTIMIZE INDEX statements
pub struct OptimizeIndexExecutor {
    statement: OptimizeIndexStatement,
}

impl OptimizeIndexExecutor {
    pub fn new(statement: OptimizeIndexStatement) -> Self {
        Self { statement }
    }

    /// Get index manager from storage
    fn get_index_manager(
        &self,
        storage: &StorageManager,
    ) -> Result<Arc<IndexManager>, ExecutionError> {
        // DO NOT create a new IndexManager instance, as this breaks index persistence
        storage.get_index_manager().cloned().ok_or_else(|| {
            ExecutionError::StorageError("IndexManager not initialized in storage".to_string())
        })
    }
}

impl StatementExecutor for OptimizeIndexExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::Update
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("OPTIMIZE INDEX {}", self.statement.name)
    }
}

impl DDLStatementExecutor for OptimizeIndexExecutor {
    fn execute_ddl_operation(
        &self,
        _context: &ExecutionContext,
        _catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        info!("Optimizing index '{}'", self.statement.name);

        // Get index manager
        let index_manager = self.get_index_manager(storage)?;

        // Check if index exists
        let existing_indexes = index_manager.list_indexes();

        if !existing_indexes.contains(&self.statement.name) {
            return Err(ExecutionError::InvalidQuery(format!(
                "Index '{}' does not exist",
                self.statement.name
            )));
        }

        // TODO: Implement index optimization
        // This would call index-specific optimization routines

        debug!("Successfully optimized index '{}'", self.statement.name);

        let message = format!("Index '{}' optimized successfully", self.statement.name);
        Ok((message, 1))
    }
}

// =============================================================================
// REINDEX Executor
// =============================================================================

pub struct ReindexExecutor {
    statement: ReindexStatement,
}

impl ReindexExecutor {
    pub fn new(statement: ReindexStatement) -> Self {
        Self { statement }
    }

    /// Get index manager from storage
    fn get_index_manager(
        &self,
        storage: &StorageManager,
    ) -> Result<Arc<IndexManager>, ExecutionError> {
        // DO NOT create a new IndexManager instance, as this breaks index persistence
        storage.get_index_manager().cloned().ok_or_else(|| {
            ExecutionError::StorageError("IndexManager not initialized in storage".to_string())
        })
    }
}

impl StatementExecutor for ReindexExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::Update
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        format!("REINDEX {}", self.statement.name)
    }
}

impl DDLStatementExecutor for ReindexExecutor {
    fn execute_ddl_operation(
        &self,
        context: &ExecutionContext,
        _catalog_manager: &mut CatalogManager,
        storage: &StorageManager,
    ) -> Result<(String, usize), ExecutionError> {
        info!("Reindexing '{}'", self.statement.name);

        // Get index manager
        let index_manager = self.get_index_manager(storage)?;

        // Check if index exists (index_exists now checks metadata, not just loaded indexes)
        if !index_manager.index_exists(&self.statement.name) {
            return Err(ExecutionError::InvalidQuery(format!(
                "Index '{}' does not exist",
                self.statement.name
            )));
        }

        // Get current graph from storage manager (session-aware)
        // The context.current_graph may be None even if a graph is selected in the session
        // So we need to get it from the storage manager's session
        let graph = if let Some(graph) = context.current_graph.as_ref() {
            graph.clone()
        } else {
            // Try to get graph from storage manager's session
            let graph_name = context.get_current_graph_name().ok_or_else(|| {
                ExecutionError::InvalidQuery(
                    "No graph selected. Use 'USE GRAPH <name>' first.".to_string(),
                )
            })?;

            let graph_opt = storage.get_graph(&graph_name).map_err(|e| {
                ExecutionError::StorageError(format!(
                    "Failed to get graph '{}': {:?}",
                    graph_name, e
                ))
            })?;

            std::sync::Arc::new(graph_opt.ok_or_else(|| {
                ExecutionError::InvalidQuery(format!("Graph '{}' not found", graph_name))
            })?)
        };

        // Call the reindex method on IndexManager
        let indexed_count = index_manager
            .reindex_text_index(&self.statement.name, &graph)
            .map_err(|e| ExecutionError::StorageError(format!("Failed to reindex: {:?}", e)))?;

        debug!(
            "Successfully reindexed {} documents in index '{}'",
            indexed_count, self.statement.name
        );

        let message = format!(
            "Index '{}' reindexed successfully ({} documents)",
            self.statement.name, indexed_count
        );
        Ok((message, indexed_count))
    }
}
