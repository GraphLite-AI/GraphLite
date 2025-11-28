// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;

use crate::ast::InsertStatement;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::insert_planner::InsertPlanner;
use crate::plan::physical::PhysicalPlan;
use crate::storage::GraphCache;
use crate::txn::UndoOperation;

/// Executor for INSERT statements using planned execution
pub struct PlannedInsertExecutor {
    statement: InsertStatement,
}

impl PlannedInsertExecutor {
    /// Create a new PlannedInsertExecutor
    pub fn new(statement: InsertStatement) -> Self {
        Self { statement }
    }

    /// Update text indexes for a newly inserted node (automatic indexing)
    ///
    /// This function finds all text indexes that match the node's labels and updates them
    /// with the node's text property values.
    fn update_text_indexes_for_node(
        _node_id: &str,
        labels: &[String],
        _properties: &HashMap<String, crate::storage::Value>,
        context: &mut ExecutionContext,
    ) {
        // Get index manager from storage manager
        let index_manager = match context.storage_manager.as_ref() {
            Some(sm) => match sm.get_index_manager() {
                Some(mgr) => mgr,
                None => {
                    log::debug!("No index manager available for automatic indexing");
                    return;
                }
            },
            None => {
                log::debug!("No storage manager available for automatic indexing");
                return;
            }
        };

        // For each label on this node, find matching text indexes
        for label in labels {
            log::debug!("Checking for text indexes on label '{}'", label);

            // Find all indexes for this label
            let matching_indexes = index_manager.find_indexes_for_label(label);

            // Text indexes not supported in GraphLite - this is a stub
            #[allow(unused_variables)]
            for _index_name in matching_indexes {
                // Text indexing not supported in GraphLite - no-op
                log::debug!("Text index automatic indexing skipped (not supported in GraphLite)");
            }
        }
    }
}

impl StatementExecutor for PlannedInsertExecutor {
    fn operation_type(&self) -> crate::txn::state::OperationType {
        crate::txn::state::OperationType::Insert
    }

    fn operation_description(&self, _context: &ExecutionContext) -> String {
        // Count nodes and edges in the patterns
        let mut node_count = 0;
        let mut edge_count = 0;

        for pattern in &self.statement.graph_patterns {
            for element in &pattern.elements {
                match element {
                    crate::ast::PatternElement::Node(_) => node_count += 1,
                    crate::ast::PatternElement::Edge(_) => edge_count += 1,
                }
            }
        }

        // Generate meaningful message based on what's being created
        match (node_count, edge_count) {
            (0, 0) => "INSERT 0".to_string(),
            (1, 0) => "Created 1 node".to_string(),
            (n, 0) => format!("Created {} nodes", n),
            (0, 1) => "Created 1 relationship".to_string(),
            (0, e) => format!("Created {} relationships", e),
            (1, 1) => "Created 1 node, 1 relationship".to_string(),
            (1, e) => format!("Created 1 node, {} relationships", e),
            (n, 1) => format!("Created {} nodes, 1 relationship", n),
            (n, e) => format!("Created {} nodes, {} relationships", n, e),
        }
    }
}

