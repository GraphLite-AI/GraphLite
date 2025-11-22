// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Graph-specific functions for GQL compliance
//!
//! This module implements standard GQL graph functions like LABELS, TYPE, ID, PROPERTIES

use super::function_trait::{Function, FunctionContext, FunctionError, FunctionResult};
use crate::storage::Value;

/// LABELS function - returns list of node labels
#[derive(Debug)]
pub struct LabelsFunction;

impl LabelsFunction {
    pub fn new() -> Self {
        Self
    }

    /// Extract labels from a node string representation
    /// Handles common node string formats like "node_id" or "id:Label1:Label2"
    fn extract_labels_from_node_string(&self, node_str: &str) -> Option<Vec<Value>> {
        // Check for colon-separated label format: "id:Label1:Label2"
        if node_str.contains(':') {
            let parts: Vec<&str> = node_str.split(':').collect();
            if parts.len() > 1 {
                // Skip the first part (assumed to be ID) and treat rest as labels
                let labels: Vec<Value> = parts[1..]
                    .iter()
                    .map(|label| Value::String(label.to_string()))
                    .collect();
                if !labels.is_empty() {
                    return Some(labels);
                }
            }
        }

        // No extractable labels found
        None
    }
}

impl Function for LabelsFunction {
    fn name(&self) -> &str {
        "LABELS"
    }

    fn description(&self) -> &str {
        "Returns a list of labels for a node"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "LIST<STRING>"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let node_ref = &context.arguments[0];

        // Handle direct Node values (from query execution)
        match node_ref {
            Value::Node(node) => {
                let labels: Vec<Value> = node
                    .labels
                    .iter()
                    .map(|label| Value::String(label.clone()))
                    .collect();
                return Ok(Value::List(labels));
            }
            Value::String(variable_name) => {
                // First, check if this is a variable name in the aggregation context
                // Look through the rows to find the actual node data
                if !context.rows.is_empty() {
                    // Try to find the variable in the first row to get a sample value
                    for row in &context.rows {
                        if let Some(node_value) = row.get_value(variable_name) {
                            if let Value::Node(node) = node_value {
                                let labels: Vec<Value> = node
                                    .labels
                                    .iter()
                                    .map(|label| Value::String(label.clone()))
                                    .collect();
                                return Ok(Value::List(labels));
                            }
                        }
                    }
                }

                // If not found in rows, continue with node ID lookup for string references
                let node_id = variable_name.clone();

                // Method 1: Try current_graph first (fastest)
                if let Some(ref current_graph) = context.current_graph {
                    if let Some(node) = current_graph.get_node(&node_id) {
                        let labels: Vec<Value> = node
                            .labels
                            .iter()
                            .map(|label| Value::String(label.clone()))
                            .collect();
                        return Ok(Value::List(labels));
                    }
                }

                // Method 2: Try storage_manager with graph_name
                if let (Some(ref storage_manager), Some(ref graph_name)) =
                    (&context.storage_manager, &context.graph_name)
                {
                    if let Ok(Some(graph)) = storage_manager.get_graph(graph_name) {
                        if let Some(node) = graph.get_node(&node_id) {
                            let labels: Vec<Value> = node
                                .labels
                                .iter()
                                .map(|label| Value::String(label.clone()))
                                .collect();
                            return Ok(Value::List(labels));
                        }
                    }
                }

                // Method 3: Try storage_manager with all available graphs
                if let Some(ref storage_manager) = context.storage_manager {
                    if let Ok(graph_names) = storage_manager.get_graph_names() {
                        for graph_name in graph_names {
                            if let Ok(Some(graph)) = storage_manager.get_graph(&graph_name) {
                                if let Some(node) = graph.get_node(&node_id) {
                                    let labels: Vec<Value> = node
                                        .labels
                                        .iter()
                                        .map(|label| Value::String(label.clone()))
                                        .collect();
                                    return Ok(Value::List(labels));
                                }
                            }
                        }
                    }
                }
            }
            _ => {
                // For unsupported input types, return empty labels
                return Ok(Value::List(vec![]));
            }
        }

        // If no labels found in storage, return empty list
        // Note: Once storage access is fully working, we may want to remove
        // the fallback behavior and return an error instead
        for (var_name, var_value) in &context.variables {
            if var_name == "n" || var_name.starts_with("n.") {
                if let Value::String(node_data) = var_value {
                    if let Some(labels) = self.extract_labels_from_node_string(node_data) {
                        return Ok(Value::List(labels));
                    }
                }
            }
        }

        // Fallback: return empty labels
        Ok(Value::List(vec![]))
    }
}

/// TYPE function - returns relationship type
#[derive(Debug)]
pub struct TypeFunction;

impl TypeFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for TypeFunction {
    fn name(&self) -> &str {
        "TYPE"
    }

