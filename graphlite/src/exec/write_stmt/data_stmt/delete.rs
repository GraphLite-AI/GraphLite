// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use crate::ast::{DeleteStatement, Expression};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::{GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for DELETE statements
pub struct DeleteExecutor {
    statement: DeleteStatement,
}

impl DeleteExecutor {
    /// Create a new DeleteExecutor
    pub fn new(statement: DeleteStatement) -> Self {
        Self { statement }
    }

    /// Check if a node matches a given pattern
    fn node_matches_pattern(node: &Node, pattern: &crate::ast::Node) -> bool {
        // Check labels
        if !pattern.labels.is_empty() {
            // Node must have all labels from the pattern
            for pattern_label in &pattern.labels {
                if !node.labels.contains(pattern_label) {
                    return false;
                }
            }
        }

        // Check properties
        if let Some(pattern_props) = &pattern.properties {
            for prop in &pattern_props.properties {
                let key = &prop.key;
                // Get the property value from the node
                let node_value = node.properties.get(key);

                // For now, we'll do simple literal matching
                // TODO: Support more complex expressions
                match &prop.value {
                    Expression::Literal(lit) => {
                        // Convert the literal to a Value for comparison
                        let expected_value = match lit {
                            crate::ast::Literal::String(s) => Value::String(s.clone()),
                            crate::ast::Literal::Integer(i) => Value::Number(*i as f64),
                            crate::ast::Literal::Float(f) => Value::Number(*f),
                            crate::ast::Literal::Boolean(b) => Value::Boolean(*b),
                            crate::ast::Literal::Null => Value::Null,
                            _ => {
                                log::warn!("Unsupported literal type in DELETE pattern");
                                return false;
                            }
                        };

                        if node_value != Some(&expected_value) {
                            return false;
                        }
                    }
                    _ => {
                        log::warn!(
                            "Complex property expressions in DELETE patterns not yet supported"
                        );
                        return false;
                    }
                }
            }
        }

        true
    }
}

impl StatementExecutor for DeleteExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Delete
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        let prefix = if self.statement.detach { "DETACH " } else { "" };
        format!("{}DELETE nodes/edges in graph '{}'", prefix, graph_name)
    }
}

