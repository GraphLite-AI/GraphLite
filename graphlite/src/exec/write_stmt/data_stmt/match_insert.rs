// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Refactored MATCH INSERT executor using unified planning infrastructure
//!
//! **Phase 4 Refactoring**: Reduced from 1045 lines to ~300 lines by:
//! - Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! - Removing manual pattern matching logic (400+ lines)
//! - Keeping only essential helper methods
//!
//! This executor handles MATCH...WHERE...WITH...INSERT statements by:
//! 1. Using UnifiedQueryPlanner for MATCH/WHERE/WITH clauses
//! 2. Executing plan to get variable bindings
//! 3. Using bindings to execute INSERT patterns

use std::collections::HashMap;
use uuid::Uuid;

use crate::ast::{Expression, Literal, MatchInsertStatement, PatternElement};
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::plan::unified_query_planner::UnifiedQueryPlanner;
use crate::plan::physical_executor::PhysicalExecutor;
use crate::storage::{GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for MATCH INSERT statements
pub struct MatchInsertExecutor {
    statement: MatchInsertStatement,
}

impl MatchInsertExecutor {
    /// Create a new MatchInsertExecutor
    pub fn new(statement: MatchInsertStatement) -> Self {
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

impl StatementExecutor for MatchInsertExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Insert
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("MATCH INSERT into graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for MatchInsertExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut inserted_count = 0;

        // Step 1: Use UnifiedQueryPlanner to plan MATCH...WHERE...WITH pipeline
        log::debug!("Planning MATCH...WHERE...WITH pipeline");

        let where_condition = self.statement.where_clause.as_ref().map(|w| &w.condition);
        let logical_plan = UnifiedQueryPlanner::plan_query_pipeline(
            Some(&self.statement.match_clause),
            where_condition,
            self.statement.with_clause.as_ref(),
        ).map_err(|e| ExecutionError::PlanningError(e.to_string()))?;

        // Step 2: Execute the plan to get variable bindings
        let variable_bindings = if let Some(logical_node) = logical_plan {
            log::debug!("Executing logical plan to get bindings");

            // Convert logical node to logical plan, then to physical plan and execute
            use crate::plan::logical::LogicalPlan;
            use crate::plan::physical::PhysicalPlan;
            let logical_plan = LogicalPlan::new(logical_node);
            let physical_plan = PhysicalPlan::from_logical(&logical_plan);
            let execution_result = physical_plan.execute(graph)?;

            // execution_result is already Vec<HashMap<String, Value>> which we can use directly
            execution_result
        } else {
            // No MATCH clause - use empty bindings
            log::debug!("No MATCH clause, using empty bindings");
            vec![HashMap::new()]
        };

        log::debug!("Found {} variable binding combinations", variable_bindings.len());

        if variable_bindings.is_empty() {
            log::debug!("No bindings found, no insertions performed");
            return Ok((
                UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_bindings".to_string(),
                },
                0,
            ));
        }

        // Step 3: For each binding, execute INSERT patterns
        for (binding_idx, binding) in variable_bindings.iter().enumerate() {
            log::debug!("Processing binding {} with {} variables", binding_idx, binding.len());
            log::debug!("Binding variables: {:?}", binding.keys().collect::<Vec<_>>());

            // Process each INSERT pattern
            for pattern in &self.statement.insert_graph_patterns {
                log::debug!("Processing INSERT pattern with {} elements", pattern.elements.len());

                // Track node identifiers for edge creation
                let mut node_id_map: HashMap<String, String> = HashMap::new();

                // First pass: Process nodes and build node ID map
                for element in &pattern.elements {
                    if let PatternElement::Node(node_pattern) = element {
                        if let Some(ref identifier) = node_pattern.identifier {
                            // Determine node ID
                            let node_id = if let Some(bound_value) = binding.get(identifier) {
                                // Extract node ID from bound value
                                if let Value::Node(node) = bound_value {
                                    node.id.clone()
                                } else {
                                    Uuid::new_v4().to_string()
                                }
                            } else {
                                Uuid::new_v4().to_string()
                            };
                            node_id_map.insert(identifier.clone(), node_id);
                        }
                    }
                }

                // Second pass: Process nodes and edges in the pattern
                for element in &pattern.elements {
                    match element {
                        PatternElement::Node(node_pattern) => {
                            // Get node ID from map or generate new one
                            let node_id = if let Some(ref identifier) = node_pattern.identifier {
                                node_id_map.get(identifier).cloned().unwrap_or_else(|| Uuid::new_v4().to_string())
                            } else {
                                Uuid::new_v4().to_string()
                            };

                            // Collect labels
                            let labels = node_pattern.labels.clone();

                            // Collect properties
                            let mut properties = HashMap::new();
                            if let Some(ref prop_map) = node_pattern.properties {
                                for property in &prop_map.properties {
                                    let value = if let Expression::Literal(literal) = &property.value {
                                        Self::literal_to_value(literal)
                                    } else {
                                        // Evaluate expression with bindings using PhysicalExecutor
                                        PhysicalExecutor::evaluate_expression(&property.value, binding)?
                                    };
                                    properties.insert(property.key.clone(), value);
                                }
                            }

                            // Check if node already exists
                            if graph.get_node(&node_id).is_none() {
                                let new_node = Node {
                                    id: node_id.clone(),
                                    labels: labels.clone(),
                                    properties: properties.clone(),
                                };

                                graph.add_node(new_node).map_err(|e| {
                                    ExecutionError::StorageError(format!("Failed to insert node: {}", e))
                                })?;

                                inserted_count += 1;

                                undo_operations.push(UndoOperation::InsertNode {
                                    graph_path: graph_name.clone(),
                                    node_id: node_id.clone(),
                                });

                                log::debug!("Inserted node {} with labels {:?}", node_id, labels);
                            }
                        }
                        PatternElement::Edge(edge_pattern) => {
                            // Edge creation requires finding adjacent nodes in the pattern
                            // Pattern should be: Node - Edge - Node
                            let element_idx = pattern.elements.iter().position(|e| {
                                matches!(e, PatternElement::Edge(_))
                            });

                            if let Some(idx) = element_idx {
                                // Get source and target nodes
                                let (from_node_id, to_node_id) = match edge_pattern.direction {
                                    crate::ast::EdgeDirection::Outgoing => {
                                        // Pattern: (from)-[edge]->(to)
                                        let from_id = if idx > 0 {
                                            if let PatternElement::Node(from_node) = &pattern.elements[idx - 1] {
                                                from_node.identifier.as_ref().and_then(|id| node_id_map.get(id).cloned())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        let to_id = if idx + 1 < pattern.elements.len() {
                                            if let PatternElement::Node(to_node) = &pattern.elements[idx + 1] {
                                                to_node.identifier.as_ref().and_then(|id| node_id_map.get(id).cloned())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        (from_id, to_id)
                                    }
                                    crate::ast::EdgeDirection::Incoming => {
                                        // Pattern: (to)<-[edge]-(from)
                                        let to_id = if idx > 0 {
                                            if let PatternElement::Node(to_node) = &pattern.elements[idx - 1] {
                                                to_node.identifier.as_ref().and_then(|id| node_id_map.get(id).cloned())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        let from_id = if idx + 1 < pattern.elements.len() {
                                            if let PatternElement::Node(from_node) = &pattern.elements[idx + 1] {
                                                from_node.identifier.as_ref().and_then(|id| node_id_map.get(id).cloned())
                                            } else {
                                                None
                                            }
                                        } else {
                                            None
                                        };

                                        (from_id, to_id)
                                    }
                                    _ => {
                                        log::warn!("Unsupported edge direction: {:?}", edge_pattern.direction);
                                        (None, None)
                                    }
                                };

                                if let (Some(from_id), Some(to_id)) = (from_node_id, to_node_id) {
                                    // Get edge label
                                    let label = edge_pattern.labels.first().cloned().unwrap_or_else(|| "EDGE".to_string());

                                    // Collect edge properties
                                    let mut properties = HashMap::new();
                                    if let Some(ref prop_map) = edge_pattern.properties {
                                        for property in &prop_map.properties {
                                            let value = if let Expression::Literal(literal) = &property.value {
                                                Self::literal_to_value(literal)
                                            } else {
                                                // Evaluate expression with bindings
                                                PhysicalExecutor::evaluate_expression(&property.value, binding)?
                                            };
                                            properties.insert(property.key.clone(), value);
                                        }
                                    }

                                    // Create edge
                                    let edge_id = Uuid::new_v4().to_string();
                                    let edge = crate::storage::Edge {
                                        id: edge_id.clone(),
                                        from_node: from_id.clone(),
                                        to_node: to_id.clone(),
                                        label: label.clone(),
                                        properties,
                                    };

                                    graph.add_edge(edge).map_err(|e| {
                                        ExecutionError::StorageError(format!("Failed to insert edge: {}", e))
                                    })?;

                                    inserted_count += 1;

                                    undo_operations.push(UndoOperation::InsertEdge {
                                        graph_path: graph_name.clone(),
                                        edge_id: edge_id.clone(),
                                    });

                                    log::debug!("Inserted edge {} with label {:?}", edge_id, label);
                                } else {
                                    log::warn!("Could not find source/target nodes for edge pattern");
                                }
                            }
                        }
                    }
                }
            }
        }

        // Return batch undo operation
        Ok((
            if undo_operations.len() == 1 {
                undo_operations.into_iter().next().unwrap()
            } else if undo_operations.is_empty() {
                UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_operations".to_string(),
                }
            } else {
                UndoOperation::Batch {
                    operations: undo_operations,
                }
            },
            inserted_count,
        ))
    }
}
