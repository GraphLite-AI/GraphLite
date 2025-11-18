// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Schema module - ISO GQL Graph Type Definitions and Validation
//
// This module implements persistent graph schema support per ISO GQL standards,
// including CREATE GRAPH TYPE and schema validation.

pub mod types;
pub mod validator;
pub mod catalog;
pub mod parser;
pub mod executor;
pub mod enforcement;
pub mod integration;
pub mod introspection;

// Re-export commonly used types - commented out until needed
// pub use types::{
//     GraphTypeDefinition,
//     NodeTypeDefinition,
//     EdgeTypeDefinition,
//     PropertyDefinition,
//     DataType,
//     Constraint,
//     GraphTypeVersion,
//     SchemaEnforcementMode,
// };

pub use validator::ValidationError;

// Schema module error type
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchemaError {
    #[error("Schema validation error: {0}")]
    ValidationError(#[from] ValidationError),

    #[error("Schema not found: {0}")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error variant for schema lookup failures in graph type catalog
    SchemaNotFound(String),

    #[error("Schema already exists: {0}")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error variant for CREATE GRAPH TYPE conflicts
    SchemaAlreadyExists(String),

    #[error("Invalid schema definition: {0}")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error variant for malformed graph type DDL
    InvalidDefinition(String),

    #[error("Version conflict: {0}")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error variant for graph type version conflicts during ALTER
    VersionConflict(String),

    #[error("Catalog error: {0}")]
    #[allow(dead_code)] // ROADMAP v0.4.0 - Error variant for graph type catalog operation failures
    CatalogError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

#[allow(dead_code)] // ROADMAP v0.4.0 - Result type for schema operations (CREATE/ALTER/DROP GRAPH TYPE)
pub type SchemaResult<T> = Result<T, SchemaError>;
