// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Execution context for variable management and session lookup

use crate::functions::FunctionRegistry;
use crate::session::models::{Session, UserSession};
use crate::session::SessionProvider;
use crate::storage::{StorageManager, Value};
use crate::types::GqlType;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct ExecutionContext {
    /// Session ID for global session lookup
    pub session_id: String,
    /// Session provider for session management
    pub session_provider: Option<Arc<dyn SessionProvider>>,
    /// Local query variables
    pub variables: HashMap<String, Value>,
    /// Type information for variables
    pub variable_types: HashMap<String, GqlType>,
    /// Schema type information for schema-aware type checking (planned feature)
    #[allow(dead_code)]
    pub schema_types: HashMap<String, GqlType>,
    /// Current graph for execution (explicit parameter)
    pub current_graph: Option<std::sync::Arc<crate::storage::GraphCache>>,
    /// Storage manager for data operations and rollback
    pub storage_manager: Option<Arc<StorageManager>>,
    /// Function registry for executing functions
    pub function_registry: Option<Arc<FunctionRegistry>>,
    /// Current user for metadata tracking
    pub current_user: Option<String>,
    /// Current transaction ID for transaction metadata tracking (planned feature)
    #[allow(dead_code)]
    pub current_transaction: Option<String>,
    /// Warnings generated during execution (e.g., duplicate insert detection)
    pub warnings: Vec<String>,
}

// Manual Debug implementation to avoid SessionProvider Debug requirement
impl std::fmt::Debug for ExecutionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionContext")
            .field("session_id", &self.session_id)
            .field("session_provider", &self.session_provider.as_ref().map(|_| "Some(SessionProvider)"))
            .field("variables", &self.variables)
            .field("variable_types", &self.variable_types)
            .field("schema_types", &self.schema_types)
            .field("current_graph", &self.current_graph)
            .field("storage_manager", &self.storage_manager)
            .field("function_registry", &self.function_registry)
            .field("current_user", &self.current_user)
            .field("current_transaction", &self.current_transaction)
            .field("warnings", &self.warnings)
            .finish()
    }
}

impl ExecutionContext {
    /// Create a new execution context with a session ID
    pub fn new(session_id: String, storage_manager: Arc<StorageManager>) -> Self {
        Self {
            session_id,
            session_provider: None,
            variables: HashMap::new(),
            variable_types: HashMap::new(),
            schema_types: HashMap::new(),
            current_graph: None,
            storage_manager: Some(storage_manager),
            function_registry: None,
            current_user: None,
            current_transaction: None,
            warnings: Vec::new(),
        }
    }

    /// Set the session provider
    pub fn with_session_provider(mut self, session_provider: Arc<dyn SessionProvider>) -> Self {
        self.session_provider = Some(session_provider);
        self
    }

    /// Add a warning to the execution context
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Get all warnings from the execution context
    pub fn get_warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Clear all warnings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Warning management for query execution diagnostics
    pub fn clear_warnings(&mut self) {
        self.warnings.clear();
    }

    /// Set the function registry
    pub fn with_function_registry(mut self, function_registry: Arc<FunctionRegistry>) -> Self {
        self.function_registry = Some(function_registry);
        self
    }

    /// Get the user session from session provider
    pub fn get_session(&self) -> Option<Arc<std::sync::RwLock<UserSession>>> {
        self.session_provider.as_ref()?.get_session(&self.session_id)
    }

    /// Get a variable value, checking session parameters first, then local variables
    pub fn get_variable(&self, name: &str) -> Option<Value> {
        // First check session parameters
        if let Some(session_arc) = self.get_session() {
            if let Ok(user_session) = session_arc.read() {
                if let Some(value) = user_session.parameters.get(name) {
                    return Some(value.clone());
                }
            }
        }
        // Check local variables
        self.variables.get(name).cloned()
    }

    /// Set a local variable with type information
    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Set a local variable with explicit type information
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type-aware variable binding for schema validation
    pub fn set_variable_with_type(&mut self, name: String, value: Value, value_type: GqlType) {
        self.variables.insert(name.clone(), value);
        self.variable_types.insert(name, value_type);
    }