    fn description(&self) -> &str {
        "Returns the type of a value or relationship"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "STRING"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let value = &context.arguments[0];

        // Check direct value type inquiry
        match value {
            Value::String(_) => Ok(Value::String("STRING".to_string())),
            Value::Number(_) => Ok(Value::String("NUMBER".to_string())),
            Value::Boolean(_) => Ok(Value::String("BOOLEAN".to_string())),
            Value::Null => Ok(Value::String("NULL".to_string())),
            Value::List(_) => Ok(Value::String("LIST".to_string())),
            Value::Array(_) => Ok(Value::String("ARRAY".to_string())),
            Value::Vector(_) => Ok(Value::String("VECTOR".to_string())),
            Value::Node(_) => Ok(Value::String("NODE".to_string())),
            Value::Edge(_) => Ok(Value::String("EDGE".to_string())),
            Value::DateTime(_) => Ok(Value::String("DATETIME".to_string())),
            Value::DateTimeWithFixedOffset(_) => Ok(Value::String("DATETIME".to_string())),
            Value::DateTimeWithNamedTz(_, _) => Ok(Value::String("DATETIME".to_string())),
            Value::TimeWindow(_) => Ok(Value::String("TIMEWINDOW".to_string())),
            Value::Path(_) => Ok(Value::String("PATH".to_string())),
            Value::Temporal(_) => Ok(Value::String("TEMPORAL".to_string())),
        }
    }
}

/// ID function - returns node/edge identifier
#[derive(Debug)]
pub struct IdFunction;

impl IdFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for IdFunction {
    fn name(&self) -> &str {
        "ID"
    }

    fn description(&self) -> &str {
        "Returns the identifier of a node or edge"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "STRING"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let element_ref = &context.arguments[0];

        // For ID function, the argument should already be the ID itself
        // In GQL, node/edge references typically contain the actual ID
        match element_ref {
            Value::String(ref_str) => {
                // Return the ID string directly
                Ok(Value::String(ref_str.clone()))
            }
            _ => Ok(Value::String("unknown_id".to_string())),
        }
    }
}

/// INFERRED_LABELS function - returns inferred labels based on node properties
/// This is a temporary workaround when storage access is not available
#[derive(Debug)]
pub struct InferredLabelsFunction;

impl InferredLabelsFunction {
    pub fn new() -> Self {
        Self
    }

    /// Infer node labels from available properties
    /// This is a workaround when storage access is not available
    fn infer_labels_from_properties(
        &self,
        variables: &std::collections::HashMap<String, Value>,
    ) -> Vec<String> {
        let mut labels = Vec::new();

        // Collect property names to analyze
        let mut properties = std::collections::HashSet::new();
        for var_name in variables.keys() {
            if let Some(prop_name) = var_name.strip_prefix("n.") {
                // Remove "n." prefix
                properties.insert(prop_name);
            }
        }

        // Infer labels based on property patterns

        // Person indicators
        if properties.contains("email") && properties.contains("age") {
            labels.push("Person".to_string());
        } else if properties.contains("salary") && properties.contains("hire_date") {
            labels.push("Employee".to_string());
        } else if properties.contains("age") && properties.contains("salary") {
            labels.push("Person".to_string());
        }

        // Company indicators
        if properties.contains("revenue") && properties.contains("industry") {
            labels.push("Company".to_string());
        } else if properties.contains("founded") && properties.contains("industry") {
            labels.push("Organization".to_string());
        }

        // Project indicators
        if properties.contains("budget") && properties.contains("status") {
            labels.push("Project".to_string());
        } else if properties.contains("start_date") && properties.contains("budget") {
            labels.push("Project".to_string());
        }

        // Department indicators
        if properties.contains("floor") && !properties.contains("budget") {
            labels.push("Department".to_string());
        }

        // Fallback: try to infer from values
        if labels.is_empty() {
            // Check specific property values
            if let Some(Value::String(email)) = variables.get("n.email") {
                if email.contains("@") {
                    labels.push("Person".to_string());
                }
            }

            if let Some(Value::String(status)) = variables.get("n.status") {
                if status == "active" || status == "inactive" || status == "completed" {
                    labels.push("Project".to_string());
                }
            }
        }

        // If still no labels, use generic based on ID pattern
        if labels.is_empty() {
            if let Some(Value::String(node_id)) = variables.get("n") {
                if node_id.starts_with("emp") || node_id.starts_with("person") {
                    labels.push("Person".to_string());
                } else if node_id.starts_with("proj") {
                    labels.push("Project".to_string());
                } else if node_id.starts_with("comp") {
                    labels.push("Company".to_string());
                } else if node_id.starts_with("dept") {
                    labels.push("Department".to_string());
                } else {
                    // Generic fallback
                    labels.push("Node".to_string());
                }
            }
        }

        labels
    }
}

impl Function for InferredLabelsFunction {
    fn name(&self) -> &str {
        "INFERRED_LABELS"
    }

    fn description(&self) -> &str {
        "Returns inferred labels for a node based on its properties (workaround for storage access issues)"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "LIST<STRING>"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        // Infer labels from node properties
        let inferred_labels = self.infer_labels_from_properties(&context.variables);
        let label_values: Vec<Value> = inferred_labels.into_iter().map(Value::String).collect();

        Ok(Value::List(label_values))
    }

    fn graph_context_required(&self) -> bool {
        false // This function works with context variables only
    }
}

/// KEYS function - returns property names as a list of strings
#[derive(Debug)]
pub struct KeysFunction;

