// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query execution results for graph databases

use crate::ast::{CatalogPath, GraphExpression};
use crate::storage::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// Entity identifier for tracking graph element identities in set operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityId {
    /// Node with its internal ID
    Node(String),
    /// Edge with its internal ID
    Edge(String),
}

/// Session change request returned by executor for session statements
/// Following PostgreSQL/Oracle pattern where executor validates and returns metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionResult {
    /// Set the current graph for the session
    SetGraph {
        graph_expression: GraphExpression,
        validated: bool, // Whether executor validated the graph exists
    },
    /// Set the current schema for the session
    SetSchema {
        schema_reference: CatalogPath,
        validated: bool, // Whether executor validated the schema exists
    },
    /// Set session timezone
    SetTimeZone { timezone: String },
    /// Reset session to defaults
    Reset,
    /// Close session
    Close,
}

/// Query execution result for graph queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows: Vec<Row>,
    /// Variable bindings from the RETURN clause (e.g., ["p.name", "p.age"])
    pub variables: Vec<String>,
    pub execution_time_ms: u64,
    pub rows_affected: usize,
    /// Session change request if this was a session statement
    pub session_result: Option<SessionResult>,
    /// Warnings generated during query execution (e.g., duplicate insert detection)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl Default for QueryResult {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryResult {
    /// Create a new empty query result
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            variables: Vec::new(),
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: None,
            warnings: Vec::new(),
        }
    }

    /// Create a query result for a session statement
    pub fn for_session(session_result: SessionResult) -> Self {
        Self {
            rows: Vec::new(),
            variables: Vec::new(),
            execution_time_ms: 0,
            rows_affected: 0,
            session_result: Some(session_result),
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the query result
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    /// Get values from all rows at a specific position (for set operations)
    pub fn get_values_at_position(&self, position: usize) -> Vec<&Value> {
        self.rows
            .iter()
            .filter_map(|row| row.get_value_at_position(position))
            .collect()
    }

    /// Create a new result with unified variable names for set operations
    pub fn with_unified_variables(mut self, unified_variables: Vec<String>) -> Self {
        self.variables = unified_variables;
        self
    }

    /// Check if this result contains a session command
    pub fn is_session_command(&self) -> bool {
        self.session_result.is_some()
    }

    /// Get a formatted message for session commands (returns None if not a session command)
    pub fn get_session_message(&self) -> Option<String> {
        self.session_result.as_ref().map(|sr| match sr {
            SessionResult::SetGraph {
                graph_expression,
                validated: _,
            } => {
                let graph_path = match graph_expression {
                    GraphExpression::Reference(path) => path.to_string(),
                    GraphExpression::Union { .. } => "UNION expression".to_string(),
                    GraphExpression::CurrentGraph => "CURRENT_GRAPH".to_string(),
                };
                format!("Session graph set to: {}", graph_path)
            }
            SessionResult::SetSchema {
                schema_reference,
                validated: _,
            } => {
                let schema_path = format!("/{}", schema_reference.segments.join("/"));
                format!("Session schema set to: {}", schema_path)
            }
            SessionResult::SetTimeZone { timezone } => {
                format!("Session timezone set to: {}", timezone)
            }
            SessionResult::Reset => "Session reset to defaults".to_string(),
            SessionResult::Close => "Session closed".to_string(),
        })
    }
}

/// Single result row representing variable bindings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// Named variable bindings (e.g., "p.name" -> "Alice")
    pub values: HashMap<String, Value>,
    /// Positional values for set operations (ordered by variable declaration)
    pub positional_values: Vec<Value>,
    /// Source entity IDs for identity-based set operations
    /// Maps variable names to their graph entity IDs (e.g., "p" -> Node ID)
    #[serde(default)]
    pub source_entities: HashMap<String, EntityId>,
    /// Text search relevance score (for text_match results)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_score: Option<f64>,
    /// Highlighted snippet (for highlight() results)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight_snippet: Option<String>,
}

