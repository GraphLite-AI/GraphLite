// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::ast::{Expression, MatchSetStatement, PatternElement, SetItem};
use crate::exec::with_clause_processor::WithClauseProcessor;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::functions::FunctionRegistry;
use crate::storage::{Edge, GraphCache, Node, Value};
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

    /// Evaluate an expression to a value, with access to matched node properties
    fn evaluate_expression(
        expr: &Expression,
        computed_values: Option<&HashMap<String, Value>>,
        combination: &HashMap<String, Node>,
        context: &ExecutionContext,
    ) -> Option<Value> {
        // First check if this is a variable from WITH clause computed values
        if let Expression::Variable(var) = expr {
            if let Some(computed_values) = computed_values {
                if let Some(value) = computed_values.get(&var.name) {
                    log::debug!(
                        "MATCH-SET: Using computed value for variable '{}': {:?}",
                        var.name,
                        value
                    );
                    return Some(value.clone());
                }
            }
        }

        // Try to evaluate using the combination (handles PropertyAccess, Literals, Binary operations)
        if let Some(value) = Self::evaluate_expression_on_combination(combination, expr) {
            log::debug!(
                "MATCH-SET: Evaluated expression on combination: {:?}",
                value
            );
            return Some(value);
        }

        // Fall back to context evaluation for function calls and other complex expressions
        match context.evaluate_simple_expression(expr) {
            Ok(value) => {
                log::debug!("MATCH-SET: Evaluated expression using context: {:?}", value);
                Some(value)
            }
            Err(e) => {
                log::warn!("MATCH-SET: Failed to evaluate expression: {}", e);
                None
            }
        }
    }

    /// Convert AST literal to storage value (still needed for WHERE clause evaluation)
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
            crate::ast::ast::Literal::Vector(vec) => {
                Value::Vector(vec.iter().map(|&f| f as f32).collect())
            }
            crate::ast::ast::Literal::List(list) => {
                let converted: Vec<Value> =
                    list.iter().map(|lit| Self::literal_to_value(lit)).collect();
                Value::List(converted)
            }
        }
    }

    /// Process a single path pattern (node or relationship pattern)
    /// Returns (node_bindings, edge_bindings)
    fn match_path_pattern(
        graph: &GraphCache,
        pattern: &crate::ast::ast::PathPattern,
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
                pattern.elements.get(0),
                pattern.elements.get(1),
                pattern.elements.get(2),
            ) {
                let edges = graph.get_all_edges();
                for edge in edges {
                    // Check if edge matches the pattern
                    let edge_label_matches = if edge_pattern.labels.is_empty() {
                        true
                    } else {
                        edge_pattern.labels.iter().any(|label| edge.label == *label)
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
                            Self::node_matches_pattern(&source_node, source_pattern);
                        let target_matches =
                            Self::node_matches_pattern(&target_node, target_pattern);

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
                    if Self::node_matches_pattern(&node, node_pattern) {
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
    fn node_matches_pattern(node: &Node, node_pattern: &crate::ast::ast::Node) -> bool {
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

    /// Evaluate WHERE clause on a variable combination
    fn evaluate_where_clause_on_combination(
        combination: &HashMap<String, Node>,
        where_clause: &crate::ast::ast::WhereClause,
        computed_values: Option<&HashMap<String, Value>>,
    ) -> bool {
        if let Some(computed_values) = computed_values {
            WithClauseProcessor::evaluate_where_with_computed_values(where_clause, computed_values)
        } else {
            Self::evaluate_where_expression_on_combination(combination, &where_clause.condition)
        }
    }

    /// Evaluate WHERE expression on a variable combination
    fn evaluate_where_expression_on_combination(
        combination: &HashMap<String, Node>,
        expr: &Expression,
    ) -> bool {
        match expr {
            Expression::Binary(binary_op) => {
                let left_val =
                    Self::evaluate_expression_on_combination(combination, &binary_op.left);
                let right_val =
                    Self::evaluate_expression_on_combination(combination, &binary_op.right);

                match binary_op.operator {
                    crate::ast::ast::Operator::GreaterThan => match (left_val, right_val) {
                        (Some(Value::Number(l)), Some(Value::Number(r))) => l > r,
                        _ => false,
                    },
                    crate::ast::ast::Operator::LessThan => match (left_val, right_val) {
                        (Some(Value::Number(l)), Some(Value::Number(r))) => l < r,
                        _ => false,
                    },
                    crate::ast::ast::Operator::GreaterEqual => match (left_val, right_val) {
                        (Some(Value::Number(l)), Some(Value::Number(r))) => l >= r,
                        _ => false,
                    },
                    crate::ast::ast::Operator::Equal => match (left_val, right_val) {
                        (Some(l), Some(r)) => l == r,
                        _ => false,
                    },
                    crate::ast::ast::Operator::And => {
                        // For AND, evaluate both sides as boolean expressions
                        let left_bool = Self::evaluate_where_expression_on_combination(
                            combination,
                            &binary_op.left,
                        );
                        let right_bool = Self::evaluate_where_expression_on_combination(
                            combination,
                            &binary_op.right,
                        );
                        left_bool && right_bool
                    }
                    crate::ast::ast::Operator::Or => {
                        // For OR, evaluate both sides as boolean expressions
                        let left_bool = Self::evaluate_where_expression_on_combination(
                            combination,
                            &binary_op.left,
                        );
                        let right_bool = Self::evaluate_where_expression_on_combination(
                            combination,
                            &binary_op.right,
                        );
                        left_bool || right_bool
                    }
                    _ => false,
                }
            }
            _ => true,
        }
    }

    /// Evaluate an expression on a variable combination
    fn evaluate_expression_on_combination(
        combination: &HashMap<String, Node>,
        expr: &Expression,
    ) -> Option<Value> {
        match expr {
            Expression::Variable(var) => {
                if let Some(node) = combination.get(&var.name) {
                    Some(Value::String(node.id.clone()))
                } else {
                    None
                }
            }
            Expression::PropertyAccess(prop_access) => {
                if let Some(node) = combination.get(&prop_access.object) {
                    node.properties.get(&prop_access.property).cloned()
                } else {
                    None
                }
            }
            Expression::Literal(literal) => Some(Self::literal_to_value(literal)),
            Expression::Binary(binary_expr) => {
                // Recursively evaluate left and right operands
                let left_val =
                    Self::evaluate_expression_on_combination(combination, &binary_expr.left)?;
                let right_val =
                    Self::evaluate_expression_on_combination(combination, &binary_expr.right)?;

                // Apply the binary operation
                use crate::ast::ast::Operator;
                match (&left_val, &binary_expr.operator, &right_val) {
                    // Numeric operations
                    (Value::Number(l), Operator::Plus, Value::Number(r)) => {
                        Some(Value::Number(l + r))
                    }
                    (Value::Number(l), Operator::Minus, Value::Number(r)) => {
                        Some(Value::Number(l - r))
                    }
                    (Value::Number(l), Operator::Star, Value::Number(r)) => {
                        Some(Value::Number(l * r))
                    }
                    (Value::Number(l), Operator::Slash, Value::Number(r)) => {
                        if r != &0.0 {
                            Some(Value::Number(l / r))
                        } else {
                            None // Division by zero
                        }
                    }
                    (Value::Number(l), Operator::Percent, Value::Number(r)) => {
                        Some(Value::Number(l % r))
                    }

                    // String concatenation
                    (Value::String(l), Operator::Plus, Value::String(r)) => {
                        Some(Value::String(format!("{}{}", l, r)))
                    }

                    // Comparison operations
                    (Value::Number(l), Operator::Equal, Value::Number(r)) => {
                        Some(Value::Boolean(l == r))
                    }
                    (Value::Number(l), Operator::NotEqual, Value::Number(r)) => {
                        Some(Value::Boolean(l != r))
                    }
                    (Value::Number(l), Operator::LessThan, Value::Number(r)) => {
                        Some(Value::Boolean(l < r))
                    }
                    (Value::Number(l), Operator::LessEqual, Value::Number(r)) => {
                        Some(Value::Boolean(l <= r))
                    }
                    (Value::Number(l), Operator::GreaterThan, Value::Number(r)) => {
                        Some(Value::Boolean(l > r))
                    }
                    (Value::Number(l), Operator::GreaterEqual, Value::Number(r)) => {
                        Some(Value::Boolean(l >= r))
                    }

                    (Value::String(l), Operator::Equal, Value::String(r)) => {
                        Some(Value::Boolean(l == r))
                    }
                    (Value::String(l), Operator::NotEqual, Value::String(r)) => {
                        Some(Value::Boolean(l != r))
                    }

                    (Value::Boolean(l), Operator::Equal, Value::Boolean(r)) => {
                        Some(Value::Boolean(l == r))
                    }
                    (Value::Boolean(l), Operator::NotEqual, Value::Boolean(r)) => {
                        Some(Value::Boolean(l != r))
                    }

                    // Logical operations
                    (Value::Boolean(l), Operator::And, Value::Boolean(r)) => {
                        Some(Value::Boolean(*l && *r))
                    }
                    (Value::Boolean(l), Operator::Or, Value::Boolean(r)) => {
                        Some(Value::Boolean(*l || *r))
                    }

                    _ => {
                        log::warn!(
                            "Unsupported binary operation: {:?} {:?} {:?}",
                            left_val,
                            binary_expr.operator,
                            right_val
                        );
                        None
                    }
                }
            }
            _ => None,
        }
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
        format!("MATCH SET properties in graph '{}'", graph_name)
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

        log::debug!(
            "MATCH-SET: Starting execute_modification with {} patterns",
            self.statement.match_clause.patterns.len()
        );
        log::debug!("MATCH-SET: SET items count: {}", self.statement.items.len());
        log::debug!(
            "MATCH-SET: Graph has {} nodes",
            graph.node_count().unwrap_or(0)
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
        let mut variable_combinations = Vec::new();
        let mut edge_combinations = Vec::new();

        if all_node_matches.len() == 1 {
            // Single pattern - use its matches directly
            variable_combinations = all_node_matches.into_iter().next().unwrap_or_default();
            edge_combinations = all_edge_matches.into_iter().next().unwrap_or_default();
        } else if all_node_matches.len() > 1 {
            // Multiple patterns - generate Cartesian product
            let mut node_combinations = vec![HashMap::new()];
            let mut edge_combo_list = vec![HashMap::new()];

            for (node_matches, edge_matches) in all_node_matches
                .into_iter()
                .zip(all_edge_matches.into_iter())
            {
                let mut new_node_combinations = Vec::new();
                let mut new_edge_combinations = Vec::new();

                for (existing_node_combo, existing_edge_combo) in
                    node_combinations.iter().zip(edge_combo_list.iter())
                {
                    for (node_match, edge_match) in node_matches.iter().zip(edge_matches.iter()) {
                        let mut new_node_combo = existing_node_combo.clone();
                        let mut new_edge_combo = existing_edge_combo.clone();

                        for (var, node) in node_match {
                            new_node_combo.insert(var.clone(), node.clone());
                        }
                        for (var, edge) in edge_match {
                            new_edge_combo.insert(var.clone(), edge.clone());
                        }

                        new_node_combinations.push(new_node_combo);
                        new_edge_combinations.push(new_edge_combo);
                    }
                }

                node_combinations = new_node_combinations;
                edge_combo_list = new_edge_combinations;
            }

            variable_combinations = node_combinations;
            edge_combinations = edge_combo_list;
        }

        if variable_combinations.is_empty() {
            log::debug!("MATCH-SET: No variable combinations found");
            return Ok((
                UndoOperation::UpdateNode {
                    graph_path: graph_name,
                    node_id: "no_matches".to_string(),
                    old_properties: HashMap::new(),
                    old_labels: vec![],
                },
                0,
            ));
        }

        log::debug!(
            "MATCH-SET: Found {} variable combinations",
            variable_combinations.len()
        );

        // Step 3: Process WITH clause if present
        let with_result = if let Some(ref with_clause) = self.statement.with_clause {
            let node_bindings: HashMap<String, Vec<Node>> = {
                let mut bindings = HashMap::new();
                for combination in &variable_combinations {
                    for (var, node) in combination {
                        bindings
                            .entry(var.clone())
                            .or_insert_with(Vec::new)
                            .push(node.clone());
                    }
                }
                bindings
            };

            // Collect edges from our pattern matches for proper relationship aggregation
            let edges: Vec<Edge> = edge_combinations
                .iter()
                .flat_map(|combo| combo.values().cloned())
                .collect();

            let temp_context = context
                .clone()
                .with_function_registry(Arc::new(FunctionRegistry::new()));
            Some(WithClauseProcessor::process_with_clause(
                with_clause,
                &node_bindings,
                &edges,
                &temp_context,
            )?)
        } else {
            None
        };

        // Step 4: Apply WHERE clause filtering and GROUP BY logic
        // Track both node and edge combinations together
        let combined_combinations: Vec<(HashMap<String, Node>, HashMap<String, Edge>)> =
            if variable_combinations.is_empty() {
                Vec::new()
            } else if edge_combinations.is_empty() {
                // Node-only patterns: create combinations with empty edge maps
                variable_combinations
                    .into_iter()
                    .map(|node_combo| (node_combo, HashMap::new()))
                    .collect()
            } else {
                // Both nodes and edges: zip them together
                variable_combinations
                    .into_iter()
                    .zip(edge_combinations.into_iter())
                    .collect()
            };

        let filtered_combinations: Vec<(HashMap<String, Node>, HashMap<String, Edge>)> =
            if let Some(ref with_result) = with_result {
                log::debug!(
                    "MATCH_SET: with_result present - has_aggregation: {}, group_results count: {}",
                    with_result.has_aggregation,
                    with_result.group_results.len()
                );
                log::debug!(
                    "MATCH_SET: Main variable_bindings has {} variables",
                    with_result.variable_bindings.len()
                );
                for (var, nodes) in &with_result.variable_bindings {
                    log::debug!("  Main binding '{}': {} nodes", var, nodes.len());
                }
                if with_result.has_aggregation {
                    if with_result.group_results.is_empty() {
                        // All groups were filtered out by WHERE clause - no mutations should happen
                        return Ok((
                            UndoOperation::UpdateNode {
                                graph_path: graph_name,
                                node_id: "filtered_out".to_string(),
                                old_properties: HashMap::new(),
                                old_labels: vec![],
                            },
                            0,
                        ));
                    }

                    // Handle GROUP BY aggregation - use groups already filtered by WITH clause
                    let mut filtered_groups = Vec::new();

                    for group_result in &with_result.group_results {
                        // Create combinations from this group's bindings
                        let mut group_combinations = vec![HashMap::new()];

                        for (var_name, nodes) in &group_result.variable_bindings {
                            let mut new_combinations = Vec::new();
                            for combination in &group_combinations {
                                for node in nodes {
                                    let mut new_combo = combination.clone();
                                    new_combo.insert(var_name.clone(), node.clone());
                                    new_combinations.push(new_combo);
                                }
                            }
                            group_combinations = new_combinations;
                        }

                        // For each node combination, find matching edge combinations
                        for node_combo in group_combinations {
                            // Find edge combinations that match this node combination
                            for (node_combination, edge_combination) in &combined_combinations {
                                if node_combination == &node_combo {
                                    filtered_groups
                                        .push((node_combo.clone(), edge_combination.clone()));
                                    break;
                                }
                            }
                        }
                    }

                    filtered_groups
                } else {
                    // No aggregation - apply WHERE clause normally
                    combined_combinations
                        .into_iter()
                        .filter(|(node_combination, _edge_combination)| {
                            if let Some(ref where_clause) = self.statement.where_clause {
                                let computed_values = Some(&with_result.computed_values);
                                Self::evaluate_where_clause_on_combination(
                                    node_combination,
                                    where_clause,
                                    computed_values,
                                )
                            } else {
                                true
                            }
                        })
                        .collect()
                }
            } else {
                // No WITH clause - apply WHERE clause directly
                combined_combinations
                    .into_iter()
                    .filter(|(node_combination, _edge_combination)| {
                        if let Some(ref where_clause) = self.statement.where_clause {
                            Self::evaluate_where_clause_on_combination(
                                node_combination,
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
            "MATCH-SET: {} combinations passed WHERE clause",
            filtered_combinations.len()
        );

        // Step 5: Apply SET operations to filtered combinations
        for (combo_idx, (combination, edge_combination)) in filtered_combinations.iter().enumerate()
        {
            log::debug!(
                "MATCH-SET: Processing combination {}: {:?}",
                combo_idx,
                combination.keys().collect::<Vec<_>>()
            );
            log::debug!(
                "MATCH-SET: Edge combination has {} variables",
                edge_combination.len()
            );

            // TRANSACTIONAL GUARANTEE: Pre-evaluate ALL property expressions for this combination
            // This ensures atomicity - if any expression fails, we abort before making ANY changes
            let mut evaluated_items = Vec::new();
            for item in &self.statement.items {
                match item {
                    SetItem::PropertyAssignment { property, value } => {
                        log::debug!(
                            "MATCH-SET: SET property assignment: {}.{} = {:?}",
                            property.object,
                            property.property,
                            value
                        );

                        // Get computed values from WITH clause result
                        let computed_values = with_result.as_ref().map(|wr| &wr.computed_values);

                        // Evaluate the new value - fail immediately if invalid (no partial updates!)
                        let new_value = Self::evaluate_expression(value, computed_values, combination, context)
                            .ok_or_else(|| ExecutionError::ExpressionError(
                                format!("Failed to evaluate MATCH SET property '{}': expression evaluation failed. Transaction aborted.", property.property)
                            ))?;

                        evaluated_items.push((property.clone(), new_value));
                    }
                    _ => {} // Handle other items separately
                }
            }

            // Capture old state for all nodes BEFORE making any changes (for rollback)
            let mut node_old_states: HashMap<String, (HashMap<String, Value>, Vec<String>)> =
                HashMap::new();
            for (_var_name, matched_node) in combination {
                if !node_old_states.contains_key(&matched_node.id) {
                    if let Some(node) = graph.get_node(&matched_node.id) {
                        node_old_states.insert(
                            matched_node.id.clone(),
                            (node.properties.clone(), node.labels.clone()),
                        );
                    }
                }
            }

            // Now that ALL expressions are valid, apply the changes
            for (property, new_value) in evaluated_items {
                // Check if this is a node property assignment
                let mut property_applied = false;
                for (var_name, matched_node) in combination {
                    log::debug!(
                        "MATCH-SET: Checking node variable '{}' with node '{}'",
                        var_name,
                        matched_node.id
                    );

                    // Check if this property assignment applies to the matched node variable
                    let node_property_matches = property.object == matched_node.id ||
                               property.object == *var_name ||
                               // Also check if the property object matches the pattern variable
                               self.statement.match_clause.patterns.iter()
                                   .any(|p| p.elements.iter()
                                       .any(|e| match e {
                                           PatternElement::Node(node_pattern) => {
                                               node_pattern.identifier.as_ref() == Some(&property.object)
                                           },
                                           _ => false,
                                       }));

                    if node_property_matches {
                        // Update the node
                        if let Some(node_mut) = graph.get_node_mut(&matched_node.id) {
                            node_mut.set_property(property.property.clone(), new_value.clone());
                            updated_count += 1;
                            property_applied = true;

                            // Add undo operation ONLY if we haven't already created one for this node
                            // Use the old state we captured BEFORE any changes
                            if let Some((old_properties, old_labels)) =
                                node_old_states.get(&matched_node.id)
                            {
                                // Only add undo if we haven't already for this node
                                if !undo_operations.iter().any(|op| {
                                    if let UndoOperation::UpdateNode { node_id, .. } = op {
                                        node_id == &matched_node.id
                                    } else {
                                        false
                                    }
                                }) {
                                    undo_operations.push(UndoOperation::UpdateNode {
                                        graph_path: graph_name.clone(),
                                        node_id: matched_node.id.clone(),
                                        old_properties: old_properties.clone(),
                                        old_labels: old_labels.clone(),
                                    });
                                }
                            }
                        }
                    }
                }

                // Check if this is an edge property assignment
                if !property_applied {
                    for (var_name, matched_edge) in edge_combination {
                        log::debug!(
                            "MATCH-SET: Checking edge variable '{}' with edge '{}'",
                            var_name,
                            matched_edge.id
                        );

                        // Check if this property assignment applies to the matched edge variable
                        let edge_property_matches = property.object == matched_edge.id ||
                                   property.object == *var_name ||
                                   // Also check if the property object matches the pattern variable
                                   self.statement.match_clause.patterns.iter()
                                       .any(|p| p.elements.iter()
                                           .any(|e| match e {
                                               PatternElement::Edge(edge_pattern) => {
                                                   edge_pattern.identifier.as_ref() == Some(&property.object)
                                               },
                                               _ => false,
                                           }));

                        if edge_property_matches {
                            // Get old property value for undo
                            let old_properties = if let Some(edge) =
                                graph.get_edge(&matched_edge.id)
                            {
                                let mut old_props = HashMap::new();
                                if let Some(old_val) = edge.properties.get(&property.property) {
                                    old_props.insert(property.property.clone(), old_val.clone());
                                }
                                old_props
                            } else {
                                HashMap::new()
                            };

                            // Update the edge
                            if let Some(edge_mut) = graph.get_edge_mut(&matched_edge.id) {
                                edge_mut
                                    .properties
                                    .insert(property.property.clone(), new_value.clone());
                                updated_count += 1;
                                property_applied = true;

                                // Add undo operation for edge update
                                undo_operations.push(UndoOperation::UpdateEdge {
                                    graph_path: graph_name.clone(),
                                    edge_id: matched_edge.id.clone(),
                                    old_properties,
                                    old_label: matched_edge.label.clone(),
                                });
                            }
                        }
                    }
                }

                if !property_applied {
                    log::warn!(
                        "MATCH-SET: Property assignment {}.{} did not match any variables",
                        property.object,
                        property.property
                    );
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
                            "Variable assignment in MATCH SET not yet fully supported: {} = {:?}",
                            variable,
                            value
                        );
                    }
                    SetItem::LabelAssignment { variable, labels } => {
                        // Handle label assignment for matched nodes
                        for (var_name, matched_node) in combination {
                            if self.statement.match_clause.patterns.iter().any(|p| {
                                p.elements.iter().any(|e| match e {
                                    PatternElement::Node(node_pattern) => {
                                        node_pattern.identifier.as_ref() == Some(variable)
                                    }
                                    _ => false,
                                })
                            }) && var_name == variable
                            {
                                // Add new labels to the node
                                if let Some(node_mut) = graph.get_node_mut(&matched_node.id) {
                                    // Extract labels from LabelExpression
                                    for term in &labels.terms {
                                        for factor in &term.factors {
                                            if let crate::ast::ast::LabelFactor::Identifier(
                                                new_label,
                                            ) = factor
                                            {
                                                if !node_mut.labels.contains(new_label) {
                                                    node_mut.labels.push(new_label.clone());
                                                    log::debug!(
                                                        "MATCH SET: Added label {} to node {}",
                                                        new_label,
                                                        matched_node.id
                                                    );
                                                    updated_count += 1;
                                                }
                                            }
                                        }
                                    }

                                    // Add undo operation ONLY if we haven't already created one for this node
                                    // Use the old state we captured BEFORE any changes
                                    if let Some((old_properties, old_labels)) =
                                        node_old_states.get(&matched_node.id)
                                    {
                                        if !undo_operations.iter().any(|op| {
                                            if let UndoOperation::UpdateNode { node_id, .. } = op {
                                                node_id == &matched_node.id
                                            } else {
                                                false
                                            }
                                        }) {
                                            undo_operations.push(UndoOperation::UpdateNode {
                                                graph_path: graph_name.clone(),
                                                node_id: matched_node.id.clone(),
                                                old_properties: old_properties.clone(),
                                                old_labels: old_labels.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
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
