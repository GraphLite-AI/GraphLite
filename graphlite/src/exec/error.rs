// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Execution error types

use crate::storage::StorageError;
use thiserror::Error;

/// Execution errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Expression evaluation error: {0}")]
    ExpressionError(String),

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),

    #[error("Unsupported operator: {0}")]
    UnsupportedOperator(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Catalog error: {0}")]
    CatalogError(String),

    #[error("Syntax error: {0}")]
    SyntaxError(String),

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Schema validation error: {0}")]
    SchemaValidation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Memory limit exceeded: requested {requested} bytes, limit {limit} bytes")]
    MemoryLimitExceeded { limit: usize, requested: usize },
}

impl From<StorageError> for ExecutionError {
    fn from(error: StorageError) -> Self {
        ExecutionError::StorageError(error.to_string())
    }
}