impl Row {
    /// Create a new empty row
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            positional_values: Vec::new(),
            source_entities: HashMap::new(),
            text_score: None,
            highlight_snippet: None,
        }
    }

    /// Create a row from a HashMap of values (backward compatibility)
    pub fn from_values(values: HashMap<String, Value>) -> Self {
        Self {
            values,
            positional_values: Vec::new(),
            source_entities: HashMap::new(),
            text_score: None,
            highlight_snippet: None,
        }
    }

    /// Create a row from positional values with variable names
    pub fn from_positional(values: Vec<Value>, variables: &[String]) -> Self {
        let mut named_values = HashMap::new();
        for (value, var_name) in values.iter().zip(variables.iter()) {
            named_values.insert(var_name.clone(), value.clone());
        }

        Self {
            values: named_values,
            positional_values: values,
            source_entities: HashMap::new(),
            text_score: None,
            highlight_snippet: None,
        }
    }

    /// Set text search score
    pub fn set_text_score(&mut self, score: f64) {
        self.text_score = Some(score);
    }

    /// Get text search score
    pub fn get_text_score(&self) -> Option<f64> {
        self.text_score
    }

    /// Set highlight snippet
    pub fn set_highlight_snippet(&mut self, snippet: String) {
        self.highlight_snippet = Some(snippet);
    }

    /// Get highlight snippet
    pub fn get_highlight_snippet(&self) -> Option<&str> {
        self.highlight_snippet.as_deref()
    }

    /// Add a value to the row (both named and positional)
    pub fn add_value(&mut self, name: String, value: Value) {
        self.values.insert(name, value.clone());
        self.positional_values.push(value);
    }

    /// Get a value by variable name
    pub fn get_value(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }

    /// Get a value by position (for set operations)
    pub fn get_value_at_position(&self, position: usize) -> Option<&Value> {
        self.positional_values.get(position)
    }

    /// Set a value in the row
    pub fn set_value(&mut self, name: String, value: Value) {
        self.values.insert(name, value.clone());
        // For simplicity, we'll rebuild positional values when setting individual values
        // In a production system, you'd want to maintain position mapping
        if !self.positional_values.is_empty() {
            // Update the existing positional value if it exists
            // This is a simplified approach - production code would need better position tracking
        }
    }

    /// Track the source entity for a variable
    /// This enables identity-based comparison in set operations
    pub fn with_entity(&mut self, var_name: &str, value: &Value) {
        match value {
            Value::Node(node) => {
                self.source_entities
                    .insert(var_name.to_string(), EntityId::Node(node.id.clone()));
            }
            Value::Edge(edge) => {
                self.source_entities
                    .insert(var_name.to_string(), EntityId::Edge(edge.id.clone()));
            }
            _ => {
                // Properties and computed values don't have entity IDs
                // We might track their source entity in the future
            }
        }
    }

    /// Get the primary entity ID for this row (if any)
    /// Used for simple identity comparisons
    pub fn get_primary_entity(&self) -> Option<&EntityId> {
        // Return the first entity (useful for single-entity queries)
        self.source_entities.values().next()
    }

    /// Check if this row has any tracked entities
    pub fn has_entities(&self) -> bool {
        !self.source_entities.is_empty()
    }

    /// Get all tracked entities for this row
    pub fn get_entities(&self) -> &HashMap<String, EntityId> {
        &self.source_entities
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        // For set operations, compare positionally if both have positional values
        if !self.positional_values.is_empty() && !other.positional_values.is_empty() {
            // SQL set operations: NULL != NULL
            self.sql_set_equal(&other.positional_values)
        } else {
            // Fallback to named comparison for backward compatibility
            self.sql_set_equal_named(other)
        }
    }
}

impl Row {
    /// SQL set operation equality: NULL != NULL
    fn sql_set_equal(&self, other_values: &[Value]) -> bool {
        if self.positional_values.len() != other_values.len() {
            return false;
        }

        for (self_val, other_val) in self.positional_values.iter().zip(other_values.iter()) {
            // In SQL set operations, NULL != NULL
            if matches!(self_val, Value::Null) || matches!(other_val, Value::Null) {
                return false;
            }
            if self_val != other_val {
                return false;
            }
        }

        true
    }

    /// SQL set operation equality for named values
    fn sql_set_equal_named(&self, other: &Self) -> bool {
        if self.values.len() != other.values.len() {
            return false;
        }

        for (key, self_val) in &self.values {
            if let Some(other_val) = other.values.get(key) {
                // In SQL set operations, NULL != NULL
                if matches!(self_val, Value::Null) || matches!(other_val, Value::Null) {
                    return false;
                }
                if self_val != other_val {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

impl Eq for Row {}

impl Hash for Row {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // For set operations, hash positionally if available
        if !self.positional_values.is_empty() {
            self.positional_values.hash(state);
        } else {
            // Fallback to named hashing for backward compatibility
            let mut items: Vec<_> = self.values.iter().collect();
            items.sort_by_key(|(k, _)| *k);
            for (key, value) in items {
                key.hash(state);
                value.hash(state);
            }
        }
    }
}
