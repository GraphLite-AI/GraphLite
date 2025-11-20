// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Transaction operation logging for rollback support
//!
//! This module provides a more efficient approach to transaction rollback
//! by logging individual operations rather than keeping full graph snapshots.

use std::collections::HashMap;

use super::state::TransactionId;
use crate::storage::{Edge, Node, Value};

/// Represents an operation that can be undone
#[derive(Debug, Clone)]
pub enum UndoOperation {
    /// A node was inserted - to undo, remove it
    InsertNode { graph_path: String, node_id: String },
    /// A node was updated - to undo, restore old properties
    UpdateNode {
        graph_path: String,
        node_id: String,
        old_properties: HashMap<String, Value>,
        old_labels: Vec<String>,
    },
    /// A node was deleted - to undo, restore it
    DeleteNode {
        graph_path: String,
        node_id: String,
        deleted_node: Node,
    },
    /// An edge was inserted - to undo, remove it
    InsertEdge { graph_path: String, edge_id: String },
    /// An edge was updated - to undo, restore old properties
    UpdateEdge {
        graph_path: String,
        edge_id: String,
        old_properties: HashMap<String, Value>,
        old_label: String,
    },
    /// An edge was deleted - to undo, restore it
    DeleteEdge {
        graph_path: String,
        edge_id: String,
        deleted_edge: Edge,
    },
    /// A batch of operations that must be undone together atomically
    /// Used for multi-property SET operations and other compound statements
    Batch { operations: Vec<UndoOperation> },
}

/// Transaction operation log for a single transaction
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.3.0 - Transaction undo log for ROLLBACK support
pub struct TransactionLog {
    /// The transaction this log belongs to
    pub transaction_id: TransactionId,
    /// Operations in reverse order (most recent first) for efficient rollback
    pub undo_operations: Vec<UndoOperation>,
    /// Total number of operations logged
    pub operation_count: usize,
    /// Memory usage estimate in bytes
    pub estimated_size_bytes: usize,
}

impl TransactionLog {
    /// Create a new empty transaction log
    pub fn new(transaction_id: TransactionId) -> Self {
        Self {
            transaction_id,
            undo_operations: Vec::new(),
            operation_count: 0,
            estimated_size_bytes: std::mem::size_of::<Self>(),
        }
    }

