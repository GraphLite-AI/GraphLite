// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Generic catalog operations and responses
//!
//! This module defines the standard operations that can be performed on any catalog
//! and the responses they can return. These types provide a unified interface for
//! all catalog interactions, regardless of the specific catalog implementation.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Entity types supported by the catalog system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    Schema,
    Graph,
    GraphType,
    VertexType,
    EdgeType,
    Index,
    User,
    Role,
    Ace, // Access Control Entry
    Collection,
    Metric,
    DefaultSchema,
    Store,     // RDF stores and vector stores
    Procedure, // User-defined procedures
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EntityType::Schema => "schema",
            EntityType::Graph => "graph",
            EntityType::GraphType => "graph_type",
            EntityType::VertexType => "vertex_type",
            EntityType::EdgeType => "edge_type",
            EntityType::Index => "index",
            EntityType::User => "user",
            EntityType::Role => "role",
            EntityType::Ace => "ace",
            EntityType::Collection => "collection",
            EntityType::Metric => "metric",
            EntityType::DefaultSchema => "default_schema",
            EntityType::Store => "store",
            EntityType::Procedure => "procedure",
        };
        write!(f, "{}", s)
    }
}

impl From<&str> for EntityType {
    fn from(s: &str) -> Self {
        match s {
            "schema" => EntityType::Schema,
            "graph" => EntityType::Graph,
            "graph_type" => EntityType::GraphType,
            "vertex_type" => EntityType::VertexType,
            "edge_type" => EntityType::EdgeType,
            "index" => EntityType::Index,
            "user" => EntityType::User,
            "role" => EntityType::Role,
            "ace" => EntityType::Ace,
            "collection" => EntityType::Collection,
            "metric" => EntityType::Metric,
            "default_schema" => EntityType::DefaultSchema,
            "store" => EntityType::Store,
            "procedure" => EntityType::Procedure,
            _ => EntityType::Schema, // default fallback
        }
    }
}

impl From<String> for EntityType {
    fn from(s: String) -> Self {
        EntityType::from(s.as_str())
    }
}

/// Query types supported by the catalog system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum QueryType {
    Get,
    GetSchema,
    GetGraph,
    GetGraphType,
    GetUser,
    GetDefault,
    List,
    Search,
    CurrentSchema,
    CurrentGraph,
    Authenticate,
    Exists,
    GetRole,
    ListRoles,
    ListUsers,
    GetAcesForResource,
    GetAcesForPrincipal,
    ByStatus,
    BySchema,
    // Version-specific operations
    GetVersion,   // Get specific version of a schema
    ListVersions, // List all versions of a schema
}

impl fmt::Display for QueryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            QueryType::Get => "get",
            QueryType::GetSchema => "get_schema",
            QueryType::GetGraph => "get_graph",
            QueryType::GetGraphType => "get_graph_type",
            QueryType::GetUser => "get_user",
            QueryType::GetDefault => "get_default",
            QueryType::List => "list",
            QueryType::Search => "search",
            QueryType::CurrentSchema => "current_schema",
            QueryType::CurrentGraph => "current_graph",
            QueryType::Authenticate => "authenticate",
            QueryType::Exists => "exists",
            QueryType::GetRole => "get_role",
            QueryType::ListRoles => "list_roles",
            QueryType::ListUsers => "list_users",
            QueryType::GetAcesForResource => "get_aces_for_resource",
            QueryType::GetAcesForPrincipal => "get_aces_for_principal",
            QueryType::ByStatus => "by_status",
            QueryType::BySchema => "by_schema",
            QueryType::GetVersion => "get_version",
            QueryType::ListVersions => "list_versions",
        };
        write!(f, "{}", s)
    }
}

impl From<&str> for QueryType {
    fn from(s: &str) -> Self {
        match s {
            "get" => QueryType::Get,
            "get_schema" => QueryType::GetSchema,
            "get_graph" => QueryType::GetGraph,
            "get_graph_type" => QueryType::GetGraphType,
            "get_user" => QueryType::GetUser,
            "get_default" => QueryType::GetDefault,
            "list" => QueryType::List,
            "search" => QueryType::Search,
            "current_schema" => QueryType::CurrentSchema,
            "current_graph" => QueryType::CurrentGraph,
            "authenticate" => QueryType::Authenticate,
            "exists" => QueryType::Exists,
            "get_role" => QueryType::GetRole,
            "list_roles" => QueryType::ListRoles,
            "list_users" => QueryType::ListUsers,
            "get_aces_for_resource" => QueryType::GetAcesForResource,
            "get_aces_for_principal" => QueryType::GetAcesForPrincipal,
            "by_status" => QueryType::ByStatus,
            "by_schema" => QueryType::BySchema,
            "get_version" => QueryType::GetVersion,
            "list_versions" => QueryType::ListVersions,
            _ => QueryType::Get, // default fallback
        }
    }
}