impl DataStatementExecutor for PlannedInsertExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        // Step 1: Logical planning
        let mut logical_planner = InsertPlanner::new();
        let logical_plan = logical_planner
            .plan_insert(&self.statement)
            .map_err(|e| ExecutionError::RuntimeError(format!("Logical planning error: {}", e)))?;

        log::debug!(
            "Logical plan created with {} variables",
            logical_plan.variables.len()
        );

        // Step 2: Physical planning
        let physical_plan = PhysicalPlan::from_logical(&logical_plan);

        log::debug!("Physical plan created");

        // Step 3: Execute physical plan
        let mut rows_affected = 0usize;
        let mut undo_operations = Vec::new();

        // Get the graph path for undo operations
        let graph_path = context.get_graph_name().unwrap_or_else(|_| String::new()); // Fall back to empty string if no graph context

        match &physical_plan.root {
            crate::plan::physical::PhysicalNode::Insert {
                node_creations,
                edge_creations,
                ..
            } => {
                // Execute node creations
                for node_creation in node_creations {
                    // Convert expression properties to storage values
                    let mut properties = HashMap::new();
                    for (key, expr) in &node_creation.properties {
                        // Use ExecutionContext's evaluate_simple_expression to handle literals and function calls
                        match context.evaluate_simple_expression(expr) {
                            Ok(value) => {
                                properties.insert(key.clone(), value);
                            }
                            Err(e) => {
                                log::warn!("Failed to evaluate property expression for '{}': {}. Skipping property.", key, e);
                            }
                        }
                    }

                    let node = crate::storage::Node {
                        id: node_creation.storage_id.clone(),
                        labels: node_creation.labels.clone(),
                        properties,
                    };

                    // Add node to graph
                    let node_id = node_creation.storage_id.clone();
                    let node_labels = node_creation.labels.clone();
                    let node_props = node.properties.clone();

                    match graph.add_node(node) {
                        Ok(_) => {
                            log::debug!("Successfully added node '{}' to graph", node_id);
                            rows_affected += 1;

                            // AUTOMATIC INDEXING: Update text indexes for this node
                            Self::update_text_indexes_for_node(
                                &node_id,
                                &node_labels,
                                &node_props,
                                context,
                            );

                            // Add undo operation for transaction management
                            undo_operations.push(UndoOperation::InsertNode {
                                graph_path: graph_path.clone(),
                                node_id,
                            });
                        }
                        Err(crate::storage::types::GraphError::NodeAlreadyExists(_)) => {
                            log::info!(
                                "Node '{}' already exists, skipping duplicate",
                                node_creation.storage_id
                            );
                            // Add warning about duplicate insertion
                            let warning_msg = format!("Duplicate node detected: Node with identical properties already exists (node_id: {})", node_creation.storage_id);
                            context.add_warning(warning_msg);
                        }
                        Err(e) => {
                            return Err(ExecutionError::RuntimeError(format!(
                                "Failed to add node '{}': {}",
                                node_creation.storage_id, e
                            )));
                        }
                    }
                }

                // Execute edge creations
                for edge_creation in edge_creations {
                    // Convert expression properties to storage values
                    let mut properties = HashMap::new();
                    for (key, expr) in &edge_creation.properties {
                        // Use ExecutionContext's evaluate_simple_expression to handle literals and function calls
                        match context.evaluate_simple_expression(expr) {
                            Ok(value) => {
                                properties.insert(key.clone(), value);
                            }
                            Err(e) => {
                                log::warn!("Failed to evaluate property expression for '{}': {}. Skipping property.", key, e);
                            }
                        }
                    }

                    let edge = crate::storage::Edge {
                        id: edge_creation.storage_id.clone(),
                        from_node: edge_creation.from_node_id.clone(),
                        to_node: edge_creation.to_node_id.clone(),
                        label: edge_creation.label.clone(),
                        properties,
                    };

                    // Add edge to graph
                    match graph.add_edge(edge) {
                        Ok(_) => {
                            log::debug!(
                                "Successfully added edge '{}' to graph",
                                edge_creation.storage_id
                            );
                            rows_affected += 1;

                            // Add undo operation for transaction management
                            undo_operations.push(UndoOperation::InsertEdge {
                                graph_path: graph_path.clone(),
                                edge_id: edge_creation.storage_id.clone(),
                            });
                        }
                        Err(crate::storage::types::GraphError::EdgeAlreadyExists(_)) => {
                            log::info!(
                                "Edge '{}' already exists, skipping duplicate",
                                edge_creation.storage_id
                            );
                        }
                        Err(e) => {
                            return Err(ExecutionError::RuntimeError(format!(
                                "Failed to add edge '{}': {}",
                                edge_creation.storage_id, e
                            )));
                        }
                    }
                }

                log::debug!(
                    "Planned INSERT completed: {} nodes, {} edges",
                    node_creations.len(),
                    edge_creations.len()
                );
            }
            _ => {
                return Err(ExecutionError::RuntimeError(
                    "Invalid physical plan for INSERT".to_string(),
                ));
            }
        }

        // Return a composite undo operation (for now, just return the first one or create a composite)
        let composite_undo = if undo_operations.is_empty() {
            // No operations were performed
            UndoOperation::InsertNode {
                graph_path: graph_path.clone(),
                node_id: "dummy".to_string(),
            }
        } else {
            // Return the first operation (the framework handles multiple operations via transaction logs)
            undo_operations.into_iter().next().unwrap()
        };

        Ok((composite_undo, rows_affected))
    }
}
