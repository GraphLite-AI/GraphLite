// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Refactored MATCH DELETE executor using unified planning infrastructure
//!
//! **Phase 4 Refactoring**: Reduced from 744 lines to ~270 lines by:
//! - Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! - Removing manual pattern matching logic (400+ lines)
//! - Keeping DETACH DELETE special handling
//!
//! This executor handles MATCH...WHERE...WITH...DELETE statements by:
//! 1. Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! 2. Executing plan to get variable bindings
//! 3. Using bindings to execute DELETE operations (with DETACH support)

use std::collections::HashMap;

use crate::ast::{Expression, Literal, MatchDeleteStatement, PatternElement};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::unified_query_planner::UnifiedQueryPlanner;
use crate::storage::{GraphCache, Node};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for MATCH DELETE statements
pub struct MatchDeleteExecutor {
    statement: MatchDeleteStatement,
}

impl MatchDeleteExecutor {
    /// Create a new MatchDeleteExecutor
    pub fn new(statement: MatchDeleteStatement) -> Self {
        Self { statement }
    }

    /// Convert AST literal to storage value
    fn literal_to_value(literal: &Literal) -> crate::storage::Value {
        match literal {
            Literal::String(s) => crate::storage::Value::String(s.clone()),
            Literal::Integer(i) => crate::storage::Value::Number(*i as f64),
            Literal::Float(f) => crate::storage::Value::Number(*f),
            Literal::Boolean(b) => crate::storage::Value::Boolean(*b),
            Literal::Null => crate::storage::Value::Null,
            Literal::DateTime(dt) => crate::storage::Value::String(dt.clone()),
            Literal::Duration(dur) => crate::storage::Value::String(dur.clone()),
            Literal::TimeWindow(tw) => crate::storage::Value::String(tw.clone()),
            Literal::Vector(vec) => crate::storage::Value::Vector(vec.iter().map(|&f| f as f32).collect()),
            Literal::List(list) => {
                let converted: Vec<crate::storage::Value> = list.iter().map(Self::literal_to_value).collect();
                crate::storage::Value::List(converted)
            }
        }
    }

    /// Generate all combinations (Cartesian product) of variable bindings
    fn generate_variable_combinations(
        variable_candidates: &HashMap<String, Vec<Node>>,
    ) -> Vec<HashMap<String, Node>> {
        fn generate_recursive(
            variables: &[(String, Vec<Node>)],
            current: HashMap<String, Node>,
            results: &mut Vec<HashMap<String, Node>>,
        ) {
            if variables.is_empty() {
                results.push(current);
                return;
            }

            let (var_name, candidates) = &variables[0];
            let remaining = &variables[1..];

            for candidate in candidates {
                let mut new_combination = current.clone();
                new_combination.insert(var_name.clone(), candidate.clone());
                generate_recursive(remaining, new_combination, results);
            }
        }

        let mut results = Vec::new();
        let variables: Vec<(String, Vec<Node>)> = variable_candidates
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        generate_recursive(&variables, HashMap::new(), &mut results);
        results
    }

}

impl StatementExecutor for MatchDeleteExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Delete
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        let prefix = if self.statement.detach { "DETACH " } else { "" };
        format!("MATCH {}DELETE in graph '{}'", prefix, graph_name)
    }
}