    /// Get the type of a variable
    #[allow(dead_code)] // ROADMAP v0.4.0 - Variable type inspection for type checking
    pub fn get_variable_type(&self, name: &str) -> Option<&GqlType> {
        self.variable_types.get(name)
    }

    /// Get the transaction state from the user session
    pub fn transaction_state(&self) -> Option<Arc<crate::session::SessionTransactionState>> {
        let session_arc = self.get_session()?;
        let user_session = session_arc.read().ok()?;
        Some(user_session.transaction_state.clone())
    }

    /// Get current graph name from session
    pub fn get_current_graph_name(&self) -> Option<String> {
        let session_arc = self.get_session()?;
        let user_session = session_arc.read().ok()?;
        user_session.current_graph.clone()
    }

    /// Get current schema from session
    pub fn get_current_schema(&self) -> Option<String> {
        let session_arc = self.get_session()?;
        let user_session = session_arc.read().ok()?;
        user_session.current_schema.clone()
    }

    /// Set schema type information
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema-aware type tracking for validation
    pub fn set_schema_type(&mut self, name: String, schema_type: GqlType) {
        self.schema_types.insert(name, schema_type);
    }

    /// Get schema type information
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema type retrieval for validation
    pub fn get_schema_type(&self, name: &str) -> Option<&GqlType> {
        self.schema_types.get(name)
    }

    /// Clear local variables while preserving session state
    pub fn clear_locals(&mut self) {
        self.variables.clear();
        self.variable_types.clear();
    }

    /// Set the current graph for execution
    pub fn set_current_graph(&mut self, graph: std::sync::Arc<crate::storage::GraphCache>) {
        self.current_graph = Some(graph);
    }

    /// Get session ID
    #[allow(dead_code)] // Session ID accessor for session management and logging
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Log an operation to the WAL if there's an active transaction
    pub fn log_operation_to_wal(
        &self,
        operation_type: crate::txn::state::OperationType,
        description: String,
    ) -> Result<(), crate::exec::error::ExecutionError> {
        if let Some(transaction_state) = self.transaction_state() {
            transaction_state.log_operation_to_wal(operation_type, description)
        } else {
            // No transaction state - skip WAL logging (common in unit tests)
            log::debug!(
                "Skipping WAL logging - no transaction state available: {:?} - {}",
                operation_type,
                description
            );
            Ok(())
        }
    }

    /// Log a transaction operation (undo operation) for rollback
    pub fn log_transaction_operation(
        &self,
        operation: crate::txn::UndoOperation,
    ) -> Result<(), crate::exec::error::ExecutionError> {
        if let Some(transaction_state) = self.transaction_state() {
            transaction_state.log_transaction_operation(operation)
        } else {
            Err(crate::exec::error::ExecutionError::RuntimeError(
                "No transaction state available for transaction logging".to_string(),
            ))
        }
    }

    /// Get the graph name for operations using session-aware resolution
    /// Returns full path in /<schema-name>/<graph-name> format
    pub fn get_graph_name(&self) -> Result<String, crate::exec::error::ExecutionError> {
        // Check session current graph
        if let Some(graph_name) = self.get_current_graph_name() {
            // Validate that the graph name is in full path format
            if graph_name.starts_with('/') && graph_name.matches('/').count() >= 2 {
                Ok(graph_name)
            } else {
                Err(crate::exec::error::ExecutionError::RuntimeError(
                    format!("Session graph name '{}' is not in full path format. Use /<schema-name>/<graph-name> format.", graph_name)
                ))
            }
        } else {
            Err(crate::exec::error::ExecutionError::RuntimeError(
                "No graph context available. Use SESSION SET GRAPH with full path /<schema-name>/<graph-name> format.".to_string()
            ))
        }
    }

    /// Create a MetadataTracker from this execution context
    /// Create ExecutionContext from a simplified Session
    #[allow(dead_code)] // ROADMAP v0.5.0 - Session-to-context conversion for test utilities
    pub fn from_session(session: &Session) -> Self {
        let storage = session.storage.clone();
        let mut context = Self::new("session".to_string(), storage);
        context.current_user = Some(session.username.clone());
        context
    }

