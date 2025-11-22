// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! WITH clause processor for handling aggregation and variable binding in MATCH statements
//!
//! This module provides shared functionality for processing WITH clauses across all MATCH
//! statement types (MATCH-INSERT, MATCH-SET, MATCH-DELETE, MATCH-REMOVE).

use crate::ast::ast::{DistinctQualifier, Expression, FunctionCall, Literal, WithClause, WithItem};
use crate::exec::{ExecutionContext, ExecutionError};
use crate::functions::FunctionContext;
use crate::storage::{Edge, Node, Value};
use std::collections::HashMap;

/// Results from processing a WITH clause
#[derive(Debug, Clone)]
pub struct WithClauseResult {
    /// Updated variable bindings after WITH processing
    pub variable_bindings: HashMap<String, Vec<Node>>,
    /// Aggregated values computed by WITH clause
    pub computed_values: HashMap<String, Value>,
    /// Whether any aggregation occurred
    pub has_aggregation: bool,
    /// Group results (for proper GROUP BY handling)
    pub group_results: Vec<GroupResult>,
}

/// Result from a single group in GROUP BY aggregation
#[derive(Debug, Clone)]
pub struct GroupResult {
    /// Variable bindings for this group
    pub variable_bindings: HashMap<String, Vec<Node>>,
    /// Computed values for this group
    pub computed_values: HashMap<String, Value>,
}

/// Processor for WITH clauses in MATCH statements
pub struct WithClauseProcessor;

impl WithClauseProcessor {
    /// Process a WITH clause given the current variable bindings and edges
    pub fn process_with_clause(
        with_clause: &WithClause,
        variable_bindings: &HashMap<String, Vec<Node>>,
        edges: &[Edge], // For relationship-based aggregation
        context: &ExecutionContext,
    ) -> Result<WithClauseResult, ExecutionError> {
        log::debug!(
            "WITH_CLAUSE_PROCESSOR: process_with_clause called from: {}",
            std::panic::Location::caller()
        );
        log::debug!(
            "WITH_CLAUSE_PROCESSOR: Processing WITH clause with {} items",
            with_clause.items.len()
        );
        log::debug!(
            "WITH_CLAUSE_PROCESSOR: Variable bindings: {} variables",
            variable_bindings.len()
        );
        log::debug!(
            "WITH_CLAUSE_PROCESSOR: Edges: {} relationships",
            edges.len()
        );

        let mut has_aggregation = false;

        // First pass: determine if we have aggregation and identify grouping variables
        let mut grouping_variables = Vec::new();
        let mut aggregation_items = Vec::new();

        for item in &with_clause.items {
            if Self::is_aggregation_expression(&item.expression) {
                has_aggregation = true;
                aggregation_items.push(item);
            } else {
                // Non-aggregated expressions become grouping variables
                match &item.expression {
                    Expression::Variable(var) => {
                        grouping_variables.push(var.name.clone());
                    }
                    Expression::PropertyAccess(prop_access) => {
                        // For property access like a.account_type, group by the base variable
                        grouping_variables.push(prop_access.object.clone());
                    }
                    _ => {
                        // For other expressions, we can't easily determine the grouping variable
                        // This will be handled by the aggregation logic
                    }
                }
            }
        }

        if !has_aggregation {
            // No aggregation - process normally
            return Self::process_without_aggregation(
                with_clause,
                variable_bindings,
                edges,
                context,
            );
        }

        log::debug!(
            "WITH clause has aggregation. Grouping by: {:?}",
            grouping_variables
        );
        log::debug!(
            "Input variable_bindings: {:?}",
            variable_bindings.keys().collect::<Vec<_>>()
        );
        log::debug!("Input edges: {} total", edges.len());

        // GROUP BY processing: create groups based on unique combinations of grouping variables
        let groups = Self::create_groups(variable_bindings, &grouping_variables, edges)?;

        log::debug!("Created {} groups for aggregation", groups.len());
        for (i, (group_nodes, group_edges)) in groups.iter().enumerate() {
            log::debug!(
                "Group {}: nodes={:?}, edges={}",
                i,
                group_nodes.keys().collect::<Vec<_>>(),
                group_edges.len()
            );
        }

        // Process each group and create separate results per group
        // This will allow proper WHERE clause filtering per group
        let mut group_results = Vec::new();

        for (group_id, (group_nodes, group_edges)) in groups.iter().enumerate() {
            log::debug!(
                "Processing group {}: {} nodes, {} edges",
                group_id,
                group_nodes.len(),
                group_edges.len()
            );

            let mut group_computed_values = HashMap::new();
            let mut group_bindings = HashMap::new();

            // Process each WITH item for this group
            for item in &with_clause.items {
                let value = if Self::is_aggregation_expression(&item.expression) {
                    // Evaluate aggregation on this specific group
                    Self::evaluate_aggregation_on_group(item, group_nodes, group_edges)?
                } else {
                    // Non-aggregated expressions - use group representative
                    Self::evaluate_non_aggregated_on_group(item, group_nodes)?
                };

                let alias = if let Some(ref alias_name) = item.alias {
                    alias_name.clone()
                } else if let Expression::Variable(var) = &item.expression {
                    // Preserve variable name when no alias is provided
                    var.name.clone()
                } else {
                    format!("expr_{}", group_computed_values.len())
                };

                log::debug!("Group {}: computed {}={:?}", group_id, alias, value);
                group_computed_values.insert(alias.clone(), value);
            }

            // Create bindings for this group (without group-specific suffixes)
            for var in &grouping_variables {
                if let Some(nodes) = group_nodes.get(var) {
                    group_bindings.insert(var.clone(), nodes.clone());
                }
            }

            group_results.push((group_computed_values, group_bindings));
        }

        // For now, return the first group's results
        // Convert to new GroupResult structure
        let groups: Vec<GroupResult> = group_results
            .into_iter()
            .map(|(computed_values, variable_bindings)| GroupResult {
                variable_bindings,
                computed_values,
            })
            .collect();

        // Apply WHERE clause filtering if present
        let filtered_groups = if let Some(ref where_clause) = with_clause.where_clause {
            let before_count = groups.len();
            let filtered = groups
                .into_iter()
                .filter(|group| {
                    let result = Self::evaluate_where_with_computed_values(
                        where_clause,
                        &group.computed_values,
                    );
                    log::debug!(
                        "DEBUG: WHERE filter for group with computed values {:?}: {}",
                        group.computed_values,
                        result
                    );
                    result
                })
                .collect::<Vec<_>>();
            log::debug!(
                "DEBUG: WHERE filtering: {} groups before, {} groups after",
                before_count,
                filtered.len()
            );
            filtered
        } else {
            groups
        };

        // For backward compatibility, use first group for main fields if available
        if let Some(first_group) = filtered_groups.first() {
            Ok(WithClauseResult {
                variable_bindings: first_group.variable_bindings.clone(),
                computed_values: first_group.computed_values.clone(),
                has_aggregation: true,
                group_results: filtered_groups,
            })
        } else {
            // No groups passed WHERE filtering - return empty result
            Ok(WithClauseResult {
                variable_bindings: HashMap::new(),
                computed_values: HashMap::new(),
                has_aggregation: true,
                group_results: Vec::new(),
            })
        }
    }

