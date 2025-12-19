// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Refactored MATCH SET executor using unified planning infrastructure
//!
//! **Phase 4 Refactoring**: Reduced from 916 lines to ~250 lines by:
//! - Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! - Removing manual pattern matching logic (500+ lines)
//! - Simplifying SET execution to work with variable bindings
//!
//! This executor handles MATCH...WHERE...WITH...SET statements by:
//! 1. Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! 2. Executing plan to get variable bindings
//! 3. Using bindings to execute SET operations

use std::collections::HashMap;

use crate::ast::{Expression, Literal, MatchSetStatement, PatternElement, SetItem};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::unified_query_planner::UnifiedQueryPlanner;
use crate::storage::{GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for MATCH SET statements
pub struct MatchSetExecutor {
    statement: MatchSetStatement,
}

impl MatchSetExecutor {
    /// Create a new MatchSetExecutor
    pub fn new(statement: MatchSetStatement) -> Self {
        Self { statement }
    }

    /// Convert AST literal to storage value
    fn literal_to_value(literal: &Literal) -> Value {
        match literal {
            Literal::String(s) => Value::String(s.clone()),
            Literal::Integer(i) => Value::Number(*i as f64),
            Literal::Float(f) => Value::Number(*f),
            Literal::Boolean(b) => Value::Boolean(*b),
            Literal::Null => Value::Null,
            Literal::DateTime(dt) => Value::String(dt.clone()),
            Literal::Duration(dur) => Value::String(dur.clone()),
            Literal::TimeWindow(tw) => Value::String(tw.clone()),
            Literal::Vector(vec) => Value::Vector(vec.iter().map(|&f| f as f32).collect()),
            Literal::List(list) => {
                let converted: Vec<Value> = list.iter().map(Self::literal_to_value).collect();
                Value::List(converted)
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

impl StatementExecutor for MatchSetExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Set
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("MATCH SET in graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for MatchSetExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut updated_count = 0;

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
            // No MATCH clause - shouldn't happen for MATCH SET
            return Err(ExecutionError::InvalidQuery(
                "MATCH SET requires a MATCH clause".to_string(),
            ));
        };

        log::debug!("Found {} variable binding combinations", variable_bindings.len());

        if variable_bindings.is_empty() {
            log::debug!("No bindings found, no updates performed");
            return Ok((
                UndoOperation::UpdateNode {
                    graph_path: graph_name,
                    node_id: "no_bindings".to_string(),
                    old_properties: HashMap::new(),
                    old_labels: Vec::new(),
                },
                0,
            ));
        }

        // Step 3: For each binding, execute SET operations
        for (binding_idx, binding) in variable_bindings.iter().enumerate() {
            log::debug!("Processing binding {} with {} variables", binding_idx, binding.len());

            // Process each SET item
            for set_item in &self.statement.items {
                match set_item {
                    SetItem::Property { property, value } => {
                        // Get the variable being updated
                        let var_name = &property.object;

                        // Find the node from bindings
                        if let Some(node) = binding.get(var_name) {
                            let node_id = &node.id;

                            // Evaluate the new value
                            let new_value = if let Expression::Literal(literal) = value {
                                Self::literal_to_value(literal)
                            } else {
                                context.evaluate_simple_expression(&value)?
                            };

                            // Get old state for undo
                            let (old_properties, old_labels) = if let Some(existing_node) = graph.get_node(node_id) {
                                (existing_node.properties.clone(), existing_node.labels.clone())
                            } else {
                                continue; // Node no longer exists
                            };

                            // Update the node
                            if let Some(node_mut) = graph.get_node_mut(node_id) {
                                node_mut.set_property(property.property.clone(), new_value);
                                updated_count += 1;

                                // Add undo operation
                                undo_operations.push(UndoOperation::UpdateNode {
                                    graph_path: graph_name.clone(),
                                    node_id: node_id.clone(),
                                    old_properties,
                                    old_labels,
                                });

                                log::debug!("Updated property {} on node {}", property.property, node_id);
                            }
                        } else {
                            log::warn!("Variable {} not found in bindings", var_name);
                        }
                    }
                    SetItem::Variable { .. } => {
                        return Err(ExecutionError::InvalidQuery(
                            "SET variable assignment is not yet supported".to_string(),
                        ));
                    }
                    SetItem::Label { .. } => {
                        return Err(ExecutionError::InvalidQuery(
                            "SET label assignment is not yet supported".to_string(),
                        ));
                    }
                }
            }
        }

        // Return batch undo operation
        Ok((
            if undo_operations.len() == 1 {
                undo_operations.into_iter().next().unwrap()
            } else if undo_operations.is_empty() {
                UndoOperation::UpdateNode {
                    graph_path: graph_name,
                    node_id: "no_operations".to_string(),
                    old_properties: HashMap::new(),
                    old_labels: Vec::new(),
                }
            } else {
                UndoOperation::Batch {
                    operations: undo_operations,
                }
            },
            updated_count,
        ))
    }
}