    /// Set the current user for metadata tracking
    #[allow(dead_code)] // ROADMAP v0.5.0 - User tracking for audit and metadata
    pub fn set_current_user(&mut self, user: String) {
        self.current_user = Some(user);
    }

    /// Set the current transaction for metadata tracking
    #[allow(dead_code)] // ROADMAP v0.5.0 - Transaction tracking for metadata and audit
    pub fn set_current_transaction(&mut self, transaction_id: String) {
        self.current_transaction = Some(transaction_id);
    }

    /// Evaluate a simple expression (literals and function calls) for INSERT/SET operations
    /// This is a lightweight evaluator for property expressions that don't require full row context
    pub fn evaluate_simple_expression(
        &self,
        expr: &crate::ast::Expression,
    ) -> Result<Value, crate::exec::error::ExecutionError> {
        use crate::ast::Expression;
        use crate::exec::result::Row;
        use crate::functions::FunctionContext;

        match expr {
            Expression::Literal(literal) => {
                // Convert literal to value
                Ok(Self::literal_to_value(literal))
            }

            Expression::FunctionCall(func_call) => {
                // Get the function registry
                let function_registry = self.function_registry.as_ref().ok_or_else(|| {
                    crate::exec::error::ExecutionError::RuntimeError(
                        "Function registry not available in execution context".to_string(),
                    )
                })?;

                // Get the function from registry
                let function = function_registry.get(&func_call.name).ok_or_else(|| {
                    crate::exec::error::ExecutionError::UnsupportedOperator(format!(
                        "Function not found: {}",
                        func_call.name
                    ))
                })?;

                // Evaluate arguments recursively
                let mut evaluated_args = Vec::new();
                for arg in &func_call.arguments {
                    let value = self.evaluate_simple_expression(arg)?;
                    evaluated_args.push(value);
                }

                // Create function context with empty row set (no row context needed for simple expressions)
                let temp_row = Row::new();
                let function_context = FunctionContext::with_storage(
                    vec![temp_row],
                    self.variables.clone(),
                    evaluated_args,
                    self.storage_manager.clone(),
                    self.current_graph.clone(),
                    self.get_current_graph_name(),
                );

                // Execute the function
                function.execute(&function_context).map_err(|e| {
                    crate::exec::error::ExecutionError::UnsupportedOperator(format!(
                        "Function execution error: {}",
                        e
                    ))
                })
            }

            Expression::Variable(var) => {
                // Try to get variable from context
                self.get_variable(&var.name).ok_or_else(|| {
                    crate::exec::error::ExecutionError::ExpressionError(format!(
                        "Variable not found: {}",
                        var.name
                    ))
                })
            }

            _ => {
                // For other expression types, return an error
                Err(crate::exec::error::ExecutionError::ExpressionError(
                    format!(
                        "Expression type not supported in simple evaluation: {:?}",
                        expr
                    ),
                ))
            }
        }
    }

    /// Convert AST literal to storage value
    fn literal_to_value(literal: &crate::ast::Literal) -> Value {
        match literal {
            crate::ast::Literal::String(s) => Value::String(s.clone()),
            crate::ast::Literal::Integer(i) => Value::Number(*i as f64),
            crate::ast::Literal::Float(f) => Value::Number(*f),
            crate::ast::Literal::Boolean(b) => Value::Boolean(*b),
            crate::ast::Literal::Null => Value::Null,
            crate::ast::Literal::DateTime(dt) => Value::String(dt.clone()),
            crate::ast::Literal::Duration(dur) => Value::String(dur.clone()),
            crate::ast::Literal::TimeWindow(tw) => Value::String(tw.clone()),
            crate::ast::Literal::Vector(vec) => {
                Value::Vector(vec.iter().map(|&f| f as f32).collect())
            }
            crate::ast::Literal::List(list) => {
                let converted: Vec<Value> = list.iter().map(Self::literal_to_value).collect();
                Value::List(converted)
            }
        }
    }
}