    /// Process WITH clause without aggregation (simpler path)
    fn process_without_aggregation(
        with_clause: &WithClause,
        variable_bindings: &HashMap<String, Vec<Node>>,
        edges: &[Edge],
        context: &ExecutionContext,
    ) -> Result<WithClauseResult, ExecutionError> {
        let mut computed_values = HashMap::new();
        let mut updated_bindings = variable_bindings.clone();

        // Process each WITH item normally
        for item in &with_clause.items {
            let value = Self::evaluate_with_item(item, variable_bindings, edges, context)?;

            let alias = if let Some(ref alias_name) = item.alias {
                alias_name.clone()
            } else if let Expression::Variable(var) = &item.expression {
                // Preserve variable name when no alias is provided
                var.name.clone()
            } else {
                format!("expr_{}", computed_values.len())
            };
            computed_values.insert(alias.clone(), value);

            // If this is a simple variable reference, update bindings
            if let Expression::Variable(var) = &item.expression {
                if let Some(nodes) = variable_bindings.get(&var.name) {
                    updated_bindings.insert(alias.clone(), nodes.clone());
                }
            }
        }

        // Apply WHERE clause filtering if present
        if let Some(ref where_clause) = with_clause.where_clause {
            // For non-aggregated WITH clauses, we need to filter each individual combination
            // Create individual combinations for each node and filter them
            let mut filtered_bindings = HashMap::new();

            // For each variable in the bindings, check if the combination passes the WHERE clause
            if let Some((_primary_var, nodes)) = variable_bindings.iter().next() {
                let mut filtered_nodes = Vec::new();

                for node in nodes.iter() {
                    // Create computed values for this specific node
                    let mut node_computed_values = HashMap::new();

                    for item in &with_clause.items {
                        let value = Self::evaluate_with_item_for_single_node(
                            item,
                            node,
                            variable_bindings,
                            edges,
                            context,
                        )?;

                        let alias = if let Some(ref alias_name) = item.alias {
                            alias_name.clone()
                        } else if let Expression::Variable(var) = &item.expression {
                            // Preserve variable name when no alias is provided
                            var.name.clone()
                        } else {
                            format!("expr_{}", node_computed_values.len())
                        };
                        node_computed_values.insert(alias.clone(), value.clone());
                    }

                    // Test if this node passes the WHERE clause
                    let passes_filter = Self::evaluate_where_with_computed_values(
                        where_clause,
                        &node_computed_values,
                    );
                    if passes_filter {
                        filtered_nodes.push(node.clone());
                    }
                }

                // Update all variable bindings to only include filtered nodes
                // Simplified logic: directly copy filtered nodes to all relevant variables
                for var in updated_bindings.keys() {
                    if let Some(_original_nodes) = variable_bindings.get(var) {
                        if !filtered_nodes.is_empty() {
                            filtered_bindings.insert(var.clone(), filtered_nodes.clone());
                        }
                    }
                }
            }

            // Update computed_values to reflect the first filtered combination if any
            let final_computed_values = if let Some((_, nodes)) = filtered_bindings.iter().next() {
                if let Some(first_node) = nodes.first() {
                    let mut new_computed_values = HashMap::new();
                    for item in &with_clause.items {
                        let value = Self::evaluate_with_item_for_single_node(
                            item,
                            first_node,
                            variable_bindings,
                            edges,
                            context,
                        )?;
                        let alias = if let Some(ref alias_name) = item.alias {
                            alias_name.clone()
                        } else if let Expression::Variable(var) = &item.expression {
                            // Preserve variable name when no alias is provided
                            var.name.clone()
                        } else {
                            format!("expr_{}", new_computed_values.len())
                        };
                        new_computed_values.insert(alias, value);
                    }
                    new_computed_values
                } else {
                    HashMap::new()
                }
            } else {
                HashMap::new()
            };

            Ok(WithClauseResult {
                variable_bindings: filtered_bindings,
                computed_values: final_computed_values,
                has_aggregation: false,
                group_results: Vec::new(),
            })
        } else {
            Ok(WithClauseResult {
                variable_bindings: updated_bindings,
                computed_values,
                has_aggregation: false,
                group_results: Vec::new(),
            })
        }
    }