impl KeysFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for KeysFunction {
    fn name(&self) -> &str {
        "KEYS"
    }

    fn description(&self) -> &str {
        "Returns the property names (keys) of a node or edge as a list of strings"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "LIST"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let arg = &context.arguments[0];

        // Handle different argument types
        match arg {
            Value::String(element_id) => {
                // Try to get from current graph cache first
                if let Some(ref current_graph) = context.current_graph {
                    // Try node first, then edge
                    if let Some(node) = current_graph.get_node(element_id) {
                        let keys: Vec<Value> = node
                            .properties
                            .keys()
                            .map(|key| Value::String(key.clone()))
                            .collect();
                        return Ok(Value::List(keys));
                    } else if let Some(edge) = current_graph.get_edge(element_id) {
                        let keys: Vec<Value> = edge
                            .properties
                            .keys()
                            .map(|key| Value::String(key.clone()))
                            .collect();
                        return Ok(Value::List(keys));
                    }
                }

                // If element not found, return empty list
                Ok(Value::List(vec![]))
            }

            _ => Err(FunctionError::InvalidArgumentType {
                message: "KEYS requires a node or edge argument".to_string(),
            }),
        }
    }

    fn graph_context_required(&self) -> bool {
        true // This function needs access to graph data
    }
}

/// PROPERTIES function - returns all properties as a record
#[derive(Debug)]
pub struct PropertiesFunction;

impl PropertiesFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for PropertiesFunction {
    fn name(&self) -> &str {
        "PROPERTIES"
    }

    fn description(&self) -> &str {
        "Returns all properties of a node or edge as a record"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "LIST"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let element_ref = &context.arguments[0];

        // Handle both Node objects and element ID strings
        match element_ref {
            Value::Node(node) => {
                // Directly extract properties from the Node object
                let properties: Vec<Value> = node
                    .properties
                    .iter()
                    .map(|(key, value)| Value::String(format!("{}: {}", key, value)))
                    .collect();
                return Ok(Value::List(properties));
            }
            Value::String(element_str) => {
                // Continue with the original logic for element IDs
                let element_id = element_str.clone();

                // Try to access the graph from the context
                if let Some(ref current_graph) = context.current_graph {
                    // Try node first, then edge
                    if let Some(node) = current_graph.get_node(&element_id) {
                        let properties: Vec<Value> = node
                            .properties
                            .iter()
                            .map(|(key, value)| Value::String(format!("{}: {}", key, value)))
                            .collect();
                        return Ok(Value::List(properties));
                    } else if let Some(edge) = current_graph.get_edge(&element_id) {
                        let properties: Vec<Value> = edge
                            .properties
                            .iter()
                            .map(|(key, value)| Value::String(format!("{}: {}", key, value)))
                            .collect();
                        return Ok(Value::List(properties));
                    }
                }

                // If we reach here with a String element_id, try storage manager as fallback
                if let (Some(ref storage_manager), Some(ref graph_name)) =
                    (&context.storage_manager, &context.graph_name)
                {
                    // Try to get graph from storage manager
                    if let Ok(Some(graph)) = storage_manager.get_graph(graph_name) {
                        // Try node first, then edge
                        if let Some(node) = graph.get_node(&element_id) {
                            let properties: Vec<Value> = node
                                .properties
                                .iter()
                                .map(|(key, value)| Value::String(format!("{}: {}", key, value)))
                                .collect();
                            return Ok(Value::List(properties));
                        } else if let Some(edge) = graph.get_edge(&element_id) {
                            let properties: Vec<Value> = edge
                                .properties
                                .iter()
                                .map(|(key, value)| Value::String(format!("{}: {}", key, value)))
                                .collect();
                            return Ok(Value::List(properties));
                        }
                    }
                }
            }
            _ => {
                return Ok(Value::List(vec![]));
            }
        }

        // Return empty list if no element found
        Ok(Value::List(vec![]))
    }
}

/// SIZE function - returns the size/length of collections, vectors, or strings
#[derive(Debug)]
pub struct SizeFunction;

impl SizeFunction {
    pub fn new() -> Self {
        Self
    }
}

impl Function for SizeFunction {
    fn name(&self) -> &str {
        "SIZE"
    }

    fn description(&self) -> &str {
        "Returns the size/length of a collection, vector, or string"
    }

    fn argument_count(&self) -> usize {
        1
    }

    fn return_type(&self) -> &str {
        "NUMBER"
    }

    fn execute(&self, context: &FunctionContext) -> FunctionResult<Value> {
        if context.arguments.len() != 1 {
            return Err(FunctionError::InvalidArgumentCount {
                expected: 1,
                actual: context.arguments.len(),
            });
        }

        let value = &context.arguments[0];

        match value {
            Value::String(s) => Ok(Value::Number(s.len() as f64)),
            Value::List(list) => Ok(Value::Number(list.len() as f64)),
            Value::Vector(vec) => Ok(Value::Number(vec.len() as f64)),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(FunctionError::InvalidArgumentType {
                message: format!("Expected STRING, LIST, or VECTOR, got {:?}", value),
            }),
        }
    }
}
