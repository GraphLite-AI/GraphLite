// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::ast::ast::{InsertStatement, PatternElement};
use crate::exec::ExecutionError;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::storage::{GraphCache, Node, Edge, Value};
use crate::txn::{UndoOperation, state::OperationType};
use crate::schema::integration::runtime_validator::RuntimeValidator;
use crate::catalog::manager::CatalogManager;

/// Executor for INSERT statements
pub struct InsertExecutor {
    statement: InsertStatement,
    runtime_validator: Option<Arc<RuntimeValidator>>,
}

impl InsertExecutor {
    /// Create a new InsertExecutor
    #[allow(dead_code)] // ROADMAP v0.5.0 - Direct INSERT executor construction without validation
    pub fn new(statement: InsertStatement) -> Self {
        Self {
            statement,
            runtime_validator: None,
        }
    }

    /// Create a new InsertExecutor with schema validation
    #[allow(dead_code)] // ROADMAP v0.5.0 - Schema-validated INSERT for type safety
    pub fn with_validation(statement: InsertStatement, catalog_manager: Arc<RwLock<CatalogManager>>) -> Self {
        Self {
            statement,
            runtime_validator: Some(Arc::new(RuntimeValidator::new(catalog_manager))),
        }
    }
    
    /// Convert AST literal to storage value
    fn literal_to_value(literal: &crate::ast::ast::Literal) -> Value {
        match literal {
            crate::ast::ast::Literal::String(s) => Value::String(s.clone()),
            crate::ast::ast::Literal::Integer(i) => Value::Number(*i as f64),
            crate::ast::ast::Literal::Float(f) => Value::Number(*f),
            crate::ast::ast::Literal::Boolean(b) => Value::Boolean(*b),
            crate::ast::ast::Literal::Null => Value::Null,
            crate::ast::ast::Literal::DateTime(dt) => Value::String(dt.clone()),
            crate::ast::ast::Literal::Duration(dur) => Value::String(dur.clone()),
            crate::ast::ast::Literal::TimeWindow(tw) => Value::String(tw.clone()),
            crate::ast::ast::Literal::Vector(vec) => Value::Vector(vec.iter().map(|&f| f as f32).collect()),
            crate::ast::ast::Literal::List(list) => {
                let converted: Vec<Value> = list.iter().map(|lit| Self::literal_to_value(lit)).collect();
                Value::List(converted)
            },
        }
    }
    
    /// Extract properties from a property map
    fn extract_properties(prop_map: &crate::ast::ast::PropertyMap) -> HashMap<String, Value> {
        let mut properties = HashMap::new();

        for property in &prop_map.properties {
            if let crate::ast::ast::Expression::Literal(literal) = &property.value {
                properties.insert(property.key.clone(), Self::literal_to_value(literal));
            } else {
                log::warn!("Complex property expressions not supported in INSERT, skipping property: {}", property.key);
            }
        }

        properties
    }

    /// Generate a content-based hash ID for a node based on labels and properties
    fn generate_node_content_id(labels: &[String], properties: &HashMap<String, Value>) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash labels (sorted for consistency)
        let mut sorted_labels = labels.to_vec();
        sorted_labels.sort();
        for label in &sorted_labels {
            label.hash(&mut hasher);
        }

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            // Hash the value in a consistent way
            match value {
                Value::String(s) => s.hash(&mut hasher),
                Value::Number(n) => n.to_bits().hash(&mut hasher),
                Value::Boolean(b) => b.hash(&mut hasher),
                Value::Null => "null".hash(&mut hasher),
                Value::Vector(v) => {
                    for f in v {
                        f.to_bits().hash(&mut hasher);
                    }
                },
                Value::List(list) => {
                    // Simplified hash for lists - could be enhanced
                    list.len().hash(&mut hasher);
                    for item in list {
                        // Recursively hash list items (simplified)
                        match item {
                            Value::String(s) => s.hash(&mut hasher),
                            Value::Number(n) => n.to_bits().hash(&mut hasher),
                            Value::Boolean(b) => b.hash(&mut hasher),
                            _ => "complex".hash(&mut hasher),
                        }
                    }
                },
                // Handle additional Value types
                Value::DateTime(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithFixedOffset(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithNamedTz(tz, dt) => {
                    tz.hash(&mut hasher);
                    dt.timestamp().hash(&mut hasher);
                },
                Value::TimeWindow(tw) => format!("{:?}", tw).hash(&mut hasher),
                Value::Array(arr) => {
                    arr.len().hash(&mut hasher);
                    for item in arr {
                        // Simplified recursive hashing for arrays
                        format!("{:?}", item).hash(&mut hasher);
                    }
                },
                // Catch-all for any other Value types
                _ => format!("{:?}", value).hash(&mut hasher),
            }
        }

        let hash = hasher.finish();
        format!("node_{:x}", hash)
    }

