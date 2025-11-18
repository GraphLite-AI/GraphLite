// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
// Schema-specific AST nodes for ISO GQL Graph Type statements

use crate::schema::types::{
    NodeTypeDefinition, EdgeTypeDefinition, GraphTypeVersion, SchemaChange
};

/// CREATE GRAPH TYPE statement AST
#[derive(Debug, Clone)]
pub struct CreateGraphTypeStatement {
    pub name: String,
    pub if_not_exists: bool,
    pub version: Option<GraphTypeVersion>,
    pub node_types: Vec<NodeTypeDefinition>,
    pub edge_types: Vec<EdgeTypeDefinition>,
}

/// DROP GRAPH TYPE statement AST
#[derive(Debug, Clone)]
pub struct DropGraphTypeStatement {
    pub name: String,
    pub if_exists: bool,
    pub cascade: bool,
}

/// ALTER GRAPH TYPE statement AST
#[derive(Debug, Clone)]
pub struct AlterGraphTypeStatement {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Graph type name for ALTER GRAPH TYPE DDL
    pub name: String,
    #[allow(dead_code)] // ROADMAP v0.4.0 - Version specification for schema evolution tracking
    pub version: Option<GraphTypeVersion>,
    #[allow(dead_code)] // ROADMAP v0.4.0 - Schema change operations (ADD/DROP/ALTER node/edge types)
    pub changes: Vec<SchemaChange>,
}