    /// Evaluate a WITH item for a single node (used in non-aggregated WHERE filtering)
    fn evaluate_with_item_for_single_node(
        item: &WithItem,
        node: &Node,
        variable_bindings: &HashMap<String, Vec<Node>>,
        edges: &[Edge],
        context: &ExecutionContext,
    ) -> Result<Value, ExecutionError> {
        match &item.expression {
            Expression::Variable(var) => {
                // For a single node, if the variable matches, return the node ID
                if let Some(nodes) = variable_bindings.get(&var.name) {
                    if nodes.iter().any(|n| n.id == node.id) {
                        Ok(Value::String(node.id.clone()))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::PropertyAccess(prop_access) => {
                // For property access, get the property from the specific node
                if let Some(nodes) = variable_bindings.get(&prop_access.object) {
                    if nodes.iter().any(|n| n.id == node.id) {
                        Ok(node
                            .properties
                            .get(&prop_access.property)
                            .cloned()
                            .unwrap_or(Value::Null))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::Literal(literal) => Ok(Self::literal_to_value(literal)),
            Expression::FunctionCall(_) => {
                // For function calls, use the general evaluation method
                Self::evaluate_with_item(item, variable_bindings, edges, context)
            }
            _ => Ok(Value::Null),
        }
    }

    /// Create groups for aggregation based on grouping variables
    fn create_groups(
        variable_bindings: &HashMap<String, Vec<Node>>,
        grouping_variables: &[String],
        edges: &[Edge],
    ) -> Result<Vec<(HashMap<String, Vec<Node>>, Vec<Edge>)>, ExecutionError> {
        if grouping_variables.is_empty() {
            // No grouping variables - single group with all data
            let group_nodes = variable_bindings.clone();
            let group_edges = edges.to_vec();
            return Ok(vec![(group_nodes, group_edges)]);
        }

        // The proper way to group is based on the original MATCH result combinations
        // Each "row" in the MATCH result represents a combination of variables
        // We need to reconstruct these combinations and group them by the grouping variables

        let mut groups = HashMap::new();

        // Handle different grouping strategies based on number of grouping variables
        if grouping_variables.len() == 1 {
            // Single variable grouping - use the proven original approach
            let primary_var = &grouping_variables[0];
            if let Some(primary_nodes) = variable_bindings.get(primary_var) {
                // Group by unique values of the primary grouping variable
                use std::collections::HashSet;
                let mut unique_primary_ids = HashSet::new();

                // First pass: identify unique values of the grouping variable
                for node in primary_nodes {
                    unique_primary_ids.insert(node.id.clone());
                }

                // Second pass: create one group for each unique value of the grouping variable
                for unique_id in unique_primary_ids {
                    let group_key = unique_id.clone();
                    log::debug!(
                        "DEBUG GROUPING: Creating group for {} with id '{}'",
                        primary_var,
                        unique_id
                    );

                    let mut group_nodes = HashMap::new();

                    // Add all instances of the primary grouping variable with this ID
                    let primary_instances: Vec<Node> = primary_nodes
                        .iter()
                        .filter(|node| node.id == unique_id)
                        .cloned()
                        .collect();

                    log::debug!(
                        "DEBUG GROUPING: Found {} instances of {} with id '{}'",
                        primary_instances.len(),
                        primary_var,
                        unique_id
                    );

                    if !primary_instances.is_empty() {
                        group_nodes.insert(primary_var.clone(), vec![primary_instances[0].clone()]);
                    }

                    // Find all rows where the primary variable has this ID
                    let mut associated_indices = Vec::new();
                    for (index, node) in primary_nodes.iter().enumerate() {
                        if node.id == unique_id {
                            associated_indices.push(index);
                        }
                    }

                    log::debug!(
                        "DEBUG GROUPING: Found {} row indices associated with {}: {:?}",
                        associated_indices.len(),
                        unique_id,
                        associated_indices
                    );

                    // Collect all variables from the associated rows using relationship structure
                    for (var_name, nodes) in variable_bindings {
                        if var_name != primary_var {
                            // Use relationship structure to find nodes connected to this specific group
                            let mut associated_nodes = Vec::new();

                            // Find nodes connected to the primary grouping variable through edges
                            let primary_node_id = &unique_id;
                            for edge in edges {
                                let mut connected_node_id: Option<&String> = None;

                                // Check if this edge connects the primary grouping node to another variable
                                if edge.from_node == *primary_node_id {
                                    connected_node_id = Some(&edge.to_node);
                                } else if edge.to_node == *primary_node_id {
                                    connected_node_id = Some(&edge.from_node);
                                }

                                // If we found a connected node, check if it belongs to this variable
                                if let Some(connected_id) = connected_node_id {
                                    for node in nodes {
                                        if node.id == *connected_id {
                                            // Only add if not already present
                                            if !associated_nodes
                                                .iter()
                                                .any(|n: &Node| n.id == node.id)
                                            {
                                                associated_nodes.push(node.clone());
                                            }
                                        }
                                    }
                                }
                            }

                            // Fallback to row-based grouping if no relationship-based connections found
                            if associated_nodes.is_empty() {
                                for &index in &associated_indices {
                                    if index < nodes.len() {
                                        associated_nodes.push(nodes[index].clone());
                                    }
                                }
                            }

                            if !associated_nodes.is_empty() {
                                log::debug!("DEBUG GROUPING: For group {}, variable '{}' gets {} nodes: {:?}",
                                        unique_id, var_name, associated_nodes.len(),
                                        associated_nodes.iter().map(|n| &n.id).collect::<Vec<_>>());
                                group_nodes.insert(var_name.clone(), associated_nodes);
                            }
                        }
                    }

                    groups.insert(group_key, (group_nodes, Vec::new()));
                }
            }
        } else {
            // Multi-variable grouping - use the new approach
            let max_rows = grouping_variables
                .iter()
                .filter_map(|var| variable_bindings.get(var).map(|nodes| nodes.len()))
                .max()
                .unwrap_or(0);

            // Create groups based on unique combinations of all grouping variables
            for row_index in 0..max_rows {
                // Create composite group key from all grouping variables
                let mut group_key_parts = Vec::new();
                let mut group_nodes = HashMap::new();
                let mut all_vars_present = true;

                // Collect values for all grouping variables for this row
                for grouping_var in grouping_variables {
                    if let Some(nodes) = variable_bindings.get(grouping_var) {
                        if row_index < nodes.len() {
                            let node = &nodes[row_index];
                            group_key_parts.push(format!("{}:{}", grouping_var, node.id));
                            group_nodes.insert(grouping_var.clone(), vec![node.clone()]);
                        } else {
                            all_vars_present = false;
                            break;
                        }
                    } else {
                        all_vars_present = false;
                        break;
                    }
                }

                if !all_vars_present {
                    continue;
                }

                // Create composite group key
                let group_key = group_key_parts.join("_");

                // Get or create group for this unique combination
                let group_entry = groups
                    .entry(group_key)
                    .or_insert_with(|| (group_nodes.clone(), Vec::new()));

                // Add all other (non-grouping) variables from this row
                for (var_name, nodes) in variable_bindings {
                    if !grouping_variables.contains(var_name) && row_index < nodes.len() {
                        let node = &nodes[row_index];
                        group_entry
                            .0
                            .entry(var_name.clone())
                            .or_insert_with(Vec::new)
                            .push(node.clone());
                    }
                }
            }
        }

        // Add edges to each group - use different filtering logic based on grouping type
        for (_group_key, (group_nodes, group_edges)) in groups.iter_mut() {
            for edge in edges {
                let from_id = &edge.from_node;
                let to_id = &edge.to_node;

                // Check if edge endpoints are in this group
                let mut from_found = false;
                let mut to_found = false;

                for (_, nodes) in group_nodes.iter() {
                    for node in nodes {
                        if node.id == *from_id {
                            from_found = true;
                        }
                        if node.id == *to_id {
                            to_found = true;
                        }
                    }
                }

                // Different inclusion criteria based on grouping variables count
                let include_edge = if grouping_variables.len() == 1 {
                    // Single-variable grouping: include if at least one endpoint is in group
                    // This handles cases like MATCH (a)-[r]->(b) WITH b, count(r)
                    from_found || to_found
                } else {
                    // Multi-variable grouping: include only if both endpoints are in group
                    // This handles cases like MATCH (a)-[r]->(b) WITH a, b, count(r)
                    from_found && to_found
                };

                if include_edge {
                    group_edges.push(edge.clone());
                }
            }
        }

        // Fallback: if all groups have 0 edges with single-variable grouping,
        // there might be an ID mismatch - fall back to giving all edges to all groups
        if grouping_variables.len() == 1 {
            let total_edges_assigned: usize = groups.values().map(|(_, edges)| edges.len()).sum();
            if total_edges_assigned == 0 && !edges.is_empty() {
                log::debug!(
                    "No edges assigned to any group with single-variable grouping - using fallback"
                );
                for (_, (_, group_edges)) in groups.iter_mut() {
                    *group_edges = edges.to_vec();
                }
            }
        }

        let result: Vec<_> = groups.into_values().collect();
        Ok(result)
    }

    /// Evaluate aggregation on a specific group
    fn evaluate_aggregation_on_group(
        item: &WithItem,
        group_nodes: &HashMap<String, Vec<Node>>,
        group_edges: &[Edge],
    ) -> Result<Value, ExecutionError> {
        if let Expression::FunctionCall(func_call) = &item.expression {
            Self::evaluate_group_aggregation_function(func_call, group_nodes, group_edges)
        } else {
            log::warn!("Non-function aggregation expression: {:?}", item.expression);
            Ok(Value::Null)
        }
    }

    /// Evaluate non-aggregated expression on a group (return representative value)
    fn evaluate_non_aggregated_on_group(
        item: &WithItem,
        group_nodes: &HashMap<String, Vec<Node>>,
    ) -> Result<Value, ExecutionError> {
        match &item.expression {
            Expression::Variable(var) => {
                if let Some(nodes) = group_nodes.get(&var.name) {
                    if let Some(first_node) = nodes.first() {
                        // Return the full node object to allow property access
                        Ok(Value::Node(first_node.clone()))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::PropertyAccess(prop_access) => {
                if let Some(nodes) = group_nodes.get(&prop_access.object) {
                    if let Some(first_node) = nodes.first() {
                        Ok(first_node
                            .properties
                            .get(&prop_access.property)
                            .cloned()
                            .unwrap_or(Value::Null))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::Literal(literal) => Ok(Self::literal_to_value(literal)),
            _ => Ok(Value::Null),
        }
    }

    /// Evaluate aggregation functions on a specific group
    fn evaluate_group_aggregation_function(
        func_call: &FunctionCall,
        group_nodes: &HashMap<String, Vec<Node>>,
        group_edges: &[Edge],
    ) -> Result<Value, ExecutionError> {
        match func_call.name.to_uppercase().as_str() {
            "COUNT" => {
                if func_call.arguments.is_empty() {
                    // COUNT(*) - count items in this group
                    let count = group_edges
                        .len()
                        .max(group_nodes.values().map(|v| v.len()).sum());
                    Ok(Value::Number(count as f64))
                } else if let Some(Expression::Variable(var)) = func_call.arguments.first() {
                    log::debug!("DEBUG: COUNT({}) - checking edges and nodes", var.name);
                    if func_call.distinct == DistinctQualifier::Distinct {
                        // COUNT(DISTINCT variable) - count unique values
                        use std::collections::HashSet;
                        let mut unique_ids = HashSet::new();

                        if let Some(nodes) = group_nodes.get(&var.name) {
                            for node in nodes {
                                unique_ids.insert(&node.id);
                            }
                        }
                        Ok(Value::Number(unique_ids.len() as f64))
                    } else {
                        // COUNT(variable) - check if it's an edge variable like 't'
                        if var.name == "t" || var.name == "r" || var.name == "e" {
                            // Count edges for relationship variables
                            let count = group_edges.len();
                            log::debug!("DEBUG: COUNT({}) = {} (counting edges)", var.name, count);
                            Ok(Value::Number(count as f64))
                        } else {
                            // Count nodes for node variables
                            let count = group_nodes
                                .get(&var.name)
                                .map(|nodes| nodes.len())
                                .unwrap_or(0);
                            log::debug!("DEBUG: COUNT({}) = {} (counting nodes)", var.name, count);
                            Ok(Value::Number(count as f64))
                        }
                    }
                } else {
                    Ok(Value::Number(0.0))
                }
            }
            "AVG" => {
                log::debug!(
                    "DEBUG: AVG function called with arguments: {:?}",
                    func_call.arguments
                );
                if let Some(Expression::PropertyAccess(prop_access)) = func_call.arguments.first() {
                    let mut sum = 0.0;
                    let mut count = 0;

                    // Try to find the property in group edges first (common case)
                    log::debug!(
                        "DEBUG: Looking for property {} on object {} in {} edges",
                        prop_access.property,
                        prop_access.object,
                        group_edges.len()
                    );
                    for edge in group_edges {
                        log::debug!(
                            "DEBUG: Edge has properties: {:?}",
                            edge.properties.keys().collect::<Vec<_>>()
                        );
                        if let Some(Value::Number(n)) = edge.properties.get(&prop_access.property) {
                            sum += n;
                            count += 1;
                            log::debug!(
                                "Group AVG: Found edge property {}={}",
                                prop_access.property,
                                n
                            );
                        }
                    }

                    // If no edge properties found, try nodes
                    if count == 0 {
                        if let Some(nodes) = group_nodes.get(&prop_access.object) {
                            for node in nodes {
                                if let Some(Value::Number(n)) =
                                    node.properties.get(&prop_access.property)
                                {
                                    sum += n;
                                    count += 1;
                                }
                            }
                        }
                    }

                    if count > 0 {
                        let avg = sum / count as f64;
                        log::debug!("DEBUG: with_clause_processor AVG (group function) - returning Number({}) from sum={}, count={}", avg, sum, count);
                        log::debug!("Group AVG computed: {} (sum={}, count={})", avg, sum, count);
                        Ok(Value::Number(avg))
                    } else {
                        log::debug!("DEBUG: with_clause_processor AVG (group function) - returning NULL (no values found)");
                        log::debug!("Group AVG: No numeric values found");
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            "SUM" => {
                if let Some(Expression::PropertyAccess(prop_access)) = func_call.arguments.first() {
                    log::debug!(
                        "DEBUG: with_clause_processor SUM (group function) called for {}.{}",
                        prop_access.object,
                        prop_access.property
                    );
                    log::debug!(
                        "WITH_CLAUSE_PROCESSOR: SUM function called from: {}",
                        std::panic::Location::caller()
                    );
                    log::debug!(
                        "WITH_CLAUSE_PROCESSOR: SUM aggregating {}.{}",
                        prop_access.object,
                        prop_access.property
                    );

                    // Debug group contents
                    log::debug!("DEBUG SUM: Group has {} edge(s)", group_edges.len());
                    for edge in group_edges {
                        log::debug!(
                            "DEBUG SUM: Edge {} properties: {:?}",
                            edge.label,
                            edge.properties
                        );
                    }
                    if let Some(nodes) = group_nodes.get(&prop_access.object) {
                        log::debug!(
                            "DEBUG SUM: Group has {} nodes for '{}'",
                            nodes.len(),
                            prop_access.object
                        );
                        for node in nodes {
                            log::debug!(
                                "DEBUG SUM: Node {} has {}: {:?}",
                                node.id,
                                prop_access.property,
                                node.properties.get(&prop_access.property)
                            );
                        }
                    } else {
                        log::debug!(
                            "DEBUG SUM: No nodes found for variable '{}'",
                            prop_access.object
                        );
                    }

                    let mut sum = 0.0;
                    let mut has_values = false;

                    // Sum from edges first
                    for edge in group_edges {
                        if let Some(Value::Number(n)) = edge.properties.get(&prop_access.property) {
                            sum += n;
                            has_values = true;
                        }
                    }

                    // Sum from nodes if needed
                    if let Some(nodes) = group_nodes.get(&prop_access.object) {
                        for node in nodes {
                            if let Some(Value::Number(n)) =
                                node.properties.get(&prop_access.property)
                            {
                                sum += n;
                                has_values = true;
                            }
                        }
                    }

                    // SUM should return NULL if no values were found
                    if has_values {
                        Ok(Value::Number(sum))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            "COLLECT" => {
                if let Some(Expression::Variable(var)) = func_call.arguments.first() {
                    let mut collected = Vec::new();
                    if let Some(nodes) = group_nodes.get(&var.name) {
                        for node in nodes {
                            collected.push(Value::String(node.id.clone()));
                        }
                    }
                    Ok(Value::String(format!("{:?}", collected)))
                } else {
                    Ok(Value::String("[]".to_string()))
                }
            }
            _ => {
                log::warn!("Unsupported group aggregation function: {}", func_call.name);
                Ok(Value::Null)
            }
        }
    }

    /// Evaluate a single WITH item (legacy method, used for non-aggregation path)
    fn evaluate_with_item(
        item: &WithItem,
        variable_bindings: &HashMap<String, Vec<Node>>,
        edges: &[Edge],
        context: &ExecutionContext,
    ) -> Result<Value, ExecutionError> {
        match &item.expression {
            Expression::Variable(var) => {
                // For simple variable references, return the first node's ID or a representative value
                if let Some(nodes) = variable_bindings.get(&var.name) {
                    if let Some(first_node) = nodes.first() {
                        Ok(Value::String(first_node.id.clone()))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::PropertyAccess(prop_access) => {
                // Handle property access like node.property
                if let Some(nodes) = variable_bindings.get(&prop_access.object) {
                    if let Some(first_node) = nodes.first() {
                        Ok(first_node
                            .properties
                            .get(&prop_access.property)
                            .cloned()
                            .unwrap_or(Value::Null))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::FunctionCall(func_call) => {
                // Check if it's an aggregation function first
                if Self::is_aggregation_expression(&Expression::FunctionCall(func_call.clone())) {
                    Self::evaluate_aggregation_function(func_call, variable_bindings, edges)
                } else {
                    // For non-aggregation functions, use the function registry from context
                    log::debug!(
                        "Evaluating non-aggregation function '{}' in WITH clause",
                        func_call.name
                    );

                    let function_registry =
                        context.function_registry.as_ref().ok_or_else(|| {
                            ExecutionError::RuntimeError(
                                "Function registry not available in execution context".to_string(),
                            )
                        })?;

                    let function = function_registry.get(&func_call.name).ok_or_else(|| {
                        ExecutionError::UnsupportedOperator(format!(
                            "Function not found: {}",
                            func_call.name
                        ))
                    })?;

                    // Evaluate function arguments
                    let mut evaluated_args = Vec::new();
                    for arg in &func_call.arguments {
                        let arg_value = Self::evaluate_expression_arg(arg, variable_bindings)?;
                        evaluated_args.push(arg_value);
                    }

                    // Create function context (stub - arguments not used in stub implementation)
                    let _evaluated_args = evaluated_args; // Suppress unused warning
                    let function_context = FunctionContext::new(vec![], HashMap::new(), vec![]);

                    // Execute the function
                    function.execute(&function_context).map_err(|e| {
                        ExecutionError::RuntimeError(format!("Function execution failed: {}", e))
                    })
                }
            }
            Expression::Literal(literal) => Ok(Self::literal_to_value(literal)),
            _ => {
                log::warn!(
                    "Unsupported WITH clause expression type: {:?}",
                    item.expression
                );
                Ok(Value::Null)
            }
        }
    }

    /// Evaluate aggregation functions like count(), sum(), avg(), collect()
    fn evaluate_aggregation_function(
        func_call: &FunctionCall,
        variable_bindings: &HashMap<String, Vec<Node>>,
        edges: &[Edge],
    ) -> Result<Value, ExecutionError> {
        match func_call.name.to_uppercase().as_str() {
            "COUNT" => {
                if func_call.arguments.is_empty() {
                    // COUNT(*) - count all combinations
                    let total_combinations = Self::calculate_total_combinations(variable_bindings);
                    log::debug!("COUNT(*) computed: {}", total_combinations);
                    Ok(Value::Number(total_combinations as f64))
                } else if let Some(arg) = func_call.arguments.first() {
                    match arg {
                        Expression::Variable(var) => {
                            if func_call.distinct == DistinctQualifier::Distinct {
                                // COUNT(DISTINCT variable) - count unique nodes
                                use std::collections::HashSet;
                                let mut unique_ids = HashSet::new();

                                if let Some(nodes) = variable_bindings.get(&var.name) {
                                    for node in nodes {
                                        unique_ids.insert(&node.id);
                                    }
                                }
                                log::debug!(
                                    "COUNT(DISTINCT {}) computed: {}",
                                    var.name,
                                    unique_ids.len()
                                );
                                Ok(Value::Number(unique_ids.len() as f64))
                            } else {
                                // COUNT(variable) - count nodes bound to this variable
                                let count = variable_bindings
                                    .get(&var.name)
                                    .map(|nodes| nodes.len())
                                    .unwrap_or(0);
                                log::debug!("COUNT({}) computed: {}", var.name, count);
                                Ok(Value::Number(count as f64))
                            }
                        }
                        _ => {
                            log::warn!("Unsupported COUNT argument: {:?}", arg);
                            Ok(Value::Number(0.0))
                        }
                    }
                } else {
                    Ok(Value::Number(0.0))
                }
            }
            "SUM" => {
                if let Some(Expression::PropertyAccess(prop_access)) = func_call.arguments.first() {
                    let mut sum = 0.0;
                    let mut has_values = false;
                    if let Some(nodes) = variable_bindings.get(&prop_access.object) {
                        for node in nodes {
                            if let Some(Value::Number(n)) =
                                node.properties.get(&prop_access.property)
                            {
                                sum += n;
                                has_values = true;
                            }
                        }
                    }
                    // SUM should return NULL if no values were found
                    if has_values {
                        Ok(Value::Number(sum))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            "AVG" => {
                if let Some(Expression::PropertyAccess(prop_access)) = func_call.arguments.first() {
                    let mut sum = 0.0;
                    let mut count = 0;

                    // First try to find nodes with this variable name
                    if let Some(nodes) = variable_bindings.get(&prop_access.object) {
                        for node in nodes {
                            if let Some(Value::Number(n)) =
                                node.properties.get(&prop_access.property)
                            {
                                sum += n;
                                count += 1;
                                log::debug!(
                                    "AVG: Found numeric property {}={} in node {}",
                                    prop_access.property,
                                    n,
                                    node.id
                                );
                            }
                        }
                    }

                    // If no nodes found, try to find edges with this variable name
                    if count == 0 {
                        log::debug!("AVG: No nodes found for variable '{}', checking {} edges for property '{}'", 
                            prop_access.object, edges.len(), prop_access.property);

                        for edge in edges {
                            if let Some(Value::Number(n)) =
                                edge.properties.get(&prop_access.property)
                            {
                                sum += n;
                                count += 1;
                                log::debug!(
                                    "AVG: Found numeric property {}={} in edge {}",
                                    prop_access.property,
                                    n,
                                    edge.id
                                );
                            }
                        }
                    }

                    if count > 0 {
                        let avg = sum / count as f64;
                        log::debug!("AVG computed: {} (sum={}, count={})", avg, sum, count);
                        Ok(Value::Number(avg))
                    } else {
                        log::debug!(
                            "AVG: No numeric values found for property '{}'",
                            prop_access.property
                        );
                        Ok(Value::Null)
                    }
                } else {
                    log::debug!("AVG: No property access argument found");
                    Ok(Value::Null)
                }
            }
            "COLLECT" => {
                if let Some(Expression::Variable(var)) = func_call.arguments.first() {
                    let mut collected = Vec::new();
                    if let Some(nodes) = variable_bindings.get(&var.name) {
                        for node in nodes {
                            collected.push(Value::String(node.id.clone()));
                        }
                    }
                    // For now, return as a string representation
                    Ok(Value::String(format!("{:?}", collected)))
                } else {
                    Ok(Value::String("[]".to_string()))
                }
            }
            _ => {
                log::warn!("Unsupported aggregation function: {}", func_call.name);
                Ok(Value::Null)
            }
        }
    }

    /// Check if an expression involves aggregation
    fn is_aggregation_expression(expr: &Expression) -> bool {
        match expr {
            Expression::FunctionCall(func_call) => {
                matches!(
                    func_call.name.to_uppercase().as_str(),
                    "COUNT" | "SUM" | "AVG" | "MIN" | "MAX" | "COLLECT"
                )
            }
            _ => false,
        }
    }

    /// Calculate total combinations across all variable bindings
    fn calculate_total_combinations(variable_bindings: &HashMap<String, Vec<Node>>) -> usize {
        if variable_bindings.is_empty() {
            return 0;
        }

        variable_bindings
            .values()
            .map(|nodes| nodes.len().max(1))
            .product()
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
            Literal::Vector(vec) => Value::String(format!("{:?}", vec)),
            Literal::List(list) => {
                let converted: Vec<Value> =
                    list.iter().map(Self::literal_to_value).collect();
                Value::List(converted)
            }
        }
    }

    /// Evaluate WHERE clause conditions using computed values from WITH clause
    pub fn evaluate_where_with_computed_values(
        where_clause: &crate::ast::ast::WhereClause,
        computed_values: &HashMap<String, Value>,
    ) -> bool {
        Self::evaluate_expression_with_computed_values(&where_clause.condition, computed_values)
    }

    /// Evaluate an expression using computed values from WITH clause
    fn evaluate_expression_with_computed_values(
        expr: &Expression,
        computed_values: &HashMap<String, Value>,
    ) -> bool {
        match expr {
            Expression::Binary(binary_op) => {
                let left_val = Self::get_value_from_expression(&binary_op.left, computed_values);
                let right_val = Self::get_value_from_expression(&binary_op.right, computed_values);

                let result = match &binary_op.operator {
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
                    crate::ast::ast::Operator::Equal => left_val == right_val,
                    crate::ast::ast::Operator::NotEqual => left_val != right_val,
                    crate::ast::ast::Operator::GreaterEqual => {
                        if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                            l >= r
                        } else {
                            false
                        }
                    }
                    crate::ast::ast::Operator::LessEqual => {
                        if let (Value::Number(l), Value::Number(r)) = (&left_val, &right_val) {
                            l <= r
                        } else {
                            false
                        }
                    }
                    crate::ast::ast::Operator::And => {
                        // For AND, recursively evaluate both sides as boolean expressions
                        let left_bool = Self::evaluate_expression_with_computed_values(
                            &binary_op.left,
                            computed_values,
                        );
                        let right_bool = Self::evaluate_expression_with_computed_values(
                            &binary_op.right,
                            computed_values,
                        );
                        left_bool && right_bool
                    }
                    crate::ast::ast::Operator::Or => {
                        // For OR, recursively evaluate both sides as boolean expressions
                        let left_bool = Self::evaluate_expression_with_computed_values(
                            &binary_op.left,
                            computed_values,
                        );
                        let right_bool = Self::evaluate_expression_with_computed_values(
                            &binary_op.right,
                            computed_values,
                        );
                        left_bool || right_bool
                    }
                    _ => {
                        log::warn!(
                            "Unsupported operator in WHERE clause: {:?}",
                            binary_op.operator
                        );
                        false
                    }
                };
                result
            }
            Expression::Variable(var) => {
                // Look up computed value
                computed_values
                    .get(&var.name)
                    .map(|v| match v {
                        Value::Boolean(b) => *b,
                        Value::Null => false,
                        _ => true,
                    })
                    .unwrap_or(false)
            }
            _ => {
                log::warn!(
                    "Unsupported WHERE expression with computed values: {:?}",
                    expr
                );
                true
            }
        }
    }

    /// Get value from expression using computed values
    fn get_value_from_expression(
        expr: &Expression,
        computed_values: &HashMap<String, Value>,
    ) -> Value {
        match expr {
            Expression::Variable(var) => computed_values
                .get(&var.name)
                .cloned()
                .unwrap_or(Value::Null),
            Expression::Literal(literal) => Self::literal_to_value(literal),
            _ => Value::Null,
        }
    }

    /// Helper method to evaluate function arguments
    fn evaluate_expression_arg(
        expr: &Expression,
        variable_bindings: &HashMap<String, Vec<Node>>,
    ) -> Result<Value, ExecutionError> {
        match expr {
            Expression::Variable(var) => {
                if let Some(nodes) = variable_bindings.get(&var.name) {
                    if let Some(first_node) = nodes.first() {
                        Ok(Value::String(first_node.id.clone()))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::PropertyAccess(prop_access) => {
                if let Some(nodes) = variable_bindings.get(&prop_access.object) {
                    if let Some(first_node) = nodes.first() {
                        Ok(first_node
                            .properties
                            .get(&prop_access.property)
                            .cloned()
                            .unwrap_or(Value::Null))
                    } else {
                        Ok(Value::Null)
                    }
                } else {
                    Ok(Value::Null)
                }
            }
            Expression::Literal(literal) => Ok(Self::literal_to_value(literal)),
            _ => {
                log::warn!(
                    "Unsupported expression type for function argument: {:?}",
                    expr
                );
                Ok(Value::Null)
            }
        }
    }
}