impl DataStatementExecutor for MatchDeleteExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut deleted_count = 0;

        // Step 1: Use UnifiedQueryPlanner to plan MATCH...WHERE...WITH pipeline
        log::debug!("Planning MATCH...WHERE...WITH pipeline");

        let where_condition = self.statement.where_clause.as_ref().map(|w| &w.condition);
        let logical_plan = UnifiedQueryPlanner::plan_query_pipeline(
            Some(&self.statement.match_clause),
            where_condition,
            self.statement.with_clause.as_ref(),
        ).map_err(|e| ExecutionError::PlanningError(e.to_string()))?;

        // Step 2: Execute the plan to get variable bindings
        let variable_bindings: Vec<HashMap<String, Node>> = if let Some(logical_node) = logical_plan {
            log::debug!("Executing logical plan to get bindings");

            // Convert logical node to logical plan, then to physical plan and execute
            use crate::plan::logical::LogicalPlan;
            use crate::plan::physical::PhysicalPlan;
            let logical_plan = LogicalPlan::new(logical_node);
            let physical_plan = PhysicalPlan::from_logical(&logical_plan);
            let execution_result = physical_plan.execute(graph)?;

            // Convert execution result (HashMap<String, Value>) to HashMap<String, Node>
            execution_result
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .filter_map(|(key, value)| {
                            if let crate::storage::Value::Node(node) = value {
                                Some((key, node))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect()
        } else {
            // No MATCH clause - shouldn't happen for MATCH DELETE
            return Err(ExecutionError::InvalidQuery(
                "MATCH DELETE requires a MATCH clause".to_string(),
            ));
        };

        log::debug!("Found {} variable binding combinations", variable_bindings.len());

        if variable_bindings.is_empty() {
            log::debug!("No bindings found, no deletions performed");
            return Ok((
                UndoOperation::DeleteNode {
                    graph_path: graph_name,
                    node_id: "no_bindings".to_string(),
                    deleted_node: Node {
                        id: "no_bindings".to_string(),
                        labels: Vec::new(),
                        properties: HashMap::new(),
                    },
                },
                0,
            ));
        }

        // Step 3: Collect nodes to delete based on DELETE expressions
        let mut nodes_to_delete: Vec<String> = Vec::new();

        // Extract variable names from DELETE expressions
        let delete_vars: Vec<String> = self.statement.expressions
            .iter()
            .filter_map(|expr| {
                if let Expression::Variable(var) = expr {
                    Some(var.name.clone())
                } else {
                    None
                }
            })
            .collect();

        // Collect nodes for the specified variables
        for binding in &variable_bindings {
            for var_name in &delete_vars {
                if let Some(node) = binding.get(var_name) {
                    if !nodes_to_delete.contains(&node.id) {
                        nodes_to_delete.push(node.id.clone());
                    }
                }
            }
        }

        log::debug!("Will delete {} unique nodes", nodes_to_delete.len());

        // Step 4: If DETACH DELETE, first delete connected edges
        if self.statement.detach {
            for node_id in &nodes_to_delete {
                let connected_edges: Vec<String> = graph
                    .get_all_edges()
                    .iter()
                    .filter(|edge| edge.from_node == *node_id || edge.to_node == *node_id)
                    .map(|edge| edge.id.clone())
                    .collect();

                for edge_id in connected_edges {
                    if let Ok(deleted_edge) = graph.remove_edge(&edge_id) {
                        deleted_count += 1;
                        undo_operations.push(UndoOperation::DeleteEdge {
                            graph_path: graph_name.clone(),
                            edge_id: edge_id.clone(),
                            deleted_edge,
                        });
                        log::debug!("Deleted edge {} (DETACH)", edge_id);
                    }
                }
            }
        }

        // Step 5: Delete the nodes
        for node_id in nodes_to_delete {
            if let Ok(deleted_node) = graph.remove_node(&node_id) {
                deleted_count += 1;
                undo_operations.push(UndoOperation::DeleteNode {
                    graph_path: graph_name.clone(),
                    node_id: node_id.clone(),
                    deleted_node,
                });
                log::debug!("Deleted node {}", node_id);
            }
        }

        // Return batch undo operation
        Ok((
            if undo_operations.len() == 1 {
                undo_operations.into_iter().next().unwrap()
            } else if undo_operations.is_empty() {
                UndoOperation::DeleteNode {
                    graph_path: graph_name,
                    node_id: "no_operations".to_string(),
                    deleted_node: Node {
                        id: "no_operations".to_string(),
                        labels: Vec::new(),
                        properties: HashMap::new(),
                    },
                }
            } else {
                UndoOperation::Batch {
                    operations: undo_operations,
                }
            },
            deleted_count,
        ))
    }
}