    /// Add an undo operation to the log
    pub fn log_operation(&mut self, operation: UndoOperation) {
        // Estimate memory usage
        let op_size = match &operation {
            UndoOperation::InsertNode {
                graph_path,
                node_id,
            } => std::mem::size_of::<UndoOperation>() + graph_path.len() + node_id.len(),
            UndoOperation::UpdateNode {
                graph_path,
                node_id,
                old_properties,
                old_labels,
            } => {
                let props_size = old_properties
                    .iter()
                    .map(|(k, v)| k.len() + estimate_value_size(v))
                    .sum::<usize>();
                let labels_size = old_labels.iter().map(|l| l.len()).sum::<usize>();
                std::mem::size_of::<UndoOperation>()
                    + graph_path.len()
                    + node_id.len()
                    + props_size
                    + labels_size
            }
            UndoOperation::DeleteNode {
                graph_path,
                node_id,
                deleted_node,
            } => {
                std::mem::size_of::<UndoOperation>()
                    + graph_path.len()
                    + node_id.len()
                    + estimate_node_size(deleted_node)
            }
            UndoOperation::InsertEdge {
                graph_path,
                edge_id,
            } => std::mem::size_of::<UndoOperation>() + graph_path.len() + edge_id.len(),
            UndoOperation::UpdateEdge {
                graph_path,
                edge_id,
                old_properties,
                old_label,
            } => {
                let props_size = old_properties
                    .iter()
                    .map(|(k, v)| k.len() + estimate_value_size(v))
                    .sum::<usize>();
                let label_size = old_label.len();
                std::mem::size_of::<UndoOperation>()
                    + graph_path.len()
                    + edge_id.len()
                    + props_size
                    + label_size
            }
            UndoOperation::DeleteEdge {
                graph_path,
                edge_id,
                deleted_edge,
            } => {
                std::mem::size_of::<UndoOperation>()
                    + graph_path.len()
                    + edge_id.len()
                    + estimate_edge_size(deleted_edge)
            }
            UndoOperation::Batch { operations } => {
                // For batch operations, sum up the size of all individual operations
                let mut total_size = std::mem::size_of::<UndoOperation>();
                for op in operations {
                    total_size += match op {
                        UndoOperation::InsertNode {
                            graph_path,
                            node_id,
                        } => {
                            std::mem::size_of::<UndoOperation>() + graph_path.len() + node_id.len()
                        }
                        UndoOperation::UpdateNode {
                            graph_path,
                            node_id,
                            old_properties,
                            old_labels,
                        } => {
                            let props_size = old_properties
                                .iter()
                                .map(|(k, v)| k.len() + estimate_value_size(v))
                                .sum::<usize>();
                            let labels_size = old_labels.iter().map(|l| l.len()).sum::<usize>();
                            std::mem::size_of::<UndoOperation>()
                                + graph_path.len()
                                + node_id.len()
                                + props_size
                                + labels_size
                        }
                        UndoOperation::DeleteNode {
                            graph_path,
                            node_id,
                            deleted_node,
                        } => {
                            std::mem::size_of::<UndoOperation>()
                                + graph_path.len()
                                + node_id.len()
                                + estimate_node_size(deleted_node)
                        }
                        UndoOperation::InsertEdge {
                            graph_path,
                            edge_id,
                        } => {
                            std::mem::size_of::<UndoOperation>() + graph_path.len() + edge_id.len()
                        }
                        UndoOperation::UpdateEdge {
                            graph_path,
                            edge_id,
                            old_properties,
                            old_label,
                        } => {
                            let props_size = old_properties
                                .iter()
                                .map(|(k, v)| k.len() + estimate_value_size(v))
                                .sum::<usize>();
                            let label_size = old_label.len();
                            std::mem::size_of::<UndoOperation>()
                                + graph_path.len()
                                + edge_id.len()
                                + props_size
                                + label_size
                        }
                        UndoOperation::DeleteEdge {
                            graph_path,
                            edge_id,
                            deleted_edge,
                        } => {
                            std::mem::size_of::<UndoOperation>()
                                + graph_path.len()
                                + edge_id.len()
                                + estimate_edge_size(deleted_edge)
                        }
                        UndoOperation::Batch { .. } => {
                            // Nested batches - just use base size to avoid infinite recursion
                            std::mem::size_of::<UndoOperation>()
                        }
                    };
                }
                total_size
            }
        };

        self.undo_operations.push(operation);
        self.operation_count += 1;
        self.estimated_size_bytes += op_size;
    }

    /// Get the operations in rollback order (most recent first)
    #[allow(dead_code)] // ROADMAP v0.3.0 - Rollback operation iteration for ROLLBACK execution
    pub fn get_rollback_operations(&self) -> impl Iterator<Item = &UndoOperation> {
        self.undo_operations.iter().rev()
    }

    /// Clear the log (used after commit or rollback)
    #[allow(dead_code)] // ROADMAP v0.3.0 - Clear transaction log after COMMIT/ROLLBACK completion
    pub fn clear(&mut self) {
        self.undo_operations.clear();
        self.operation_count = 0;
        self.estimated_size_bytes = std::mem::size_of::<Self>();
    }

    /// Check if the log is empty
    #[allow(dead_code)] // ROADMAP v0.3.0 - Check if transaction has any undo operations for ROLLBACK decision
    pub fn is_empty(&self) -> bool {
        self.undo_operations.is_empty()
    }

    /// Get statistics about the log
    #[allow(dead_code)] // ROADMAP v0.6.0 - Transaction log statistics for monitoring and observability
    pub fn stats(&self) -> TransactionLogStats {
        TransactionLogStats {
            transaction_id: self.transaction_id,
            operation_count: self.operation_count,
            estimated_size_bytes: self.estimated_size_bytes,
        }
    }
}

/// Statistics about a transaction log
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.6.0 - Transaction statistics for monitoring
pub struct TransactionLogStats {
    pub transaction_id: TransactionId,
    pub operation_count: usize,
    pub estimated_size_bytes: usize,
}

