// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::ast::ast::{Expression, Literal, MatchInsertStatement, PatternElement};
use crate::exec::with_clause_processor::WithClauseProcessor;
use crate::exec::write_stmt::data_stmt::DataStatementExecutor;
use crate::exec::write_stmt::{ExecutionContext, StatementExecutor};
use crate::exec::ExecutionError;
use crate::functions::FunctionRegistry;
use crate::storage::{Edge, GraphCache, Node, Value};
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
                let converted: Vec<Value> =
                    list.iter().map(|lit| Self::literal_to_value(lit)).collect();
                Value::List(converted)
            }
        }
    }

    /// Match a path pattern (including relationships) and return all binding combinations
    fn match_path_pattern(
        graph: &GraphCache,
        pattern: &crate::ast::ast::PathPattern,
    ) -> Result<Vec<HashMap<String, Node>>, ExecutionError> {
        let mut matches = Vec::new();

        // Handle patterns like (u:User)-[p:PLAYED]->(g:Game)
        if pattern.elements.len() == 3 {
            if let (
                Some(PatternElement::Node(source_pattern)),
                Some(PatternElement::Edge(edge_pattern)),
                Some(PatternElement::Node(target_pattern)),
            ) = (
                pattern.elements.get(0),
                pattern.elements.get(1),
                pattern.elements.get(2),
            ) {
                // Find all edges matching the edge pattern
                let matching_edges: Vec<&Edge> = graph
                    .get_all_edges()
                    .into_iter()
                    .filter(|edge| {
                        // Check if edge labels match
                        if !edge_pattern.labels.is_empty()
                            && !edge_pattern.labels.contains(&edge.label)
                        {
                            return false;
                        }

                        // Check if edge properties match
                        if let Some(ref prop_map) = edge_pattern.properties {
                            for property in &prop_map.properties {
                                if let Expression::Literal(literal) = &property.value {
                                    let expected_value = Self::literal_to_value(&literal);
                                    if edge.properties.get(&property.key) != Some(&expected_value) {
                                        return false;
                                    }
                                }
                            }
                        }
                        true
                    })
                    .collect();

                // For each matching edge, check if source and target nodes match their patterns
                for edge in matching_edges {
                    // Get source and target nodes
                    if let (Some(source_node), Some(target_node)) = (
                        graph.get_node(&edge.from_node),
                        graph.get_node(&edge.to_node),
                    ) {
                        // Check if source node matches source pattern
                        let source_matches =
                            Self::node_matches_pattern(source_node, source_pattern);
                        let target_matches =
                            Self::node_matches_pattern(target_node, target_pattern);

                        if source_matches && target_matches {
                            let mut binding = HashMap::new();

                            // Bind source node if it has an identifier
                            if let Some(ref identifier) = source_pattern.identifier {
                                binding.insert(identifier.clone(), source_node.clone());
                            }

                            // Bind target node if it has an identifier
                            if let Some(ref identifier) = target_pattern.identifier {
                                binding.insert(identifier.clone(), target_node.clone());
                            }

                            // Create a pseudo-node for the edge binding if needed
                            if let Some(ref identifier) = edge_pattern.identifier {
                                // Create a node representation of the edge for WITH clause processing
                                let edge_node = Node {
                                    id: edge.id.clone(),
                                    labels: vec![edge.label.clone()],
                                    properties: edge.properties.clone(),
                                };
                                binding.insert(identifier.clone(), edge_node);
                            }

                            matches.push(binding);
                        }
                    }
                }

                return Ok(matches);
            }
        }

        // Fallback to single node pattern matching for simpler patterns
        if pattern.elements.len() == 1 {
            if let Some(PatternElement::Node(node_pattern)) = pattern.elements.get(0) {
                if let Some(ref identifier) = node_pattern.identifier {
                    let matching_nodes: Vec<Node> = graph
                        .get_all_nodes()
                        .into_iter()
                        .filter(|node| Self::node_matches_pattern(node, node_pattern))
                        .cloned()
                        .collect();

                    for node in matching_nodes {
                        let mut binding = HashMap::new();
                        binding.insert(identifier.clone(), node);
                        matches.push(binding);
                    }
                }
            }
        }

        Ok(matches)
    }

    /// Check if a node matches a node pattern
    fn node_matches_pattern(node: &Node, node_pattern: &crate::ast::ast::Node) -> bool {
        // Check if labels match
        if !node_pattern.labels.is_empty()
            && !node_pattern
                .labels
                .iter()
                .any(|label| node.labels.contains(label))
        {
            return false;
        }

        // Check if properties match
        if let Some(ref prop_map) = node_pattern.properties {
            for property in &prop_map.properties {
                if let Expression::Literal(literal) = &property.value {
                    let expected_value = Self::literal_to_value(&literal);
                    if node.properties.get(&property.key) != Some(&expected_value) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Generate all combinations (Cartesian product) of variable bindings
    fn generate_variable_combinations(
        variable_candidates: &HashMap<String, Vec<Node>>,
    ) -> Vec<HashMap<String, Node>> {
        if variable_candidates.is_empty() {
            return vec![];
        }

        let variables: Vec<&String> = variable_candidates.keys().collect();
        let mut combinations = vec![];

        // Generate Cartesian product recursively
        fn generate_recursive(
            variables: &[&String],
            variable_candidates: &HashMap<String, Vec<Node>>,
            current_binding: HashMap<String, Node>,
            combinations: &mut Vec<HashMap<String, Node>>,
        ) {
            if variables.is_empty() {
                combinations.push(current_binding);
                return;
            }

            let var = variables[0];
            let remaining = &variables[1..];

            if let Some(candidates) = variable_candidates.get(var) {
                for node in candidates {
                    let mut new_binding = current_binding.clone();
                    new_binding.insert(var.clone(), node.clone());
                    generate_recursive(remaining, variable_candidates, new_binding, combinations);
                }
            }
        }

        generate_recursive(
            &variables,
            variable_candidates,
            HashMap::new(),
            &mut combinations,
        );
        combinations
    }

    /// Evaluate WHERE clause against a variable combination
    fn evaluate_where_clause_on_combination(
        combination: &HashMap<String, Node>,
        where_clause: &crate::ast::ast::WhereClause,
    ) -> bool {
        Self::evaluate_where_expression_on_combination(combination, &where_clause.condition)
    }

    /// Evaluate WHERE expression against a variable combination
    fn evaluate_where_expression_on_combination(
        combination: &HashMap<String, Node>,
        expr: &crate::ast::ast::Expression,
    ) -> bool {
        match expr {
            crate::ast::ast::Expression::Binary(binary_op) => {
                let left_val =
                    Self::evaluate_expression_on_combination(combination, &binary_op.left);
                let right_val =
                    Self::evaluate_expression_on_combination(combination, &binary_op.right);

                match &binary_op.operator {
                    crate::ast::ast::Operator::Equal => left_val == right_val,
                    crate::ast::ast::Operator::NotEqual => left_val != right_val,
                    crate::ast::ast::Operator::GreaterThan => {
                        if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                            l > r
                        } else {
                            false
                        }
                    }
                    crate::ast::ast::Operator::LessThan => {
                        if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                            l < r
                        } else {
                            false
                        }
                    }
                    crate::ast::ast::Operator::And => {
                        Self::evaluate_where_expression_on_combination(combination, &binary_op.left)
                            && Self::evaluate_where_expression_on_combination(
                                combination,
                                &binary_op.right,
                            )
                    }
                    crate::ast::ast::Operator::Or => {
                        Self::evaluate_where_expression_on_combination(combination, &binary_op.left)
                            || Self::evaluate_where_expression_on_combination(
                                combination,
                                &binary_op.right,
                            )
                    }
                    _ => {
                        log::warn!(
                            "Unsupported operator in WHERE clause: {:?}",
                            binary_op.operator
                        );
                        false
                    }
                }
            }
            _ => {
                log::warn!("Unsupported WHERE expression type in combination evaluation");
                true
            }
        }
    }

    /// Evaluate expression on combination to get value
    fn evaluate_expression_on_combination(
        combination: &HashMap<String, Node>,
        expr: &crate::ast::ast::Expression,
    ) -> Value {
        match expr {
            crate::ast::ast::Expression::Variable(var) => combination
                .get(&var.name)
                .map(|node| Value::String(node.id.clone()))
                .unwrap_or(Value::Null),
            crate::ast::ast::Expression::PropertyAccess(prop_access) => {
                log::debug!(
                    "INSERT PropertyAccess: Looking for {}.{} in combination with variables: {:?}",
                    prop_access.object,
                    prop_access.property,
                    combination.keys().collect::<Vec<_>>()
                );
                if let Some(node) = combination.get(&prop_access.object) {
                    log::debug!(
                        "INSERT PropertyAccess: Found node {} with properties: {:?}",
                        prop_access.object,
                        node.properties.keys().collect::<Vec<_>>()
                    );
                    let result = node
                        .properties
                        .get(&prop_access.property)
                        .cloned()
                        .unwrap_or(Value::Null);
                    log::debug!(
                        "INSERT PropertyAccess: {}.{} = {:?}",
                        prop_access.object,
                        prop_access.property,
                        result
                    );
                    result
                } else {
                    log::debug!(
                        "INSERT PropertyAccess: Variable '{}' not found in combination",
                        prop_access.object
                    );
                    Value::Null
                }
            }
            crate::ast::ast::Expression::Literal(literal) => Self::literal_to_value(&literal),
            _ => {
                log::warn!("Unsupported expression type in combination evaluation");
                Value::Null
            }
        }
    }

    /// Extract properties with variable substitution
    fn extract_properties(
        prop_map: &crate::ast::ast::PropertyMap,
        variable_bindings: &HashMap<String, Node>,
        context: &ExecutionContext,
    ) -> HashMap<String, Value> {
        let mut properties = HashMap::new();

        for property in &prop_map.properties {
            let value = match &property.value {
                Expression::Variable(var) => {
                    // Try to substitute from variable bindings
                    if let Some(bound_node) = variable_bindings.get(&var.name) {
                        // Check if this is a computed value (virtual node with Computed label)
                        if bound_node.labels.contains(&"Computed".to_string()) {
                            // This is a computed value from WITH clause
                            bound_node
                                .properties
                                .get("value")
                                .cloned()
                                .unwrap_or(Value::Null)
                        } else if let Some(bound_value) = bound_node.properties.get(&property.key) {
                            bound_value.clone()
                        } else {
                            // Try to use the variable name as a direct property
                            bound_node
                                .properties
                                .get(&var.name)
                                .cloned()
                                .unwrap_or_else(|| Value::String(format!("bound_{}", var.name)))
                        }
                    } else {
                        Value::String(var.name.clone())
                    }
                }
                Expression::PropertyAccess(prop_access) => {
                    // Handle property access like node.property or computed_value.field
                    if let Some(bound_node) = variable_bindings.get(&prop_access.object) {
                        bound_node
                            .properties
                            .get(&prop_access.property)
                            .cloned()
                            .unwrap_or(Value::Null)
                    } else {
                        Value::Null
                    }
                }
                Expression::Literal(literal) => Self::literal_to_value(literal),
                Expression::FunctionCall(_) => {
                    // Use ExecutionContext to evaluate function calls
                    match context.evaluate_simple_expression(&property.value) {
                        Ok(val) => val,
                        Err(e) => {
                            log::warn!("Failed to evaluate function call for property '{}': {}. Skipping property.", property.key, e);
                            continue;
                        }
                    }
                }
                _ => {
                    // Try to use ExecutionContext's evaluate_simple_expression for other expression types
                    match context.evaluate_simple_expression(&property.value) {
                        Ok(val) => val,
                        Err(e) => {
                            log::warn!("Failed to evaluate expression for property '{}': {}. Skipping property.", property.key, e);
                            continue;
                        }
                    }
                }
            };
            properties.insert(property.key.clone(), value);
        }

        properties
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

        // Step 1: Execute MATCH clause to find bindings
        log::debug!("Executing MATCH clause: {:?}", self.statement.match_clause);

        // First collect all possible matches for each variable
        let mut variable_candidates: HashMap<String, Vec<Node>> = HashMap::new();

        // Match nodes based on the MATCH clause patterns
        log::debug!(
            "Processing {} MATCH patterns",
            self.statement.match_clause.patterns.len()
        );
        for (pattern_idx, pattern) in self.statement.match_clause.patterns.iter().enumerate() {
            log::debug!(
                "Processing pattern {}: {} elements",
                pattern_idx,
                pattern.elements.len()
            );

            // Use new pattern matching for relationship patterns, fallback for others
            if pattern.elements.len() == 3 {
                // Try relationship pattern matching
                log::debug!("Using relationship pattern matching for 3-element pattern");
                let relationship_matches = Self::match_path_pattern(graph, pattern)?;
                log::debug!(
                    "Relationship pattern matching found {} matches",
                    relationship_matches.len()
                );
                for binding in relationship_matches {
                    for (var_name, node) in binding {
                        log::debug!(
                            "Adding relationship match: variable '{}' -> node '{}'",
                            var_name,
                            node.id
                        );
                        variable_candidates
                            .entry(var_name)
                            .or_insert_with(Vec::new)
                            .push(node);
                    }
                }
            } else {
                // Original single-node pattern matching for backward compatibility
                log::debug!(
                    "Using single-node pattern matching for {}-element pattern",
                    pattern.elements.len()
                );
                for (element_idx, element) in pattern.elements.iter().enumerate() {
                    log::debug!(
                        "Processing element {} in pattern {}",
                        element_idx,
                        pattern_idx
                    );
                    if let PatternElement::Node(node_pattern) = element {
                        if let Some(ref identifier) = node_pattern.identifier {
                            log::debug!(
                                "Looking for nodes matching variable '{}' with labels {:?}",
                                identifier,
                                node_pattern.labels
                            );

                            // Find ALL matching nodes in graph
                            let matching_nodes: Vec<Node> = graph
                                .get_all_nodes()
                                .into_iter()
                                .filter(|node| {
                                    // Check if labels match
                                    if !node_pattern.labels.is_empty()
                                        && !node_pattern
                                            .labels
                                            .iter()
                                            .any(|label| node.labels.contains(label))
                                    {
                                        return false;
                                    }

                                    // Check if properties match
                                    if let Some(ref prop_map) = node_pattern.properties {
                                        for property in &prop_map.properties {
                                            if let Expression::Literal(literal) = &property.value {
                                                let expected_value =
                                                    Self::literal_to_value(&literal);
                                                if node.properties.get(&property.key)
                                                    != Some(&expected_value)
                                                {
                                                    return false;
                                                }
                                            }
                                        }
                                    }

                                    true // Don't check WHERE clause here, we'll check it later
                                })
                                .cloned()
                                .collect();

                            log::debug!(
                                "Found {} candidates for variable '{}'",
                                matching_nodes.len(),
                                identifier
                            );

                            if matching_nodes.is_empty() {
                                log::debug!("No match found for variable: {}", identifier);
                                return Ok((
                                    UndoOperation::InsertEdge {
                                        graph_path: graph_name,
                                        edge_id: "no_matches".to_string(),
                                    },
                                    0,
                                ));
                            }

                            variable_candidates.insert(identifier.clone(), matching_nodes);
                        } else {
                            log::debug!("Node pattern has no identifier, skipping");
                        }
                    } else {
                        log::debug!("Pattern element is not a node, skipping");
                    }
                }
            }
        }

        log::debug!("Variable candidates collected:");
        for (var_name, candidates) in &variable_candidates {
            log::debug!("  Variable '{}': {} candidates", var_name, candidates.len());
        }

        // Generate all combinations (Cartesian product) of variable bindings
        let variable_combinations = Self::generate_variable_combinations(&variable_candidates);
        log::debug!(
            "Generated {} variable combinations",
            variable_combinations.len()
        );

        if variable_combinations.is_empty() {
            log::debug!("No variable combinations found, no insertions performed");
            return Ok((
                UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_bindings".to_string(),
                },
                0,
            ));
        }

        // Filter combinations by WHERE clause if present
        let filtered_combinations = if let Some(ref where_clause) = self.statement.where_clause {
            log::debug!("Filtering combinations with WHERE clause");
            let original_count = variable_combinations.len();
            let filtered: Vec<_> = variable_combinations
                .into_iter()
                .filter(|combination| {
                    // Check WHERE clause against the combination
                    Self::evaluate_where_clause_on_combination(combination, where_clause)
                })
                .collect();
            log::debug!(
                "WHERE clause filtering: {} -> {} combinations",
                original_count,
                filtered.len()
            );
            filtered
        } else {
            variable_combinations
        };

        if filtered_combinations.is_empty() {
            log::debug!("No combinations passed WHERE clause filtering");
            return Ok((
                UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_where_matches".to_string(),
                },
                0,
            ));
        }

        // Step 1.5: Process WITH clause if present
        let processed_combinations = if let Some(ref with_clause) = self.statement.with_clause {
            // Convert filtered combinations to multi-binding format for WITH clause processor
            let mut multi_bindings = HashMap::new();

            // Collect all variables and their bindings across all filtered combinations
            for combination in &filtered_combinations {
                for (var_name, node) in combination {
                    multi_bindings
                        .entry(var_name.clone())
                        .or_insert_with(Vec::new)
                        .push(node.clone());
                }
            }

            log::debug!(
                "Processing WITH clause with {} variable bindings",
                multi_bindings.len()
            );
            for (var_name, nodes) in &multi_bindings {
                log::debug!("  Variable '{}' has {} bindings", var_name, nodes.len());
            }

            // Process WITH clause with all collected bindings
            let edges: Vec<Edge> = graph.get_all_edges().into_iter().cloned().collect();
            let temp_context = if context.function_registry.is_some() {
                context.clone()
            } else {
                context
                    .clone()
                    .with_function_registry(Arc::new(FunctionRegistry::new()))
            };
            match WithClauseProcessor::process_with_clause(
                with_clause,
                &multi_bindings,
                &edges,
                &temp_context,
            ) {
                Ok(with_result) => {
                    log::debug!(
                        "WITH clause processed successfully, has_aggregation: {}",
                        with_result.has_aggregation
                    );
                    log::debug!("Computed values: {:?}", with_result.computed_values);

                    if with_result.has_aggregation && !with_result.group_results.is_empty() {
                        // Handle GROUP BY aggregation - process each filtered group
                        let mut aggregated_bindings = Vec::new();

                        for group_result in &with_result.group_results {
                            // WITH clause has already filtered the groups, so all groups here should be processed
                            // Create binding combining group variables and computed values
                            let mut final_binding = HashMap::new();

                            // Add variables from the group bindings
                            for (var_name, nodes) in &group_result.variable_bindings {
                                if let Some(first_node) = nodes.first() {
                                    final_binding.insert(var_name.clone(), first_node.clone());
                                }
                            }

                            // Add computed values as virtual nodes (but don't overwrite original variables)
                            for (alias, value) in &group_result.computed_values {
                                // Check if this alias is already an original variable - if so, don't overwrite it
                                if !final_binding.contains_key(alias) {
                                    let virtual_node = Node {
                                        id: format!(
                                            "computed_{}_{}",
                                            alias,
                                            aggregated_bindings.len()
                                        ),
                                        labels: vec!["Computed".to_string()],
                                        properties: {
                                            let mut props = HashMap::new();
                                            props.insert("value".to_string(), value.clone());
                                            props
                                        },
                                    };
                                    final_binding.insert(alias.clone(), virtual_node);
                                }
                            }

                            aggregated_bindings.push(final_binding);
                        }

                        aggregated_bindings
                    } else if with_result.has_aggregation {
                        // Fallback for backward compatibility (single group)
                        let mut aggregated_bindings = Vec::new();

                        // Create single binding combining non-aggregate variables and computed values
                        let mut final_binding = HashMap::new();

                        // Add non-aggregate variables (those that appear in WITH clause)
                        for item in &with_clause.items {
                            if let Expression::Variable(var) = &item.expression {
                                if let Some(nodes) = multi_bindings.get(&var.name) {
                                    if let Some(first_node) = nodes.first() {
                                        let alias = if let Some(ref alias_name) = item.alias {
                                            alias_name.clone()
                                        } else {
                                            var.name.clone()
                                        };
                                        final_binding.insert(alias, first_node.clone());
                                    }
                                }
                            }
                        }

                        // Add computed values as virtual nodes
                        for (alias, value) in &with_result.computed_values {
                            let virtual_node = Node {
                                id: format!("computed_{}", alias),
                                labels: vec!["Computed".to_string()],
                                properties: {
                                    let mut props = HashMap::new();
                                    props.insert("value".to_string(), value.clone());
                                    props
                                },
                            };
                            final_binding.insert(alias.clone(), virtual_node);
                        }

                        // Note: WHERE clause filtering is now handled by WITH clause processor
                        aggregated_bindings.push(final_binding);
                        aggregated_bindings
                    } else {
                        // Non-aggregated WITH clause - process each filtered combination individually
                        let mut processed = Vec::new();

                        for combination in filtered_combinations {
                            let mut multi_binding = HashMap::new();
                            for (var_name, node) in &combination {
                                multi_binding.insert(var_name.clone(), vec![node.clone()]);
                            }

                            let temp_context = if context.function_registry.is_some() {
                                context.clone()
                            } else {
                                context
                                    .clone()
                                    .with_function_registry(Arc::new(FunctionRegistry::new()))
                            };
                            match WithClauseProcessor::process_with_clause(
                                with_clause,
                                &multi_binding,
                                &edges,
                                &temp_context,
                            ) {
                                Ok(individual_result) => {
                                    // Check WHERE clause
                                    if let Some(ref where_clause) = self.statement.where_clause {
                                        if !WithClauseProcessor::evaluate_where_with_computed_values(
                                            where_clause,
                                            &individual_result.computed_values,
                                        ) {
                                            continue;
                                        }
                                    }

                                    let mut final_bindings = combination;

                                    // Add computed values (but don't overwrite original variables)
                                    for (alias, value) in &individual_result.computed_values {
                                        // Check if this alias is already an original variable - if so, don't overwrite it
                                        if !final_bindings.contains_key(alias) {
                                            let virtual_node = Node {
                                                id: format!("computed_{}", alias),
                                                labels: vec!["Computed".to_string()],
                                                properties: {
                                                    let mut props = HashMap::new();
                                                    props
                                                        .insert("value".to_string(), value.clone());
                                                    props
                                                },
                                            };
                                            final_bindings.insert(alias.clone(), virtual_node);
                                        }
                                    }

                                    processed.push(final_bindings);
                                }
                                Err(e) => {
                                    log::error!(
                                        "Failed to process WITH clause for individual binding: {}",
                                        e
                                    );
                                }
                            }
                        }

                        processed
                    }
                }
                Err(e) => {
                    log::error!("Failed to process WITH clause: {}", e);
                    return Err(e);
                }
            }
        } else {
            // No WITH clause, use filtered combinations directly
            filtered_combinations
        };

        if processed_combinations.is_empty() {
            log::debug!("No combinations passed WITH/WHERE filtering");
            return Ok((
                UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_matches_after_filtering".to_string(),
                },
                0,
            ));
        }

        // Process each combination
        for mut variable_bindings in processed_combinations {
            // Step 2: Execute INSERT patterns - only insert edges, use matched nodes
            for (_pattern_idx, pattern) in self.statement.insert_graph_patterns.iter().enumerate() {
                for (i, element) in pattern.elements.iter().enumerate() {
                    match element {
                        PatternElement::Node(node_pattern) => {
                            // Check if this node is a reference to a matched variable or a new node
                            let should_create_node =
                                if let Some(ref identifier) = node_pattern.identifier {
                                    // If identifier exists in variable bindings, it's a matched node reference
                                    // If not, it's a new node to create
                                    !variable_bindings.contains_key(identifier)
                                } else {
                                    // No identifier means it's a new anonymous node
                                    true
                                };

                            if should_create_node {
                                // This is a NEW node to be created (like regular INSERT)

                                // Generate node ID - always use UUID to avoid conflicts
                                // The identifier is for reference within the query, not the node ID
                                let node_id = format!("insert_node_{}", Uuid::new_v4().simple());

                                // Extract properties with variable substitution
                                let properties = if let Some(ref prop_map) = node_pattern.properties
                                {
                                    Self::extract_properties(prop_map, &variable_bindings, context)
                                } else {
                                    HashMap::new()
                                };

                                // Create the node
                                let node = Node {
                                    id: node_id.clone(),
                                    labels: node_pattern.labels.clone(),
                                    properties,
                                };

                                // Add to graph
                                graph.add_node(node).map_err(|e| {
                                    ExecutionError::RuntimeError(format!(
                                        "Failed to insert node in MATCH INSERT: {}",
                                        e
                                    ))
                                })?;

                                log::debug!("Successfully created node with ID: {}", node_id);
                                inserted_count += 1;

                                // Add undo operation
                                undo_operations.push(UndoOperation::InsertNode {
                                    graph_path: graph_name.clone(),
                                    node_id: node_id.clone(),
                                });

                                // Add the created node to variable bindings for subsequent references
                                if let Some(ref identifier) = node_pattern.identifier {
                                    // Get the created node back from the graph to add to bindings
                                    if let Some(created_node) = graph.get_node(&node_id) {
                                        variable_bindings
                                            .insert(identifier.clone(), created_node.clone());
                                    }
                                }
                            } else {
                                log::debug!("Skipping matched node: {:?}", node_pattern.identifier);
                            }
                        }
                        PatternElement::Edge(edge_pattern) => {
                            // Handle edge creation in MATCH INSERT
                            if i == 0 || i >= pattern.elements.len() - 1 {
                                return Err(ExecutionError::RuntimeError(
                                    "Edge patterns in MATCH INSERT must be between two nodes"
                                        .to_string(),
                                ));
                            }

                            // Get source and target node identifiers from the pattern
                            let source_node_id = match pattern.elements.get(i - 1) {
                        Some(PatternElement::Node(source_node)) => {
                            if let Some(ref identifier) = source_node.identifier {
                                // Look up the matched node ID from variable bindings
                                if let Some(bound_node) = variable_bindings.get(identifier) {
                                    bound_node.id.clone()
                                } else {
                                    return Err(ExecutionError::RuntimeError(
                                        format!("Source node variable '{}' not found in MATCH bindings", identifier)
                                    ));
                                }
                            } else {
                                return Err(ExecutionError::RuntimeError(
                                    "Source node in MATCH INSERT must have an identifier".to_string()
                                ));
                            }
                        },
                        _ => return Err(ExecutionError::RuntimeError(
                            "Edge pattern in MATCH INSERT must be preceded by a source node".to_string()
                        )),
                    };

                            let target_node_id = match pattern.elements.get(i + 1) {
                        Some(PatternElement::Node(target_node)) => {
                            if let Some(ref identifier) = target_node.identifier {
                                // Look up the matched node ID from variable bindings
                                if let Some(bound_node) = variable_bindings.get(identifier) {
                                    bound_node.id.clone()
                                } else {
                                    return Err(ExecutionError::RuntimeError(
                                        format!("Target node variable '{}' not found in MATCH bindings", identifier)
                                    ));
                                }
                            } else {
                                return Err(ExecutionError::RuntimeError(
                                    "Target node in MATCH INSERT must have an identifier".to_string()
                                ));
                            }
                        },
                        _ => return Err(ExecutionError::RuntimeError(
                            "Edge pattern in MATCH INSERT must be followed by a target node".to_string()
                        )),
                    };

                            let edge_id = if let Some(ref identifier) = edge_pattern.identifier {
                                identifier.clone()
                            } else {
                                format!("insert_edge_{}", Uuid::new_v4().simple())
                            };

                            // Extract edge properties with variable substitution
                            let edge_properties =
                                if let Some(ref prop_map) = edge_pattern.properties {
                                    Self::extract_properties(prop_map, &variable_bindings, context)
                                } else {
                                    HashMap::new()
                                };

                            // Create the edge
                            let edge_label = edge_pattern
                                .labels
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "CONNECTED".to_string());

                            let edge = Edge {
                                id: edge_id.clone(),
                                from_node: source_node_id.clone(),
                                to_node: target_node_id.clone(),
                                label: edge_label.clone(),
                                properties: edge_properties,
                            };

                            // Add to graph
                            match graph.add_edge(edge) {
                                Ok(_) => {}
                                Err(crate::storage::types::GraphError::EdgeAlreadyExists(_)) => {
                                    log::info!("Edge '{}' already exists in MATCH INSERT, skipping duplicate", edge_id);
                                    // Add warning about duplicate insertion
                                    let warning_msg = format!("Duplicate edge detected: Edge with identical properties already exists (edge_id: {})", edge_id);
                                    context.add_warning(warning_msg);
                                    continue; // Skip this edge and continue with the next one
                                }
                                Err(e) => {
                                    return Err(ExecutionError::RuntimeError(format!(
                                        "Failed to insert edge in MATCH INSERT: {}",
                                        e
                                    )));
                                }
                            }

                            log::debug!("Successfully inserted edge with ID: {}", edge_id);
                            inserted_count += 1;

                            // Add undo operation
                            undo_operations.push(UndoOperation::InsertEdge {
                                graph_path: graph_name.clone(),
                                edge_id,
                            });
                        }
                    }
                }
            } // End of for pattern in insert_graph_patterns
        } // End of for variable_bindings in variable_combinations

        // Return the first undo operation if any
        let undo_op =
            undo_operations
                .into_iter()
                .next()
                .unwrap_or_else(|| UndoOperation::InsertEdge {
                    graph_path: graph_name,
                    edge_id: "no_operations".to_string(),
                });

        Ok((undo_op, inserted_count))
    }
}
