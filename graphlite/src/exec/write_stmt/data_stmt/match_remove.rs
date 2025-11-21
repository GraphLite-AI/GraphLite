// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::ast::{
    Expression, LabelFactor, Literal, MatchRemoveStatement, PatternElement, RemoveItem,
};
use crate::exec::with_clause_processor::WithClauseProcessor;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::functions::FunctionRegistry;
use crate::storage::{Edge, GraphCache, Node, Value};
use crate::txn::{state::OperationType, UndoOperation};

/// Executor for MATCH REMOVE statements
pub struct MatchRemoveExecutor {
    statement: MatchRemoveStatement,
}

impl MatchRemoveExecutor {
    /// Create a new MatchRemoveExecutor
    pub fn new(statement: MatchRemoveStatement) -> Self {
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
                let converted: Vec<Value> =
                    list.iter().map(Self::literal_to_value).collect();
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

                // COMPARISON OPERATORS - NULL-aware for WHERE clause evaluation
                // In WHERE clause, NULL comparisons evaluate to FALSE (exclude rows)
                // This follows SQL/GQL three-valued logic where NULL is treated as FALSE in WHERE
                match binary_op.operator {
                    crate::ast::ast::Operator::GreaterThan => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l > r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l > r,
                            _ => false,
                        }
                    }
                    crate::ast::ast::Operator::LessThan => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l < r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l < r,
                            _ => false,
                        }
                    }
                    crate::ast::ast::Operator::GreaterEqual => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l >= r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l >= r,
                            _ => false,
                        }
                    }
                    crate::ast::ast::Operator::LessEqual => {
                        match (left_val, right_val) {
                            // NULL comparison returns false in WHERE clause
                            (None, _) | (_, None) => false,
                            (Some(Value::Number(l)), Some(Value::Number(r))) => l <= r,
                            (Some(Value::String(l)), Some(Value::String(r))) => l <= r,
                            _ => false,
                        }
                    }
                    crate::ast::ast::Operator::Equal => {
                        match (&left_val, &right_val) {
                            // NULL = NULL is false in WHERE clause (SQL three-valued logic)
                            (None, _) | (_, None) => false,
                            (Some(l), Some(r)) => l == r,
                        }
                    }
                    crate::ast::ast::Operator::NotEqual => {
                        match (&left_val, &right_val) {
                            // NULL != value is false in WHERE clause (SQL three-valued logic)
                            (None, _) | (_, None) => false,
                            (Some(l), Some(r)) => l != r,
                        }
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
                combination.get(&var.name).map(|node| Value::String(node.id.clone()))
            }
            Expression::PropertyAccess(prop_access) => {
                if let Some(node) = combination.get(&prop_access.object) {
                    node.properties.get(&prop_access.property).cloned()
                } else {
                    None
                }
            }
            Expression::Literal(literal) => Some(Self::literal_to_value(literal)),
            _ => None,
        }
    }
}

impl StatementExecutor for MatchRemoveExecutor {
    fn operation_type(&self) -> OperationType {
        OperationType::Remove
    }

    fn operation_description(&self, context: &ExecutionContext) -> String {
        let graph_name = context
            .get_graph_name()
            .unwrap_or_else(|_| "unknown".to_string());
        format!("MATCH REMOVE properties/labels in graph '{}'", graph_name)
    }
}