impl DataStatementExecutor for DeleteExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut deleted_count = 0;

        // Process each expression in DELETE statement
        for expr in &self.statement.expressions {
            match expr {
                Expression::Pattern(pattern_expr) => {
                    // Handle pattern-based deletion (e.g., (c:Company {name: 'Google'}))
                    let pattern = &pattern_expr.pattern;

                    // For now, we only support simple node patterns, not full path patterns
                    if pattern.elements.len() == 1 {
                        if let crate::ast::PatternElement::Node(node_pattern) = &pattern.elements[0]
                        {
                            // Collect nodes that match the pattern
                            let node_ids_to_delete: Vec<String> = graph
                                .get_all_nodes()
                                .iter()
                                .filter(|node| Self::node_matches_pattern(node, node_pattern))
                                .map(|node| node.id.clone())
                                .collect();

                            // Delete matching nodes
                            for node_id in node_ids_to_delete {
                                // Get the node data before deleting it for undo
                                let node = if let Some(node) = graph.get_node(&node_id) {
                                    node.clone()
                                } else {
                                    log::error!("Node {} not found for deletion", node_id);
                                    continue;
                                };

                                // Check for connected edges
                                let connected_edge_ids: Vec<String> = graph
                                    .get_all_edges()
                                    .iter()
                                    .filter(|edge| {
                                        edge.from_node == node_id || edge.to_node == node_id
                                    })
                                    .map(|edge| edge.id.clone())
                                    .collect();

                                if self.statement.detach {
                                    // DETACH DELETE: remove all connected edges first
                                    for edge_id in connected_edge_ids {
                                        // Get edge data before deleting for undo
                                        let edge = if let Some(edge) = graph.get_edge(&edge_id) {
                                            edge.clone()
                                        } else {
                                            log::error!("Edge {} not found for deletion", edge_id);
                                            continue;
                                        };

                                        if let Err(e) = graph.remove_edge(&edge.id) {
                                            log::error!(
                                                "Failed to remove edge {} during DETACH DELETE: {}",
                                                edge.id,
                                                e
                                            );
                                            continue;
                                        }

                                        log::debug!(
                                            "Removed edge {} during DETACH DELETE of node {}",
                                            edge.id,
                                            node_id
                                        );

                                        // Add undo operation for edge
                                        undo_operations.push(UndoOperation::DeleteEdge {
                                            graph_path: graph_name.clone(),
                                            edge_id: edge.id.clone(),
                                            deleted_edge: edge,
                                        });
                                    }
                                } else if !connected_edge_ids.is_empty() {
                                    // Regular DELETE: cannot delete node with relationships
                                    return Err(ExecutionError::RuntimeError(format!(
                                        "Cannot delete node {} with relationships. Use DETACH DELETE to remove relationships first.",
                                        node_id
                                    )));
                                }

                                // Delete the node
                                if let Err(e) = graph.remove_node(&node_id) {
                                    log::error!("Failed to delete node {}: {}", node_id, e);
                                    continue;
                                }

                                log::debug!("Deleted node {} matching pattern", node_id);
                                deleted_count += 1;

                                // Add undo operation for node
                                undo_operations.push(UndoOperation::DeleteNode {
                                    graph_path: graph_name.clone(),
                                    node_id: node_id.clone(),
                                    deleted_node: node,
                                });
                            }
                        } else if pattern.elements.len() == 3 {
                            // Handle edge patterns: (node)-[edge]->(node)
                            // TODO: Implement edge pattern matching
                            log::warn!("Edge pattern deletion not yet fully implemented");
                        }
                    } else {
                        log::warn!("Complex path patterns in DELETE not yet supported");
                    }
                }
                Expression::Variable(var) => {
                    // Delete nodes/edges identified by variable
                    let var_name = &var.name;

                    // First, collect nodes to delete
                    let node_ids_to_delete: Vec<String> = graph
                        .get_all_nodes()
                        .iter()
                        .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                        .map(|node| node.id.clone())
                        .collect();

                    // Delete nodes
                    for node_id in node_ids_to_delete {
                        // Get the node data before deleting it for undo
                        let node = if let Some(node) = graph.get_node(&node_id) {
                            node.clone()
                        } else {
                            log::error!("Node {} not found for deletion", node_id);
                            continue;
                        };

                        // Check for connected edges
                        let connected_edge_ids: Vec<String> = graph
                            .get_all_edges()
                            .iter()
                            .filter(|edge| edge.from_node == node_id || edge.to_node == node_id)
                            .map(|edge| edge.id.clone())
                            .collect();

                        if self.statement.detach {
                            // DETACH DELETE: remove all connected edges first
                            for edge_id in connected_edge_ids {
                                // Get edge data before deleting for undo
                                let edge = if let Some(edge) = graph.get_edge(&edge_id) {
                                    edge.clone()
                                } else {
                                    log::error!("Edge {} not found for deletion", edge_id);
                                    continue;
                                };

                                if let Err(e) = graph.remove_edge(&edge.id) {
                                    log::error!(
                                        "Failed to remove edge {} during DETACH DELETE: {}",
                                        edge.id,
                                        e
                                    );
                                    continue;
                                }

                                log::debug!(
                                    "Removed edge {} during DETACH DELETE of node {}",
                                    edge.id,
                                    node_id
                                );

                                // Add undo operation for edge
                                undo_operations.push(UndoOperation::DeleteEdge {
                                    graph_path: graph_name.clone(),
                                    edge_id: edge.id.clone(),
                                    deleted_edge: edge,
                                });
                            }
                        } else if !connected_edge_ids.is_empty() {
                            // Regular DELETE: cannot delete node with relationships
                            return Err(ExecutionError::RuntimeError(format!(
                                "Cannot delete node {} with relationships. Use DETACH DELETE to remove relationships first.",
                                node_id
                            )));
                        }

                        // Delete the node
                        if let Err(e) = graph.remove_node(&node_id) {
                            log::error!("Failed to delete node {}: {}", node_id, e);
                            continue;
                        }

                        log::debug!("Deleted node {}", node_id);
                        deleted_count += 1;

                        // Add undo operation for node
                        undo_operations.push(UndoOperation::DeleteNode {
                            graph_path: graph_name.clone(),
                            node_id: node_id.clone(),
                            deleted_node: node,
                        });
                    }

                    // Also look for edges to delete directly (when deleting edge variables)
                    let edge_ids_to_delete: Vec<String> = graph
                        .get_all_edges()
                        .iter()
                        .filter(|edge| edge.id == *var_name)
                        .map(|edge| edge.id.clone())
                        .collect();

                    for edge_id in edge_ids_to_delete {
                        // Get edge data before deleting for undo
                        let edge = if let Some(edge) = graph.get_edge(&edge_id) {
                            edge.clone()
                        } else {
                            log::error!("Edge {} not found for deletion", edge_id);
                            continue;
                        };

                        if let Err(e) = graph.remove_edge(&edge.id) {
                            log::error!("Failed to delete edge {}: {}", edge.id, e);
                            continue;
                        }

                        log::debug!("Deleted edge {}", edge.id);
                        deleted_count += 1;

                        // Add undo operation for edge
                        undo_operations.push(UndoOperation::DeleteEdge {
                            graph_path: graph_name.clone(),
                            edge_id: edge.id.clone(),
                            deleted_edge: edge,
                        });
                    }
                }
                _ => {
                    log::warn!(
                        "Complex expressions in DELETE not yet supported: {:?}",
                        expr
                    );
                }
            }
        }

        // Return the first undo operation if any
        let undo_op =
            undo_operations
                .into_iter()
                .next()
                .unwrap_or_else(|| UndoOperation::DeleteNode {
                    graph_path: graph_name,
                    node_id: "no_operations".to_string(),
                    deleted_node: Node {
                        id: "no_operations".to_string(),
                        labels: vec![],
                        properties: HashMap::new(),
                    },
                });

        Ok((undo_op, deleted_count))
    }
}
