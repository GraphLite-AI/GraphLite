// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Core schema type definitions for ISO GQL Graph Types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a complete graph type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTypeDefinition {
    pub name: String,
    pub version: GraphTypeVersion,
    pub previous_version: Option<GraphTypeVersion>,
    pub node_types: Vec<NodeTypeDefinition>,
    pub edge_types: Vec<EdgeTypeDefinition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    pub description: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Version information for a graph type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphTypeVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
    pub build_metadata: Option<String>,
}

impl GraphTypeVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
            build_metadata: None,
        }
    }

    pub fn to_string(&self) -> String {
        let mut version = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if let Some(pre) = &self.pre_release {
            version.push_str(&format!("-{}", pre));
        }
        if let Some(build) = &self.build_metadata {
            version.push_str(&format!("+{}", build));
        }
        version
    }

    pub fn parse(version_str: &str) -> Result<Self, String> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid version format: {}", version_str));
        }

        let major = parts[0].parse::<u32>()
            .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
        let minor = parts[1].parse::<u32>()
            .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;

        // Handle patch with optional pre-release/build metadata
        let patch_parts: Vec<&str> = parts[2].split('-').collect();
        let patch = patch_parts[0].parse::<u32>()
            .map_err(|_| format!("Invalid patch version: {}", patch_parts[0]))?;

        let mut version = Self::new(major, minor, patch);

        if patch_parts.len() > 1 {
            let pre_build: Vec<&str> = patch_parts[1].split('+').collect();
            version.pre_release = Some(pre_build[0].to_string());
            if pre_build.len() > 1 {
                version.build_metadata = Some(pre_build[1].to_string());
            }
        }

        Ok(version)
    }

    /// Check if this version is compatible with another version
    #[allow(dead_code)] // ROADMAP v0.4.0 - Version compatibility checking for schema evolution
    pub fn is_compatible_with(&self, other: &GraphTypeVersion) -> bool {
        // Same major version = compatible
        // Different major version = potentially incompatible
        self.major == other.major
    }
}

/// Definition of a node type within a graph type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTypeDefinition {
    pub label: String,
    pub properties: Vec<PropertyDefinition>,
    pub constraints: Vec<Constraint>,
    pub description: Option<String>,
    pub is_abstract: bool,
    pub extends: Option<String>, // For inheritance
}

/// Definition of an edge type within a graph type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTypeDefinition {
    pub type_name: String,
    pub from_node_types: Vec<String>,
    pub to_node_types: Vec<String>,
    pub properties: Vec<PropertyDefinition>,
    pub constraints: Vec<Constraint>,
    pub description: Option<String>,
    pub cardinality: EdgeCardinality,
}

/// Cardinality constraints for edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeCardinality {
    pub from_min: Option<u32>,
    pub from_max: Option<u32>,
    pub to_min: Option<u32>,
    pub to_max: Option<u32>,
}

impl Default for EdgeCardinality {
    fn default() -> Self {
        Self {
            from_min: None,
            from_max: None,
            to_min: None,
            to_max: None,
        }
    }
}

/// Definition of a property within a node or edge type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub name: String,
    pub data_type: DataType,
    pub required: bool,
    pub unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub description: Option<String>,
    pub deprecated: bool,
    pub deprecation_message: Option<String>,
    pub validation_pattern: Option<String>, // Regular expression for validation
    pub constraints: Vec<Constraint>,        // Property-level constraints
}

/// Supported data types in graph schemas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    // Primitive types
    String,
    Integer,
    BigInt,
    Float,
    Double,
    Boolean,

    // Temporal types
    Date,
    Time,
    DateTime,
    Timestamp,
    Duration,

    // Special types
    UUID,
    Text,      // For full-text indexable content
    Json,      // JSON object
    Bytes,     // Binary data

    // Collection types
    Array(Box<DataType>),
    Map(Box<DataType>, Box<DataType>),
    Set(Box<DataType>),
    List(Box<DataType>),      // List of elements
    Vector(usize),            // Vector with dimension size

    // User-defined types
    Enum(Vec<String>),
    Reference(String), // Reference to another node type
}

