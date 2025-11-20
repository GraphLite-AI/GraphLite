// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use crate::ast::ast::{SetItem, SetStatement};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::storage::GraphCache;
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for SET statements
pub struct SetExecutor {
    statement: SetStatement,
}

impl SetExecutor {
    /// Create a new SetExecutor
    pub fn new(statement: SetStatement) -> Self {
        Self { statement }
    }
}

impl StatementExecutor for SetExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Set
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("SET properties in graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for SetExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut updated_count = 0;

        // TRANSACTIONAL GUARANTEE: Pre-evaluate ALL property expressions before making ANY changes
        // This ensures that if any expression fails, we fail the entire SET operation atomically
        let mut evaluated_properties = Vec::new();
        for item in &self.statement.items {
            match item {
                SetItem::PropertyAssignment { property, value } => {
                    // Evaluate the value - fail immediately if invalid (no partial updates!)
                    let new_value = context.evaluate_simple_expression(value).map_err(|e| {
                        ExecutionError::ExpressionError(format!(
                            "Failed to evaluate SET property '{}': {}. Transaction aborted.",
                            property.property, e
                        ))
                    })?;
                    evaluated_properties.push((property.clone(), new_value));
                }
                _ => {} // Handle other item types separately
            }
        }

        // Now that ALL expressions are valid, apply the changes
        for (property, new_value) in evaluated_properties {
            let var_name = &property.object;

            // Find and update nodes with this variable identifier
            // This is a simplified approach - in reality, would use execution context
            let node_ids_to_update: Vec<String> = graph
                .get_all_nodes()
                .iter()
                .filter(|node| node.id == *var_name || node.labels.contains(var_name))
                .map(|node| node.id.clone())
                .collect();

            for node_id in node_ids_to_update {
                // Get ALL old properties and labels for undo (need full state for rollback)
                let (old_properties, old_labels) = if let Some(node) = graph.get_node(&node_id) {
                    (node.properties.clone(), node.labels.clone())
                } else {
                    (HashMap::new(), Vec::new())
                };

                // Update the node
                if let Some(node_mut) = graph.get_node_mut(&node_id) {
                    node_mut.set_property(property.property.clone(), new_value.clone());
                    log::debug!(
                        "Set property {} on node {} to {:?}",
                        property.property,
                        node_id,
                        new_value
                    );
                    updated_count += 1;

                    // Add undo operation
                    undo_operations.push(UndoOperation::UpdateNode {
                        graph_path: graph_name.clone(),
                        node_id: node_id.clone(),
                        old_properties,
                        old_labels,
                    });
                }
            }
        }

        // Handle other SET item types (TODO: these should also be transactional)
        for item in &self.statement.items {
            match item {
                SetItem::PropertyAssignment { .. } => {
                    // Already handled above
                }
                SetItem::VariableAssignment { variable, value } => {
                    log::warn!(
                        "Variable assignment in SET not yet fully supported: {} = {:?}",
                        variable,
                        value
                    );
                }
                SetItem::LabelAssignment { variable, labels } => {
                    log::warn!(
                        "Label assignment in SET not yet fully supported: {} {:?}",
                        variable,
                        labels
                    );
                }
            }
        }

        // Return all undo operations as a batch for transactional rollback
        let undo_op = if undo_operations.is_empty() {
            UndoOperation::UpdateNode {
                graph_path: graph_name,
                node_id: "no_operations".to_string(),
                old_properties: HashMap::new(),
                old_labels: vec![],
            }
        } else if undo_operations.len() == 1 {
            // Single operation - return it directly
            undo_operations.into_iter().next().unwrap()
        } else {
            // Multiple operations - return as batch for atomic undo
            UndoOperation::Batch {
                operations: undo_operations,
            }
        };

        Ok((undo_op, updated_count))
    }
}