impl From<String> for QueryType {
    fn from(s: String) -> Self {
        QueryType::from(s.as_str())
    }
}

/// Generic catalog operations
///
/// This enum represents all possible operations that can be performed on catalogs.
/// Individual catalog providers implement support for the operations they handle,
/// returning `NotSupported` for operations they don't implement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CatalogOperation {
    /// Create a new entity in the catalog
    ///
    /// # Fields
    /// * `entity_type` - Type of entity to create
    /// * `name` - Name of the entity to create
    /// * `params` - Additional parameters for entity creation
    Create {
        entity_type: EntityType,
        name: String,
        params: Value,
    },

    /// Drop (delete) an entity from the catalog
    ///
    /// # Fields
    /// * `entity_type` - Type of entity to drop
    /// * `name` - Name of the entity to drop
    /// * `cascade` - Whether to cascade the deletion to dependent entities
    Drop {
        entity_type: EntityType,
        name: String,
        cascade: bool,
    },

    /// Register a data source schema in the catalog
    ///
    /// # Fields
    /// * `name` - Name of the schema to register
    /// * `params` - Registration parameters (source, validation, etc.)
    Register { name: String, params: Value },

    /// Unregister a data source schema from the catalog
    ///
    /// # Fields
    /// * `name` - Name of the schema to unregister
    /// * `cascade` - Whether to cascade the unregistration to dependent entities
    Unregister { name: String, cascade: bool },

    /// Query the catalog for information
    ///
    /// # Fields
    /// * `query_type` - Type of query to perform
    /// * `params` - Query parameters and filters
    Query {
        query_type: QueryType,
        params: Value,
    },

    /// Update an existing entity in the catalog
    ///
    /// # Fields
    /// * `entity_type` - Type of entity to update
    /// * `name` - Name of the entity to update
    /// * `updates` - Updates to apply to the entity
    Update {
        entity_type: EntityType,
        name: String,
        updates: Value,
    },

    /// List entities of a specific type
    ///
    /// # Fields
    /// * `entity_type` - Type of entities to list
    /// * `filters` - Optional filters to apply to the listing
    List {
        entity_type: EntityType,
        filters: Option<Value>,
    },

    /// Serialize the catalog state
    ///
    /// This operation requests the catalog to serialize its current state
    /// for persistence or transfer purposes.
    Serialize,

    /// Deserialize catalog state from data
    ///
    /// # Fields
    /// * `data` - Serialized catalog state to restore
    Deserialize { data: Vec<u8> },
}

/// Generic catalog responses
///
/// This enum represents all possible responses from catalog operations.
/// Catalog providers return appropriate response types based on the
/// operation performed and its outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CatalogResponse {
    /// Operation completed successfully
    ///
    /// # Fields
    /// * `data` - Optional data returned by the operation
    Success { data: Option<Value> },

    /// Operation failed with an error
    ///
    /// # Fields
    /// * `message` - Error message describing what went wrong
    Error { message: String },

    /// List operation response
    ///
    /// # Fields
    /// * `items` - List of items returned by the operation
    List { items: Vec<Value> },

    /// Query operation response
    ///
    /// # Fields
    /// * `results` - Query results as a structured value
    Query { results: Value },

    /// Operation is not supported by this catalog
    ///
    /// Returned when a catalog doesn't implement support for
    /// a particular operation type.
    NotSupported,
}

impl CatalogResponse {
    /// Create a successful response with no data
    pub fn success() -> Self {
        Self::Success { data: None }
    }

    /// Create a successful response with data
    pub fn success_with_data(data: Value) -> Self {
        Self::Success { data: Some(data) }
    }

    /// Create an error response
    pub fn error<S: Into<String>>(message: S) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Create a list response
    pub fn list(items: Vec<Value>) -> Self {
        Self::List { items }
    }

    /// Create a query response
    pub fn query(results: Value) -> Self {
        Self::Query { results }
    }

    /// Check if the response indicates success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if the response indicates an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Check if the operation was not supported
    pub fn is_not_supported(&self) -> bool {
        matches!(self, Self::NotSupported)
    }

    /// Extract data from a successful response
    pub fn data(&self) -> Option<&Value> {
        match self {
            Self::Success { data } => data.as_ref(),
            _ => None,
        }
    }

    /// Extract error message from an error response
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error { message } => Some(message),
            _ => None,
        }
    }

    /// Extract items from a list response
    pub fn items(&self) -> Option<&[Value]> {
        match self {
            Self::List { items } => Some(items),
            _ => None,
        }
    }

    /// Extract results from a query response
    pub fn results(&self) -> Option<&Value> {
        match self {
            Self::Query { results } => Some(results),
            _ => None,
        }
    }
}