impl DataStatementExecutor for MatchRemoveExecutor {
    fn execute_modification(
        &self,
        graph: &mut GraphCache,
        context: &mut ExecutionContext,
    ) -> Result<(UndoOperation, usize), ExecutionError> {
        let graph_name = context.get_graph_name()?;
        let mut undo_operations = Vec::new();
        let mut removed_count = 0;

        log::debug!(
            "MATCH-REMOVE: Processing {} patterns",
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
            log::debug!("MATCH-REMOVE: No variable combinations found");
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
            "MATCH-REMOVE: Found {} variable combinations",
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
        let filtered_combinations: Vec<HashMap<String, Node>> =
            if let Some(ref with_result) = with_result {
                if with_result.has_aggregation && !with_result.group_results.is_empty() {
                    // Handle GROUP BY aggregation - use groups already filtered by WITH clause
                    let mut filtered_groups = Vec::new();

                    for group_result in &with_result.group_results {
                        // WITH clause has already filtered the groups, so all groups here should be processed
                        // For aggregated groups, we need to apply the mutation to the representative nodes
                        // of each group, not all possible combinations

                        // Create combinations from this group's bindings
                        // Each group may have multiple variables, and each variable may have multiple nodes
                        // We need to generate all combinations within this group
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

                        filtered_groups.extend(group_combinations);
                    }

                    filtered_groups
                } else {
                    // No aggregation - apply WHERE clause normally
                    variable_combinations
                        .into_iter()
                        .filter(|combination| {
                            if let Some(ref where_clause) = self.statement.where_clause {
                                let computed_values = Some(&with_result.computed_values);
                                Self::evaluate_where_clause_on_combination(
                                    combination,
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
                variable_combinations
                    .into_iter()
                    .filter(|combination| {
                        if let Some(ref where_clause) = self.statement.where_clause {
                            let result = Self::evaluate_where_clause_on_combination(
                                combination,
                                where_clause,
                                None,
                            );
                            log::debug!(
                                "DEBUG: WHERE clause evaluation for combination {:?}: {}",
                                combination
                                    .iter()
                                    .map(|(k, v)| (k.clone(), v.properties.get("name").cloned()))
                                    .collect::<Vec<_>>(),
                                result
                            );
                            result
                        } else {
                            true
                        }
                    })
                    .collect()
            };

        log::debug!(
            "MATCH-REMOVE: {} combinations passed WHERE clause",
            filtered_combinations.len()
        );

        // Step 5: Apply REMOVE operations to filtered combinations

        for combination in filtered_combinations {
            for (var_name, matched_node) in combination {
                for item in &self.statement.items {
                    match item {
                        RemoveItem::Property(property_access) => {
                            // Check if this property removal applies to the matched variable
                            if property_access.object == var_name {
                                // Get old property value and labels for undo
                                let (old_properties, old_labels, has_property) =
                                    if let Some(node) = graph.get_node(&matched_node.id) {
                                        let mut old_props = HashMap::new();
                                        let has_prop = if let Some(old_val) =
                                            node.properties.get(&property_access.property)
                                        {
                                            old_props.insert(
                                                property_access.property.clone(),
                                                old_val.clone(),
                                            );
                                            true
                                        } else {
                                            false
                                        };
                                        (old_props, node.labels.clone(), has_prop)
                                    } else {
                                        (HashMap::new(), Vec::new(), false)
                                    };

                                if has_property {
                                    // Remove the property
                                    if let Some(node_mut) = graph.get_node_mut(&matched_node.id) {
                                        node_mut.remove_property(&property_access.property);
                                        log::debug!("DEBUG: MATCH REMOVE: Removed property {} from node {} (name: {:?})", 
                                           property_access.property, matched_node.id, matched_node.properties.get("name"));
                                        log::debug!(
                                            "DEBUG: Node after removal - has score: {:?}",
                                            node_mut.properties.contains_key("score")
                                        );
                                        log::debug!(
                                            "MATCH REMOVE: Removed property {} from node {}",
                                            property_access.property,
                                            matched_node.id
                                        );
                                        removed_count += 1;

                                        // Add undo operation
                                        undo_operations.push(UndoOperation::UpdateNode {
                                            graph_path: graph_name.clone(),
                                            node_id: matched_node.id.clone(),
                                            old_properties,
                                            old_labels,
                                        });
                                    }
                                }
                            }
                        }
                        RemoveItem::Label { variable, labels } => {
                            // Handle label removal for matched nodes
                            if self.statement.match_clause.patterns.iter().any(|p| {
                                p.elements.iter().any(|e| match e {
                                    PatternElement::Node(node_pattern) => {
                                        node_pattern.identifier.as_ref() == Some(variable)
                                    }
                                    _ => false,
                                })
                            }) {
                                // Get original labels for undo
                                let old_labels =
                                    if let Some(node) = graph.get_node(&matched_node.id) {
                                        node.labels.clone()
                                    } else {
                                        continue;
                                    };

                                let mut removed_any = false;

                                // Extract labels to remove
                                for term in &labels.terms {
                                    for factor in &term.factors {
                                        if let LabelFactor::Identifier(label_name) = factor {
                                            if let Some(node_mut) =
                                                graph.get_node_mut(&matched_node.id)
                                            {
                                                let original_len = node_mut.labels.len();
                                                node_mut.labels.retain(|l| l != label_name);
                                                if node_mut.labels.len() < original_len {
                                                    log::debug!("MATCH REMOVE: Removed label {} from node {}", 
                                                           label_name, matched_node.id);
                                                    removed_any = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                if removed_any {
                                    removed_count += 1;

                                    // Add undo operation
                                    undo_operations.push(UndoOperation::UpdateNode {
                                        graph_path: graph_name.clone(),
                                        node_id: matched_node.id.clone(),
                                        old_properties: HashMap::new(),
                                        old_labels,
                                    });
                                }
                            }
                        }
                        RemoveItem::Variable(variable) => {
                            log::warn!(
                                "Variable removal in MATCH REMOVE not yet supported: {}",
                                variable
                            );
                            // This would typically remove variable bindings from execution context
                        }
                    }
                }
            }
        }

        // Return the first undo operation if any
        let undo_op =
            undo_operations
                .into_iter()
                .next()
                .unwrap_or_else(|| UndoOperation::UpdateNode {
                    graph_path: graph_name,
                    node_id: "no_operations".to_string(),
                    old_properties: HashMap::new(),
                    old_labels: vec![],
                });

        Ok((undo_op, removed_count))
    }
}
