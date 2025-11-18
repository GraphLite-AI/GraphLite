// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Generic function trait for query execution
//!
//! This module defines the core Function trait that all functions must implement.
//! Functions can be anything - aggregate, scalar, or any other type.

use crate::storage::Value;
use crate::exec::result::Row;
use std::collections::HashMap;
use std::sync::Arc;

/// Error type for function execution
#[derive(Debug, thiserror::Error)]
pub enum FunctionError {
    #[error("Invalid argument count: expected {expected}, got {actual}")]
    InvalidArgumentCount { expected: usize, actual: usize },
    
    #[error("Invalid argument type: {message}")]
    InvalidArgumentType { message: String },
    
    #[error("Function execution failed: {message}")]
    ExecutionError { message: String },
    
    #[error("Unsupported operation: {operation}")]
    UnsupportedOperation { operation: String },
}

/// Result type for function execution
pub type FunctionResult<T> = Result<T, FunctionError>;

/// Function execution context
pub struct FunctionContext {
    /// Input rows for the function
    pub rows: Vec<Row>,
    /// Variables available in the current scope
    pub variables: HashMap<String, Value>,
    /// Function arguments
    pub arguments: Vec<Value>,
    /// Optional storage manager for graph operations
    pub storage_manager: Option<Arc<crate::storage::StorageManager>>,
    /// Optional current graph for execution
    pub current_graph: Option<Arc<crate::storage::GraphCache>>,
    /// Optional graph name
    pub graph_name: Option<String>,
}

impl FunctionContext {
    /// Create a new function context (backward compatibility)
    pub fn new(rows: Vec<Row>, variables: HashMap<String, Value>, arguments: Vec<Value>) -> Self {
        Self {
            rows,
            variables,
            arguments,
            storage_manager: None,
            current_graph: None,
            graph_name: None,
        }
    }
    
    /// Create a new function context with storage access
    pub fn with_storage(
        rows: Vec<Row>, 
        variables: HashMap<String, Value>, 
        arguments: Vec<Value>,
        storage_manager: Option<Arc<crate::storage::StorageManager>>,
        current_graph: Option<Arc<crate::storage::GraphCache>>,
        graph_name: Option<String>,
    ) -> Self {
        Self {
            rows,
            variables,
            arguments,
            storage_manager,
            current_graph,
            graph_name,
        }
    }
    
    /// Get a specific argument by index
    pub fn get_argument(&self, index: usize) -> FunctionResult<&Value> {
        self.arguments.get(index).ok_or_else(|| {
            FunctionError::InvalidArgumentCount {
                expected: index + 1,
                actual: self.arguments.len(),
            }
        })
    }
    
    /// Get the number of arguments
    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }
    
    /// Check if the function has the expected number of arguments
    pub fn validate_argument_count(&self, expected: usize) -> FunctionResult<()> {
        if self.argument_count() != expected {
            return Err(FunctionError::InvalidArgumentCount {
                expected,
                actual: self.argument_count(),
            });
        }
        Ok(())
    }
}

/// Core trait for all functions - just implement this for any function
pub trait Function: Send + Sync + std::fmt::Debug {
    /// Get the name of the function
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function metadata for introspection (see ROADMAP.md §8)
    fn name(&self) -> &str;

    /// Get the function description
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function metadata for introspection (see ROADMAP.md §8)
    fn description(&self) -> &str;

    /// Get the expected number of arguments
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function metadata for introspection (see ROADMAP.md §8)
    fn argument_count(&self) -> usize;

    /// Execute the function with the given context
    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value>;

    /// Get the return type of the function
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function metadata for introspection (see ROADMAP.md §8)
    fn return_type(&self) -> &str;

    /// Check if this function requires graph context to execute
    /// Most functions that work with variables, properties, or graph data require context.
    /// Pure scalar functions (math, string manipulation, timezone) do not.
    fn graph_context_required(&self) -> bool {
        true // Default to requiring context for safety
    }

    /// Check if this function accepts a variable number of arguments
    /// Returns true for functions like ALL_DIFFERENT that can take any number of args
    #[allow(dead_code)] // ROADMAP v0.5.0 - Function metadata for introspection (see ROADMAP.md §8)
    fn is_variadic(&self) -> bool {
        false // Default to fixed argument count
    }
} 
