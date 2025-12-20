// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{Expression, Literal, MatchDeleteStatement, PatternElement};
use crate::exec::with_clause_processor::WithClauseProcessor;
use crate::exec::write_engine::operations::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::functions::FunctionRegistry;
use crate::storage::{Edge, GraphCache, Node, Value};
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

    /// Process a single path pattern (node or relationship pattern)
    /// Returns (node_bindings, edge_bindings)
    fn match_path_pattern(
        graph: &GraphCache,
        pattern: &crate::ast::PathPattern,
    ) -> Result<(Vec<HashMap<String, Node>>, Vec<HashMap<String, Edge>>), ExecutionError> {
        let mut node_matches = Vec::new();
        let mut edge_matches = Vec::new();

        if pattern.elements.len() == 3 {
            // Handle relationship patterns like (u:User)-[p:PLAYED]->(g:Game)
            if let (
                Some(PatternElement::Node(source_pattern)),
                Some(PatternElement::Edge(edge_pattern)),
                Some(PatternElement::Node(target_pattern)),
            ) = (
                pattern.elements.first(),
                pattern.elements.get(1),
                pattern.elements.get(2),
            ) {
                let edges = graph.get_all_edges();
                for edge in edges {
                    // Check if edge matches the pattern
                    let edge_label_matches = if edge_pattern.labels.is_empty() {
                        true
                    } else {
                        edge_pattern.labels.contains(&edge.label)
                    };

                    if !edge_label_matches {
                        continue;
                    }

                    // Get source and target nodes
                    if let (Some(source_node), Some(target_node)) = (
                        graph.get_node(&edge.from_node),
                        graph.get_node(&edge.to_node),
                    ) {
                        // Check if source node matches pattern
                        let source_matches =
                            Self::node_matches_pattern(source_node, source_pattern);
                        let target_matches =
                            Self::node_matches_pattern(target_node, target_pattern);

                        if source_matches && target_matches {
                            let mut node_binding = HashMap::new();
                            let mut edge_binding = HashMap::new();

                            if let Some(ref source_id) = source_pattern.identifier {
                                node_binding.insert(source_id.clone(), source_node.clone());
                            }
                            if let Some(ref target_id) = target_pattern.identifier {
                                node_binding.insert(target_id.clone(), target_node.clone());
                            }
                            if let Some(ref edge_id) = edge_pattern.identifier {
                                edge_binding.insert(edge_id.clone(), edge.clone());
                            }

                            node_matches.push(node_binding);
                            edge_matches.push(edge_binding);
                        }
                    }
                }
            }
        } else if pattern.elements.len() == 1 {
            // Handle single node patterns
            if let Some(PatternElement::Node(node_pattern)) = pattern.elements.first() {
                let nodes = graph.get_all_nodes();
                for node in nodes {
                    if Self::node_matches_pattern(node, node_pattern) {
                        let mut binding = HashMap::new();
                        if let Some(ref identifier) = node_pattern.identifier {
                            binding.insert(identifier.clone(), node.clone());
                        }
                        node_matches.push(binding);
                    }
                }
            }
        }

        Ok((node_matches, edge_matches))
    }

    /// Check if a node matches a node pattern
    fn node_matches_pattern(node: &Node, node_pattern: &crate::ast::Node) -> bool {
        // Check labels
        if !node_pattern.labels.is_empty() {
            let has_required_label = node_pattern
                .labels
                .iter()
                .any(|pattern_label| node.labels.contains(pattern_label));
            if !has_required_label {
                return false;
            }
        }

        // Check properties
        if let Some(ref prop_map) = node_pattern.properties {
            for property in &prop_map.properties {
                if let Expression::Literal(literal) = &property.value {
                    let expected_value = Self::literal_to_value(literal);
                    if node.properties.get(&property.key) != Some(&expected_value) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Evaluate WHERE clause on a variable combination (nodes and edges)
    fn evaluate_where_clause_on_combination(
        node_combination: &HashMap<String, Node>,
        edge_combination: &HashMap<String, Edge>,
        where_clause: &crate::ast::WhereClause,
        computed_values: Option<&HashMap<String, Value>>,
    ) -> bool {
        if let Some(computed_values) = computed_values {
            WithClauseProcessor::evaluate_where_with_computed_values(where_clause, computed_values)
        } else {
            Self::evaluate_where_expression_on_combination(
                node_combination,
                edge_combination,
                &where_clause.condition,
            )
        }
    }

    /// Evaluate WHERE expression on a variable combination (nodes and edges)
    fn evaluate_where_expression_on_combination(
        node_combination: &HashMap<String, Node>,
        edge_combination: &HashMap<String, Edge>,
        expr: &Expression,
    ) -> bool {
        match expr {
            Expression::Binary(binary_op) => {
                let left_val = Self::evaluate_expression_on_combination(
                    node_combination,
                    edge_combination,
                    &binary_op.left,
                );
                let right_val = Self::evaluate_expression_on_combination(
                    node_combination,
                    edge_combination,
                    &binary_op.right,
                );

                match binary_op.operator {
                    // COMPARISON OPERATORS - NULL-aware for WHERE clause evaluation
                    // In WHERE clause, NULL comparisons evaluate to FALSE (exclude rows)
                    // This follows SQL/GQL three-valued logic where NULL is treated as FALSE in WHERE
                    crate::ast::Operator::GreaterThan => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l > r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l > r,
                            _ => false,
                        }
                    }
                    crate::ast::Operator::LessThan => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l < r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l < r,
                            _ => false,
                        }
                    }
                    crate::ast::Operator::GreaterEqual => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l >= r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l >= r,
                            _ => false,
                        }
                    }
                    crate::ast::Operator::LessEqual => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l <= r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l <= r,
                            _ => false,
                        }
                    }
                    crate::ast::Operator::Equal => {
                        match (left_val, right_val) {
                            // NULL = NULL is false in WHERE clause (SQL three-valued logic)
                            (None, _) | (_, None) => false,
                            (Some(l), Some(r)) => l == r,
                        }
                    }
                    crate::ast::Operator::NotEqual => {
                        match (left_val, right_val) {
                            // NULL != value is false in WHERE clause (SQL three-valued logic)
                            (None, _) | (_, None) => false,
                            (Some(l), Some(r)) => l != r,
                        }
                    }
                    crate::ast::Operator::And => {
                        Self::evaluate_where_expression_on_combination(
                            node_combination,
                            edge_combination,
                            &binary_op.left,
                        ) && Self::evaluate_where_expression_on_combination(
                            node_combination,
                            edge_combination,
                            &binary_op.right,
                        )
                    }
                    crate::ast::Operator::Or => {
                        Self::evaluate_where_expression_on_combination(
                            node_combination,
                            edge_combination,
                            &binary_op.left,
                        ) || Self::evaluate_where_expression_on_combination(
                            node_combination,
                            edge_combination,
                            &binary_op.right,
                        )
                    }
                    _ => false,
                }
            }
            _ => true,
        }
    }

    /// Evaluate an expression on a variable combination (nodes and edges)
    fn evaluate_expression_on_combination(
        node_combination: &HashMap<String, Node>,
        edge_combination: &HashMap<String, Edge>,
        expr: &Expression,
    ) -> Option<Value> {
        match expr {
            Expression::Variable(var) => {
                // First check nodes
                if let Some(node) = node_combination.get(&var.name) {
                    Some(Value::String(node.id.clone()))
                } else {
                    edge_combination
                        .get(&var.name)
                        .map(|edge| Value::String(edge.id.clone()))
                }
            }
            Expression::PropertyAccess(prop_access) => {
                // First check if the object is a node
                if let Some(node) = node_combination.get(&prop_access.object) {
                    node.properties.get(&prop_access.property).cloned()
                } else if let Some(edge) = edge_combination.get(&prop_access.object) {
                    // Then check if it's an edge
                    edge.properties.get(&prop_access.property).cloned()
                } else {
                    None
                }
            }
            Expression::Literal(literal) => Some(Self::literal_to_value(literal)),
            _ => None,
        }
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
        let prefix = if self.statement.detach {
            "MATCH DETACH "
        } else {
            "MATCH "
        };
        format!("{}DELETE nodes/edges in graph '{}'", prefix, graph_name)
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

        log::debug!(
            "MATCH-DELETE: Processing {} patterns",
            self.statement.match_clause.patterns.len()
        );

        // Step 1: Process MATCH clause patterns to find target variable combinations
        let mut all_node_matches = Vec::new();
        let mut all_edge_matches = Vec::new();

        for pattern in &self.statement.match_clause.patterns {
            let (node_matches, edge_matches) = Self::match_path_pattern(graph, pattern)?;
            all_node_matches.push(node_matches);
            all_edge_matches.push(edge_matches);
        }

        // Step 2: Generate Cartesian product of all pattern matches
        // Keep node and edge combinations paired together
        let mut combined_matches: Vec<(HashMap<String, Node>, HashMap<String, Edge>)> = Vec::new();

        if all_node_matches.len() == 1 {
            // Single pattern - pair node and edge matches directly
            let node_matches = all_node_matches.into_iter().next().unwrap_or_default();
            let edge_matches = all_edge_matches.into_iter().next().unwrap_or_default();

            // If we have edge matches, pair them with node matches
            if !edge_matches.is_empty() && node_matches.len() == edge_matches.len() {
                for (node_match, edge_match) in
                    node_matches.into_iter().zip(edge_matches.into_iter())
                {
                    combined_matches.push((node_match, edge_match));
                }
            } else if !edge_matches.is_empty() {
                // Edge matches without corresponding node matches (edge-only pattern)
                for edge_match in edge_matches {
                    combined_matches.push((HashMap::new(), edge_match));
                }
            } else {
                // Node matches without edge matches
                for node_match in node_matches {
                    combined_matches.push((node_match, HashMap::new()));
                }
            }
        } else if all_node_matches.len() > 1 {
            // Multiple patterns - generate Cartesian product
            let mut current_combined = vec![(HashMap::new(), HashMap::new())];

            for (node_matches, edge_matches) in all_node_matches
                .into_iter()
                .zip(all_edge_matches.into_iter())
            {
                let mut new_combined = Vec::new();

                for (existing_nodes, existing_edges) in &current_combined {
                    // Handle case where we have matching node and edge counts
                    if node_matches.len() == edge_matches.len() && !edge_matches.is_empty() {
                        for (node_match, edge_match) in node_matches.iter().zip(edge_matches.iter())
                        {
                            let mut new_node_combo = existing_nodes.clone();
                            let mut new_edge_combo = existing_edges.clone();

                            for (var, node) in node_match {
                                new_node_combo.insert(var.clone(), node.clone());
                            }
                            for (var, edge) in edge_match {
                                new_edge_combo.insert(var.clone(), edge.clone());
                            }

                            new_combined.push((new_node_combo, new_edge_combo));
                        }
                    } else {
                        // Handle separate node and edge matches
                        for node_match in &node_matches {
                            let mut new_node_combo = existing_nodes.clone();
                            for (var, node) in node_match {
                                new_node_combo.insert(var.clone(), node.clone());
                            }
                            new_combined.push((new_node_combo, existing_edges.clone()));
                        }
                        for edge_match in &edge_matches {
                            let mut new_edge_combo = existing_edges.clone();
                            for (var, edge) in edge_match {
                                new_edge_combo.insert(var.clone(), edge.clone());
                            }
                            new_combined.push((existing_nodes.clone(), new_edge_combo));
                        }
                    }
                }

                current_combined = new_combined;
            }

            combined_matches = current_combined;
        }

        if combined_matches.is_empty() {
            log::debug!("MATCH-DELETE: No matches found");
            return Ok((
                UndoOperation::DeleteNode {
                    graph_path: graph_name,
                    node_id: "no_matches".to_string(),
                    deleted_node: Node {
                        id: "no_matches".to_string(),
                        labels: vec![],
                        properties: HashMap::new(),
                    },
                },
                0,
            ));
        }

        log::debug!("MATCH-DELETE: Found {} matches", combined_matches.len());

        // Step 3: Process WITH clause if present and handle GROUP BY logic
        let filtered_combined = if let Some(ref with_clause) = self.statement.with_clause {
            let node_bindings: HashMap<String, Vec<Node>> = {
                let mut bindings = HashMap::new();
                for (node_combo, _) in &combined_matches {
                    for (var, node) in node_combo {
                        bindings
                            .entry(var.clone())
                            .or_insert_with(Vec::new)
                            .push(node.clone());
                    }
                }
                bindings
            };

            // Collect edges from our pattern matches for proper relationship aggregation
            let edges: Vec<Edge> = combined_matches
                .iter()
                .flat_map(|(_, edge_combo)| edge_combo.values().cloned())
                .collect();

            let temp_context = context
                .clone()
                .with_function_registry(Arc::new(FunctionRegistry::new()));
            let with_result = WithClauseProcessor::process_with_clause(
                with_clause,
                &node_bindings,
                &edges,
                &temp_context,
            )?;

            // Apply WITH clause filtering to combined matches
            // Only keep combinations where nodes appear in the WITH clause results
            let mut filtered_combinations = Vec::new();

            // Collect all node IDs that passed the WITH clause filter
            let mut qualifying_node_ids = std::collections::HashSet::new();

            // For aggregated queries, use group_results
            log::debug!(
                "DELETE DEBUG: WITH result - has_aggregation: {}, group_results: {}",
                with_result.has_aggregation,
                with_result.group_results.len()
            );
            if with_result.has_aggregation {
                log::debug!(
                    "MATCH_DELETE: Processing {} groups from WITH aggregation",
                    with_result.group_results.len()
                );
                for (i, group_result) in with_result.group_results.iter().enumerate() {
                    log::debug!(
                        "  Group {}: {} variables",
                        i,
                        group_result.variable_bindings.len()
                    );
                    for (var_name, nodes) in &group_result.variable_bindings {
                        log::debug!("    Variable '{}': {} nodes", var_name, nodes.len());
                        for node in nodes {
                            log::debug!("      Adding node {} for deletion", node.id);
                            qualifying_node_ids.insert(node.id.clone());
                        }
                    }
                }
                log::debug!(
                    "MATCH_DELETE: Total qualifying nodes: {}",
                    qualifying_node_ids.len()
                );
                log::debug!(
                    "DELETE DEBUG: Qualifying node IDs: {:?}",
                    qualifying_node_ids
                );
            } else {
                // For non-aggregated queries, use variable_bindings directly
                for nodes in with_result.variable_bindings.values() {
                    for node in nodes {
                        qualifying_node_ids.insert(node.id.clone());
                    }
                }
            }

            // Extract variables that are being deleted
            let mut delete_variables = std::collections::HashSet::new();
            for expr in &self.statement.expressions {
                if let Expression::Variable(var) = expr {
                    delete_variables.insert(&var.name);
                }
            }

            log::debug!("DELETE DEBUG: Variables to delete: {:?}", delete_variables);

            // Only keep combinations where the nodes being deleted are in qualifying set
            for (node_combo, edge_combo) in &combined_matches {
                let mut combination_qualifies = true;

                // Only check nodes that are being deleted, not all nodes in the combination
                for (var_name, node) in node_combo {
                    if delete_variables.contains(var_name) {
                        // This variable is being deleted, so check if it's in qualifying set
                        if !qualifying_node_ids.contains(&node.id) {
                            combination_qualifies = false;
                            break;
                        }
                    }
                    // Variables not being deleted don't need to be in the qualifying set
                }

                if combination_qualifies {
                    filtered_combinations.push((node_combo.clone(), edge_combo.clone()));
                }
            }

            filtered_combinations
        } else {
            // No WITH clause - apply WHERE clause directly on combinations
            combined_matches
                .into_iter()
                .filter(|(node_combo, edge_combo)| {
                    if let Some(ref where_clause) = self.statement.where_clause {
                        Self::evaluate_where_clause_on_combination(
                            node_combo,
                            edge_combo,
                            where_clause,
                            None,
                        )
                    } else {
                        true
                    }
                })
                .collect()
        };

        log::debug!(
            "MATCH-DELETE: {} combinations passed WHERE clause",
            filtered_combined.len()
        );
        log::debug!(
            "DELETE DEBUG: {} combinations passed filtering",
            filtered_combined.len()
        );

        // Step 5: Process DELETE expressions on filtered combinations
        for (node_combination, edge_combination) in &filtered_combined {
            for expr in &self.statement.expressions {
                match expr {
                    Expression::Variable(var) => {
                        let var_name = &var.name;

                        // First check if this variable refers to an edge
                        if let Some(edge_to_delete) = edge_combination.get(var_name) {
                            let edge_id = &edge_to_delete.id;

                            // Get the edge data before deleting it for undo
                            let edge = if let Some(edge) = graph.get_edge(edge_id) {
                                edge.clone()
                            } else {
                                log::debug!("Edge {} already deleted or not found", edge_id);
                                continue;
                            };

                            // Delete the edge
                            match graph.remove_edge(edge_id) {
                                Ok(_) => {
                                    deleted_count += 1;
                                }
                                Err(e) => {
                                    log::error!("Failed to delete edge {}: {}", edge_id, e);
                                    continue;
                                }
                            }

                            log::debug!(
                                "MATCH DELETE: Deleted edge {} matching variable {}",
                                edge_id,
                                var_name
                            );

                            // Add undo operation for edge
                            undo_operations.push(UndoOperation::DeleteEdge {
                                graph_path: graph_name.clone(),
                                edge_id: edge_id.clone(),
                                deleted_edge: edge,
                            });
                        }
                        // Then check if this variable refers to a node
                        else if let Some(node_to_delete) = node_combination.get(var_name) {
                            let node_id = &node_to_delete.id;

                            // Get the node data before deleting it for undo
                            let node = if let Some(node) = graph.get_node(node_id) {
                                node.clone()
                            } else {
                                log::debug!("Node {} already deleted or not found", node_id);
                                continue;
                            };

                            // Check for connected edges
                            let connected_edge_ids: Vec<String> = graph
                                .get_all_edges()
                                .iter()
                                .filter(|edge| {
                                    edge.from_node == *node_id || edge.to_node == *node_id
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
                                        log::debug!(
                                            "Edge {} already deleted or not found",
                                            edge_id
                                        );
                                        continue;
                                    };

                                    if let Err(e) = graph.remove_edge(&edge.id) {
                                        log::error!("Failed to remove edge {} during MATCH DETACH DELETE: {}", edge.id, e);
                                        continue;
                                    }

                                    log::debug!("MATCH DETACH DELETE: Removed edge {} during deletion of node {}", edge.id, node_id);

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
                            match graph.remove_node(node_id) {
                                Ok(_) => {
                                    deleted_count += 1;
                                }
                                Err(e) => {
                                    log::error!("Failed to delete node {}: {}", node_id, e);
                                    continue;
                                }
                            }

                            log::debug!(
                                "MATCH DELETE: Deleted node {} matching variable {}",
                                node_id,
                                var_name
                            );

                            // Add undo operation for node
                            undo_operations.push(UndoOperation::DeleteNode {
                                graph_path: graph_name.clone(),
                                node_id: node_id.clone(),
                                deleted_node: node,
                            });
                        }
                    }
                    _ => {
                        log::warn!(
                            "Complex expressions in MATCH DELETE not yet supported: {:?}",
                            expr
                        );
                    }
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