    /// Generate a content-based hash ID for an edge
    fn generate_edge_content_id(from_node_id: &str, to_node_id: &str, label: &str, properties: &HashMap<String, Value>) -> String {
        let mut hasher = DefaultHasher::new();

        // Hash the connection (from_node -> to_node -> label)
        from_node_id.hash(&mut hasher);
        to_node_id.hash(&mut hasher);
        label.hash(&mut hasher);

        // Hash properties (sorted by key for consistency)
        let mut sorted_properties: Vec<_> = properties.iter().collect();
        sorted_properties.sort_by_key(|(k, _)| *k);
        for (key, value) in sorted_properties {
            key.hash(&mut hasher);
            // Hash the value (same logic as node properties)
            match value {
                Value::String(s) => s.hash(&mut hasher),
                Value::Number(n) => n.to_bits().hash(&mut hasher),
                Value::Boolean(b) => b.hash(&mut hasher),
                Value::Null => "null".hash(&mut hasher),
                Value::Vector(v) => {
                    for f in v {
                        f.to_bits().hash(&mut hasher);
                    }
                },
                Value::List(list) => {
                    list.len().hash(&mut hasher);
                    for item in list {
                        match item {
                            Value::String(s) => s.hash(&mut hasher),
                            Value::Number(n) => n.to_bits().hash(&mut hasher),
                            Value::Boolean(b) => b.hash(&mut hasher),
                            _ => "complex".hash(&mut hasher),
                        }
                    }
                },
                // Handle additional Value types
                Value::DateTime(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithFixedOffset(dt) => dt.timestamp().hash(&mut hasher),
                Value::DateTimeWithNamedTz(tz, dt) => {
                    tz.hash(&mut hasher);
                    dt.timestamp().hash(&mut hasher);
                },
                Value::TimeWindow(tw) => format!("{:?}", tw).hash(&mut hasher),
                Value::Array(arr) => {
                    arr.len().hash(&mut hasher);
                    for item in arr {
                        // Simplified recursive hashing for arrays
                        format!("{:?}", item).hash(&mut hasher);
                    }
                },
                // Catch-all for any other Value types
                _ => format!("{:?}", value).hash(&mut hasher),
            }
        }

        let hash = hasher.finish();
        format!("edge_{:x}", hash)
    }
}

impl StatementExecutor for InsertExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Insert
    }
    
    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context.get_graph_name().unwrap_or_else(|_| "unknown".to_string());
        format!("INSERT into graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for InsertExecutor {
    
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        log::debug!("DEBUG INSERT: execute_modification called");

        let graph_name = match context.get_graph_name() {
            Ok(name) => {
                log::debug!("DEBUG INSERT: Successfully got graph_name: '{}'", name);
                name
            },
            Err(e) => {
                log::debug!("DEBUG INSERT: Failed to get graph_name: {}", e);
                return Err(e);
            }
        };
        let patterns = &self.statement.graph_patterns;

        let mut undo_operations = Vec::new();
        let mut inserted_nodes = 0;
        let mut inserted_edges = 0;
        let mut created_node_ids: Vec<String> = Vec::new();
        // Map from user identifiers to actual storage node IDs
        let mut identifier_to_node_id: HashMap<String, String> = HashMap::new();
        
        // Process each graph pattern to extract nodes and edges to insert
        log::debug!("INSERT processing {} graph patterns", patterns.len());

        // First pass: process all nodes
        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            log::debug!("INSERT processing pattern {} for nodes (pass 1)", pattern_idx);
            for (_i, element) in pattern.elements.iter().enumerate() {
                if let PatternElement::Node(node_pattern) = element {
                    // Check if this is a reference to an existing node
                    if let Some(ref user_identifier) = node_pattern.identifier {
                        // If we already have a mapping for this identifier, it's a reference
                        if identifier_to_node_id.contains_key(user_identifier) {
                            log::debug!("Skipping node creation for identifier '{}' - already exists", user_identifier);
                            continue;
                        }
                    }

                    // Extract properties first
                    let properties = if let Some(ref prop_map) = node_pattern.properties {
                        Self::extract_properties(prop_map)
                    } else {
                        HashMap::new()
                    };

                    // Validate against schema if validator is available
                    if let Some(ref validator) = self.runtime_validator {
                        // Use the first label as the primary type (GQL allows multiple labels)
                        if let Some(primary_label) = node_pattern.labels.first() {
                            // Convert storage::Value to serde_json::Value for validation
                            let json_properties: HashMap<String, serde_json::Value> = properties.iter()
                                .map(|(k, v)| {
                                    let json_val = match v {
                                        Value::String(s) => serde_json::Value::String(s.clone()),
                                        Value::Number(n) => serde_json::json!(n),
                                        Value::Boolean(b) => serde_json::Value::Bool(*b),
                                        Value::Null => serde_json::Value::Null,
                                        Value::Vector(vec) => serde_json::json!(vec),
                                        Value::List(list) => {
                                            // Convert list recursively (simplified for now)
                                            serde_json::json!(list)
                                        },
                                        _ => serde_json::Value::Null, // Handle other types as needed
                                    };
                                    (k.clone(), json_val)
                                })
                                .collect();

                            // Run validation synchronously
                            if let Err(e) = validator.validate_insert(&graph_name, primary_label, &json_properties) {
                                log::error!("Schema validation failed for node: {}", e);
                                return Err(e);
                            }
                        }
                    }

                    // Generate content-based storage ID from labels and properties
                    let storage_node_id = Self::generate_node_content_id(&node_pattern.labels, &properties);

                    // If there's a user identifier, map it to the storage ID
                    if let Some(ref user_identifier) = node_pattern.identifier {
                        log::debug!("INSERT mapping user identifier '{}' to storage ID '{}'", user_identifier, storage_node_id);
                        identifier_to_node_id.insert(user_identifier.clone(), storage_node_id.clone());
                    }

                    log::debug!("INSERT creating node with content-based storage ID: {}", storage_node_id);

                    // Create the node
                    let node = Node {
                        id: storage_node_id.clone(),
                        labels: node_pattern.labels.clone(),
                        properties,
                    };

                    // Try to add to graph - this will detect duplicates automatically
                    match graph.add_node(node) {
                        Ok(_) => {
                            log::info!("Successfully added node '{}' to graph", storage_node_id);
                            inserted_nodes += 1;

                            // Add undo operation only for newly inserted nodes
                            let undo_op = UndoOperation::InsertNode {
                                graph_path: graph_name.clone(),
                                node_id: storage_node_id.clone(),
                            };
                            log::debug!("DEBUG INSERT: Created undo_op with graph_path: '{}', node_id: '{}'", graph_name, storage_node_id);
                            undo_operations.push(undo_op);
                        },
                        Err(crate::storage::types::GraphError::NodeAlreadyExists(_)) => {
                            log::info!("Node with content '{}' already exists, skipping duplicate", storage_node_id);
                            // Add warning about duplicate insertion
                            let warning_msg = format!("Duplicate node detected: Node with identical properties already exists (node_id: {})", storage_node_id);
                            context.add_warning(warning_msg);
                            // Don't count this as an insertion or error - it's a duplicate
                        },
                        Err(e) => {
                            log::error!("Failed to insert node '{}': {}", storage_node_id, e);
                            return Err(ExecutionError::RuntimeError(format!("Failed to insert node '{}': {}", storage_node_id, e)));
                        }
                    }

                    log::debug!("Processed node with storage ID: {}", storage_node_id);
                    created_node_ids.push(storage_node_id.clone());
                }
            }
        }

        // Second pass: process all edges
        for (pattern_idx, pattern) in patterns.iter().enumerate() {
            log::debug!("INSERT processing pattern {} for edges (pass 2)", pattern_idx);
            for (i, element) in pattern.elements.iter().enumerate() {
            match element {
                PatternElement::Node(_node_pattern) => {
                    // In pass 2, we skip node processing - all nodes were already created in pass 1
                    // The identifier mappings are already established from pass 1
                },
                PatternElement::Edge(edge_pattern) => {
                    // For edges, we need to connect the previous and next nodes
                    if i == 0 || i >= pattern.elements.len() - 1 {
                        return Err(ExecutionError::RuntimeError(
                            "Edge patterns in INSERT must be between two nodes".to_string()
                        ));
                    }

                    // Get the source node from the previous element
                    let source_node_id = match pattern.elements.get(i - 1) {
                        Some(PatternElement::Node(source_node)) => {
                            if let Some(ref identifier) = source_node.identifier {
                                // Use the mapping to get the actual storage ID
                                match identifier_to_node_id.get(identifier) {
                                    Some(storage_id) => {
                                        log::debug!("Edge source: found mapping '{}' -> '{}'", identifier, storage_id);
                                        storage_id.clone()
                                    },
                                    None => {
                                        log::error!("Edge source: identifier '{}' not found in mapping. Available mappings: {:?}", identifier, identifier_to_node_id);
                                        return Err(ExecutionError::RuntimeError(format!("Source node identifier '{}' not found in current statement", identifier)));
                                    }
                                }
                            } else {
                                // Anonymous nodes in edge patterns need special handling
                                // Check if this is an empty node reference that shouldn't create a new node
                                if source_node.labels.is_empty() && source_node.properties.is_none() {
                                    // This is likely a reference like (n) with no content - error out
                                    return Err(ExecutionError::RuntimeError(
                                        "Cannot create edge from anonymous empty node - use an identifier instead".to_string()
                                    ));
                                }
                                // Generate storage ID from node content for truly anonymous nodes with content
                                let properties = if let Some(ref prop_map) = source_node.properties {
                                    Self::extract_properties(prop_map)
                                } else {
                                    HashMap::new()
                                };
                                let storage_id = Self::generate_node_content_id(&source_node.labels, &properties);
                                log::debug!("Edge source: anonymous node with labels={:?}, properties={:?} generated ID '{}'", source_node.labels, properties, storage_id);
                                storage_id
                            }
                        },
                        _ => return Err(ExecutionError::RuntimeError(
                            "Edge pattern must be preceded by a source node".to_string()
                        )),
                    };

                    // Get the target node from the next element
                    let target_node_id = match pattern.elements.get(i + 1) {
                        Some(PatternElement::Node(target_node)) => {
                            if let Some(ref identifier) = target_node.identifier {
                                // Use the mapping to get the actual storage ID
                                match identifier_to_node_id.get(identifier) {
                                    Some(storage_id) => {
                                        log::debug!("Edge target: found mapping '{}' -> '{}'", identifier, storage_id);
                                        storage_id.clone()
                                    },
                                    None => {
                                        log::error!("Edge target: identifier '{}' not found in mapping. Available mappings: {:?}", identifier, identifier_to_node_id);
                                        return Err(ExecutionError::RuntimeError(format!("Target node identifier '{}' not found in current statement", identifier)));
                                    }
                                }
                            } else {
                                // Anonymous nodes in edge patterns need special handling
                                // Check if this is an empty node reference that shouldn't create a new node
                                if target_node.labels.is_empty() && target_node.properties.is_none() {
                                    // This is likely a reference like (m) with no content - error out
                                    return Err(ExecutionError::RuntimeError(
                                        "Cannot create edge to anonymous empty node - use an identifier instead".to_string()
                                    ));
                                }
                                // Generate storage ID from node content for truly anonymous nodes with content
                                let properties = if let Some(ref prop_map) = target_node.properties {
                                    Self::extract_properties(prop_map)
                                } else {
                                    HashMap::new()
                                };
                                let storage_id = Self::generate_node_content_id(&target_node.labels, &properties);
                                log::debug!("Edge target: anonymous node with labels={:?}, properties={:?} generated ID '{}'", target_node.labels, properties, storage_id);
                                storage_id
                            }
                        },
                        _ => return Err(ExecutionError::RuntimeError(
                            "Edge pattern must be followed by a target node".to_string()
                        )),
                    };
                    
                    // Extract edge properties if present
                    let edge_properties = if let Some(ref prop_map) = edge_pattern.properties {
                        Self::extract_properties(prop_map)
                    } else {
                        HashMap::new()
                    };

                    let edge_label = edge_pattern.labels.first().cloned().unwrap_or_else(|| "CONNECTED".to_string());

                    // Generate content-based storage ID for the edge
                    let edge_storage_id = Self::generate_edge_content_id(&source_node_id, &target_node_id, &edge_label, &edge_properties);

                    log::debug!("Creating edge with content-based storage ID: {}", edge_storage_id);

                    // Create the edge
                    let edge = Edge {
                        id: edge_storage_id.clone(),
                        from_node: source_node_id,
                        to_node: target_node_id,
                        label: edge_label,
                        properties: edge_properties,
                    };

                    // Try to add to graph - this will detect duplicate edges automatically
                    match graph.add_edge(edge) {
                        Ok(_) => {
                            log::info!("Successfully added edge '{}' to graph", edge_storage_id);
                            inserted_edges += 1;

                            // Add undo operation only for newly inserted edges
                            undo_operations.push(UndoOperation::InsertEdge {
                                graph_path: graph_name.clone(),
                                edge_id: edge_storage_id,
                            });
                        },
                        Err(crate::storage::types::GraphError::EdgeAlreadyExists(_)) => {
                            log::info!("Edge with content '{}' already exists, skipping duplicate", edge_storage_id);
                            // Add warning about duplicate insertion
                            let warning_msg = format!("Duplicate edge detected: Edge with identical properties already exists (edge_id: {})", edge_storage_id);
                            context.add_warning(warning_msg);
                            // Don't count this as an insertion or error - it's a duplicate
                        },
                        Err(e) => {
                            return Err(ExecutionError::RuntimeError(format!("Failed to insert edge: {}", e)));
                        }
                    }
                }
            }
        }
        }
        
        let total_inserted = inserted_nodes + inserted_edges;

        // Return the first undo operation (unified system handles multiple operations internally)
        let undo_op = undo_operations.into_iter().next().unwrap_or_else(|| {
            UndoOperation::InsertNode {
                graph_path: graph_name.clone(),
                node_id: "no_operations".to_string(),
            }
        });

        Ok((undo_op, total_inserted))
    }
}
