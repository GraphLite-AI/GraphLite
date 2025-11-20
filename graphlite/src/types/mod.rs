// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! ISO GQL-compliant Type System Operational Components
//!
//! This module provides operational components for the type system that work with
//! the existing TypeSpec defined in the AST. It includes type inference, validation,
//! coercion, and casting engines.

pub mod casting;
pub mod coercion;
pub mod inference;
pub mod validation;

use std::fmt;

pub use self::casting::TypeCaster;
pub use self::coercion::{CoercionStrategy, TypeCoercion};
pub use self::inference::TypeInferenceEngine as TypeInference;
pub use self::validation::TypeValidator;

// Re-export TypeSpec as the main type for the type system
pub use crate::ast::ast::{GraphTypeSpec, TypeSpec as GqlType};

/// Type error for type system operations
#[derive(Debug, Clone)]
pub enum TypeError {
    #[allow(dead_code)] // ROADMAP v0.5.0 - Type incompatibility error variant
    IncompatibleTypes(String, String),
    #[allow(dead_code)] // ISO GQL - Type casting operations
    InvalidCast(String, String),
    #[allow(dead_code)] // ISO GQL - Type validation and checking
    TypeMismatch {
        expected: String,
        actual: String,
    },
    InvalidTypeSpecification(String),
    #[allow(dead_code)] // ISO GQL - NOT NULL constraint violations
    NullabilityViolation(String),
    #[allow(dead_code)] // ISO GQL - LIST and SET type mismatches
    CollectionTypeMismatch(String),
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph schema validation errors for type enforcement
    GraphSchemaViolation(String),
    #[allow(dead_code)] // ISO GQL - Numeric overflow detection in type operations
    NumericOverflow(String),
    #[allow(dead_code)] // ISO GQL - Unknown type references in schema
    UnknownType(String),
}

impl fmt::Display for TypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::IncompatibleTypes(t1, t2) => {
                write!(f, "Incompatible types: {} and {}", t1, t2)
            }
            TypeError::InvalidCast(from, to) => {
                write!(f, "Cannot cast from {} to {}", from, to)
            }
            TypeError::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            TypeError::InvalidTypeSpecification(msg) => {
                write!(f, "Invalid type specification: {}", msg)
            }
            TypeError::NullabilityViolation(msg) => {
                write!(f, "Nullability violation: {}", msg)
            }
            TypeError::CollectionTypeMismatch(msg) => {
                write!(f, "Collection type mismatch: {}", msg)
            }
            TypeError::GraphSchemaViolation(msg) => {
                write!(f, "Graph schema violation: {}", msg)
            }
            TypeError::NumericOverflow(msg) => {
                write!(f, "Numeric overflow: {}", msg)
            }
            TypeError::UnknownType(name) => {
                write!(f, "Unknown type: {}", name)
            }
        }
    }
}

impl std::error::Error for TypeError {}

/// Result type for type system operations
pub type TypeResult<T> = Result<T, TypeError>;