impl DataType {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Type compatibility for schema validation
    pub fn is_compatible_with(&self, other: &DataType) -> bool {
        match (self, other) {
            (DataType::String, DataType::Text) | (DataType::Text, DataType::String) => true,
            (DataType::Integer, DataType::BigInt) => true, // Integer can be promoted to BigInt
            (DataType::Float, DataType::Double) => true,   // Float can be promoted to Double
            (a, b) => a == b,
        }
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - SQL type mapping for storage layer integration
    pub fn to_sql_type(&self) -> String {
        match self {
            DataType::String => "VARCHAR".to_string(),
            DataType::Integer => "INTEGER".to_string(),
            DataType::BigInt => "BIGINT".to_string(),
            DataType::Float => "REAL".to_string(),
            DataType::Double => "DOUBLE PRECISION".to_string(),
            DataType::Boolean => "BOOLEAN".to_string(),
            DataType::Date => "DATE".to_string(),
            DataType::Time => "TIME".to_string(),
            DataType::DateTime => "TIMESTAMP".to_string(),
            DataType::Timestamp => "TIMESTAMP WITH TIME ZONE".to_string(),
            DataType::Duration => "INTERVAL".to_string(),
            DataType::UUID => "UUID".to_string(),
            DataType::Text => "TEXT".to_string(),
            DataType::Json => "JSONB".to_string(),
            DataType::Bytes => "BYTEA".to_string(),
            DataType::Array(_) => "JSONB".to_string(), // Arrays stored as JSON
            DataType::Map(_, _) => "JSONB".to_string(),
            DataType::Set(_) => "JSONB".to_string(),
            DataType::List(_) => "JSONB".to_string(),  // Lists stored as JSON
            DataType::Vector(dim) => format!("VECTOR({})", dim), // Vector with dimension
            DataType::Enum(_) => "VARCHAR".to_string(),
            DataType::Reference(_) => "UUID".to_string(),
        }
    }
}

/// Constraints that can be applied to properties or types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Constraint {
    NotNull,
    Unique,
    PrimaryKey,
    ForeignKey { references: String, on_delete: ForeignKeyAction },
    Check { expression: String },
    MinLength(usize),
    MaxLength(usize),
    MinValue(f64),
    MaxValue(f64),
    Pattern(String), // Regular expression
    In(Vec<serde_json::Value>), // Value must be in this list
}

/// Foreign key actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForeignKeyAction {
    Cascade,
    SetNull,
    SetDefault,
    Restrict,
    NoAction,
}

/// Schema enforcement modes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SchemaEnforcementMode {
    /// Block operations that violate schema
    Strict,
    /// Warn but allow violations
    Advisory,
    /// No schema validation
    Disabled,
}

impl Default for SchemaEnforcementMode {
    fn default() -> Self {
        SchemaEnforcementMode::Advisory
    }
}

/// Schema change for ALTER operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SchemaChange {
    AddNodeType(NodeTypeDefinition),
    DropNodeType(String),
    AddEdgeType(EdgeTypeDefinition),
    DropEdgeType(String),
    AddProperty {
        type_name: String,
        is_node: bool,
        property: PropertyDefinition,
    },
    DropProperty {
        type_name: String,
        is_node: bool,
        property_name: String,
    },
    AlterProperty {
        type_name: String,
        is_node: bool,
        property_name: String,
        changes: PropertyChanges,
    },
    AddConstraint {
        type_name: String,
        is_node: bool,
        constraint: Constraint,
    },
    DropConstraint {
        type_name: String,
        is_node: bool,
        constraint_type: String,
    },
}

/// Changes that can be made to a property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyChanges {
    pub new_type: Option<DataType>,
    pub new_default: Option<serde_json::Value>,
    pub new_required: Option<bool>,
    pub new_unique: Option<bool>,
    pub new_description: Option<String>,
    pub mark_deprecated: Option<bool>,
    pub deprecation_message: Option<String>,
}

/// Result of schema compatibility check
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityResult {
    pub level: CompatibilityLevel,
    pub issues: Vec<CompatibilityIssue>,
    pub migration_hints: Vec<String>,
}

/// Level of compatibility between schema versions
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompatibilityLevel {
    FullyCompatible,
    CompatibleWithDefaults,
    CompatibleWithTransform,
    Incompatible,
}

/// Specific compatibility issue
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityIssue {
    pub severity: IssueSeverity,
    pub type_name: String,
    pub property_name: Option<String>,
    pub description: String,
    pub suggested_fix: Option<String>,
}

/// Severity of compatibility issues
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    Error,   // Blocks migration
    Warning, // May cause issues
    Info,    // Informational only
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let version = GraphTypeVersion::parse("1.2.3").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
        assert_eq!(version.to_string(), "1.2.3");

        let version_with_pre = GraphTypeVersion::parse("2.0.0-beta+build123").unwrap();
        assert_eq!(version_with_pre.major, 2);
        assert_eq!(version_with_pre.pre_release, Some("beta".to_string()));
        assert_eq!(version_with_pre.build_metadata, Some("build123".to_string()));
    }

    #[test]
    fn test_version_compatibility() {
        let v1 = GraphTypeVersion::new(1, 2, 3);
        let v2 = GraphTypeVersion::new(1, 3, 0);
        let v3 = GraphTypeVersion::new(2, 0, 0);

        assert!(v1.is_compatible_with(&v2)); // Same major version
        assert!(!v1.is_compatible_with(&v3)); // Different major version
    }

    #[test]
    fn test_data_type_compatibility() {
        assert!(DataType::String.is_compatible_with(&DataType::Text));
        assert!(DataType::Integer.is_compatible_with(&DataType::BigInt));
        assert!(DataType::Float.is_compatible_with(&DataType::Double));
        assert!(!DataType::String.is_compatible_with(&DataType::Integer));
    }
}