/// Estimate memory usage of a Value
fn estimate_value_size(value: &Value) -> usize {
    match value {
        Value::Null => 0,
        Value::Boolean(_) => 1,
        Value::Number(_) => 8,
        Value::String(s) => s.len(),
        Value::List(list) | Value::Array(list) => list.iter().map(estimate_value_size).sum(),
        Value::Vector(vec) => vec.len() * 4, // f32 is 4 bytes
        _ => 64, // Rough estimate for complex types like DateTime, TimeWindow, Path
    }
}

/// Estimate memory usage of a Node
fn estimate_node_size(node: &Node) -> usize {
    let id_size = node.id.len();
    let labels_size = node.labels.iter().map(|l| l.len()).sum::<usize>();
    let props_size = node
        .properties
        .iter()
        .map(|(k, v)| k.len() + estimate_value_size(v))
        .sum::<usize>();

    std::mem::size_of::<Node>() + id_size + labels_size + props_size
}

/// Estimate memory usage of an Edge
fn estimate_edge_size(edge: &Edge) -> usize {
    let id_size = edge.id.len();
    let from_size = edge.from_node.len();
    let to_size = edge.to_node.len();
    let label_size = edge.label.len();
    let props_size = edge
        .properties
        .iter()
        .map(|(k, v)| k.len() + estimate_value_size(v))
        .sum::<usize>();

    std::mem::size_of::<Edge>() + id_size + from_size + to_size + label_size + props_size
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_log_creation() {
        let txn_id = TransactionId::new();
        let log = TransactionLog::new(txn_id);

        assert_eq!(log.transaction_id, txn_id);
        assert!(log.is_empty());
        assert_eq!(log.operation_count, 0);
    }

    #[test]
    fn test_log_operations() {
        let txn_id = TransactionId::new();
        let mut log = TransactionLog::new(txn_id);

        // Log an insert operation
        log.log_operation(UndoOperation::InsertNode {
            graph_path: "/test_graph".to_string(),
            node_id: "node1".to_string(),
        });

        assert!(!log.is_empty());
        assert_eq!(log.operation_count, 1);
        assert!(log.estimated_size_bytes > std::mem::size_of::<TransactionLog>());
    }

    #[test]
    fn test_rollback_order() {
        let txn_id = TransactionId::new();
        let mut log = TransactionLog::new(txn_id);

        // Log operations in order: insert, update, delete
        log.log_operation(UndoOperation::InsertNode {
            graph_path: "/test_graph".to_string(),
            node_id: "node1".to_string(),
        });
        log.log_operation(UndoOperation::UpdateNode {
            graph_path: "/test_graph".to_string(),
            node_id: "node1".to_string(),
            old_properties: HashMap::new(),
            old_labels: vec![],
        });
        log.log_operation(UndoOperation::DeleteNode {
            graph_path: "/test_graph".to_string(),
            node_id: "node1".to_string(),
            deleted_node: Node {
                id: "node1".to_string(),
                labels: vec![],
                properties: HashMap::new(),
            },
        });

        // Rollback should be in reverse order: delete, update, insert
        let rollback_ops: Vec<_> = log.get_rollback_operations().collect();
        assert_eq!(rollback_ops.len(), 3);

        match rollback_ops[0] {
            UndoOperation::DeleteNode {
                graph_path,
                node_id,
                ..
            } => {
                assert_eq!(graph_path, "/test_graph");
                assert_eq!(node_id, "node1");
            }
            _ => panic!("Expected DeleteNode operation first"),
        }
        match rollback_ops[1] {
            UndoOperation::UpdateNode {
                graph_path,
                node_id,
                ..
            } => {
                assert_eq!(graph_path, "/test_graph");
                assert_eq!(node_id, "node1");
            }
            _ => panic!("Expected UpdateNode operation second"),
        }
        match rollback_ops[2] {
            UndoOperation::InsertNode {
                graph_path,
                node_id,
            } => {
                assert_eq!(graph_path, "/test_graph");
                assert_eq!(node_id, "node1");
            }
            _ => panic!("Expected InsertNode operation third"),
        }
    }
}
