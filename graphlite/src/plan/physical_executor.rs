// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical plan execution engine
//!
//! This module implements the execution logic for physical query plans,
//! converting PhysicalNode operators into actual graph operations that
//! produce variable bindings.

use crate::ast::{EdgeDirection, Expression, Literal, Operator};
use crate::exec::ExecutionError;
use crate::plan::logical::AggregateFunction;
use crate::plan::physical::{AggregateItem, PhysicalNode, PhysicalPlan, ProjectionItem, SortItem};
use crate::storage::{Edge, GraphCache, Node, Value};
use std::collections::HashMap;

/// A row of variable bindings produced during execution
pub type BindingRow = HashMap<String, Value>;

/// Result of executing a physical plan - a collection of binding rows
pub type ExecutionResult = Vec<BindingRow>;

/// Physical plan executor
pub struct PhysicalExecutor<'a> {
    graph: &'a GraphCache,
}

impl<'a> PhysicalExecutor<'a> {
    /// Create a new physical executor
    pub fn new(graph: &'a GraphCache) -> Self {
        Self { graph }
    }

    /// Execute a physical plan and return variable bindings
    pub fn execute(&self, plan: &PhysicalPlan) -> Result<ExecutionResult, ExecutionError> {
        self.execute_node(&plan.root)
    }

    /// Execute a physical node and return bindings
    fn execute_node(&self, node: &PhysicalNode) -> Result<ExecutionResult, ExecutionError> {
        match node {
            PhysicalNode::NodeSeqScan {
                variable,
                labels,
                properties,
                ..
            } => self.execute_node_seq_scan(variable, labels, properties),

            PhysicalNode::NodeIndexScan {
                variable,
                labels,
                properties,
                ..
            } => self.execute_node_index_scan(variable, labels, properties),

            PhysicalNode::EdgeSeqScan {
                variable,
                labels,
                properties,
                ..
            } => self.execute_edge_seq_scan(variable, labels, properties),

            PhysicalNode::IndexedExpand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
                ..
            } => self.execute_indexed_expand(
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            ),

            PhysicalNode::HashExpand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
                ..
            } => {
                // Hash expand uses same logic as indexed expand for now
                self.execute_indexed_expand(
                    from_variable,
                    edge_variable,
                    to_variable,
                    edge_labels,
                    direction,
                    properties,
                    input,
                )
            }

            PhysicalNode::Filter {
                condition,
                input,
                ..
            } => self.execute_filter(condition, input),

            PhysicalNode::Project {
                expressions,
                input,
                ..
            } => self.execute_project(expressions, input),

            PhysicalNode::HashAggregate {
                group_by,
                aggregates,
                input,
                ..
            } => self.execute_aggregate(group_by, aggregates, input),

            PhysicalNode::SortAggregate {
                group_by,
                aggregates,
                input,
                ..
            } => {
                // Use same logic as hash aggregate for now
                self.execute_aggregate(group_by, aggregates, input)
            }

            PhysicalNode::InMemorySort {
                expressions,
                input,
                ..
            } => self.execute_sort(expressions, input),

            PhysicalNode::ExternalSort {
                expressions,
                input,
                ..
            } => {
                // Use same logic as in-memory sort for now
                self.execute_sort(expressions, input)
            }

            PhysicalNode::Limit {
                count,
                offset,
                input,
                ..
            } => self.execute_limit(*count, *offset, input),

            PhysicalNode::Distinct { input, .. } => self.execute_distinct(input),

            PhysicalNode::NestedLoopJoin {
                join_type,
                condition,
                left,
                right,
                ..
            } => self.execute_nested_loop_join(join_type, condition, left, right),

            PhysicalNode::SingleRow { .. } => {
                // Return single empty row
                Ok(vec![HashMap::new()])
            }

            _ => Err(ExecutionError::UnsupportedOperator(format!(
                "Physical operator not yet implemented: {:?}",
                node
            ))),
        }
    }

    /// Execute node sequential scan
    fn execute_node_seq_scan(
        &self,
        variable: &str,
        labels: &[String],
        properties: &Option<HashMap<String, Expression>>,
    ) -> Result<ExecutionResult, ExecutionError> {
        let all_nodes = self.graph.get_all_nodes();
        let mut results = Vec::new();

        for node in all_nodes {
            // Check labels
            if !labels.is_empty() && !labels.iter().any(|label| node.labels.contains(label)) {
                continue;
            }

            // Check properties
            if let Some(prop_map) = properties {
                let mut matches = true;
                for (key, expr) in prop_map {
                    if let Expression::Literal(literal) = expr {
                        let expected_value = Self::literal_to_value(literal);
                        if node.properties.get(key) != Some(&expected_value) {
                            matches = false;
                            break;
                        }
                    }
                }
                if !matches {
                    continue;
                }
            }

            // Create binding row with node
            let mut row = HashMap::new();
            row.insert(variable.to_string(), Self::node_to_value(&node));
            results.push(row);
        }

        Ok(results)
    }

    /// Execute node index scan (uses label index)
    fn execute_node_index_scan(
        &self,
        variable: &str,
        labels: &[String],
        properties: &Option<HashMap<String, Expression>>,
    ) -> Result<ExecutionResult, ExecutionError> {
        let mut results = Vec::new();

        if labels.is_empty() {
            // No label specified - fall back to sequential scan
            return self.execute_node_seq_scan(variable, labels, properties);
        }

        // Use index to get nodes with labels
        for label in labels {
            let nodes = self.graph.get_nodes_by_label(label);

            for node in nodes {
                // Check properties
                if let Some(prop_map) = properties {
                    let mut matches = true;
                    for (key, expr) in prop_map {
                        if let Expression::Literal(literal) = expr {
                            let expected_value = Self::literal_to_value(literal);
                            if node.properties.get(key) != Some(&expected_value) {
                                matches = false;
                                break;
                            }
                        }
                    }
                    if !matches {
                        continue;
                    }
                }

                // Create binding row
                let mut row = HashMap::new();
                row.insert(variable.to_string(), Self::node_to_value(&node));
                results.push(row);
            }
        }

        Ok(results)
    }

    /// Execute edge sequential scan
    fn execute_edge_seq_scan(
        &self,
        variable: &str,
        labels: &[String],
        properties: &Option<HashMap<String, Expression>>,
    ) -> Result<ExecutionResult, ExecutionError> {
        let all_edges = self.graph.get_all_edges();
        let mut results = Vec::new();

        for edge in all_edges {
            // Check labels
            if !labels.is_empty() && !labels.contains(&edge.label) {
                continue;
            }

            // Check properties
            if let Some(prop_map) = properties {
                let mut matches = true;
                for (key, expr) in prop_map {
                    if let Expression::Literal(literal) = expr {
                        let expected_value = Self::literal_to_value(literal);
                        if edge.properties.get(key) != Some(&expected_value) {
                            matches = false;
                            break;
                        }
                    }
                }
                if !matches {
                    continue;
                }
            }

            // Create binding row with edge
            let mut row = HashMap::new();
            row.insert(variable.to_string(), Self::edge_to_value(&edge));
            results.push(row);
        }

        Ok(results)
    }

    /// Execute indexed expand (edge traversal from nodes)
    fn execute_indexed_expand(
        &self,
        from_variable: &str,
        edge_variable: &Option<String>,
        to_variable: &str,
        edge_labels: &[String],
        direction: &EdgeDirection,
        _properties: &Option<HashMap<String, Expression>>,
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        // Execute input to get starting nodes
        let input_rows = self.execute_node(input)?;
        let mut results = Vec::new();

        for row in input_rows {
            // Get the from_node from the input row
            let from_value = row.get(from_variable).ok_or_else(|| {
                ExecutionError::RuntimeError(format!(
                    "Variable '{}' not found in input row",
                    from_variable
                ))
            })?;

            let from_node = Self::value_to_node(from_value)?;

            // Find edges based on direction
            let edges = match direction {
                EdgeDirection::Outgoing => {
                    let outgoing = self.graph.get_outgoing_edges(&from_node.id);
                    if edge_labels.is_empty() {
                        outgoing
                    } else {
                        outgoing
                            .into_iter()
                            .filter(|edge| edge_labels.contains(&edge.label))
                            .collect()
                    }
                }
                EdgeDirection::Incoming => {
                    let incoming = self.graph.get_incoming_edges(&from_node.id);
                    if edge_labels.is_empty() {
                        incoming
                    } else {
                        incoming
                            .into_iter()
                            .filter(|edge| edge_labels.contains(&edge.label))
                            .collect()
                    }
                }
                EdgeDirection::Both => {
                    // Both means we need to traverse in both directions
                    let mut all_edges = self.graph.get_outgoing_edges(&from_node.id);
                    all_edges.extend(self.graph.get_incoming_edges(&from_node.id));

                    if edge_labels.is_empty() {
                        all_edges
                    } else {
                        all_edges
                            .into_iter()
                            .filter(|edge| edge_labels.contains(&edge.label))
                            .collect()
                    }
                }
                EdgeDirection::Undirected => {
                    // Undirected is similar to Both
                    let mut all_edges = self.graph.get_outgoing_edges(&from_node.id);
                    all_edges.extend(self.graph.get_incoming_edges(&from_node.id));

                    if edge_labels.is_empty() {
                        all_edges
                    } else {
                        all_edges
                            .into_iter()
                            .filter(|edge| edge_labels.contains(&edge.label))
                            .collect()
                    }
                }
            };

            // For each edge, create a new row with the target node
            for edge in edges {
                let to_node_id = if edge.from_node == from_node.id {
                    &edge.to_node
                } else {
                    &edge.from_node
                };

                if let Some(to_node) = self.graph.get_node(to_node_id) {
                    let mut new_row = row.clone();

                    // Add edge variable if specified
                    if let Some(edge_var) = edge_variable {
                        new_row.insert(edge_var.clone(), Self::edge_to_value(&edge));
                    }

                    // Add to_node
                    new_row.insert(to_variable.to_string(), Self::node_to_value(&to_node));

                    results.push(new_row);
                }
            }
        }

        Ok(results)
    }

    /// Execute filter predicate
    fn execute_filter(
        &self,
        condition: &Expression,
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let input_rows = self.execute_node(input)?;
        let mut results = Vec::new();

        for row in input_rows {
            if Self::evaluate_condition(condition, &row)? {
                results.push(row);
            }
        }

        Ok(results)
    }

    /// Execute projection
    fn execute_project(
        &self,
        expressions: &[ProjectionItem],
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let input_rows = self.execute_node(input)?;
        let mut results = Vec::new();

        for row in input_rows {
            let mut new_row = HashMap::new();

            for proj_item in expressions {
                let value = Self::evaluate_expression(&proj_item.expression, &row)?;
                let alias = proj_item
                    .alias
                    .clone()
                    .unwrap_or_else(|| format!("col_{}", new_row.len()));
                new_row.insert(alias, value);
            }

            results.push(new_row);
        }

        Ok(results)
    }

    /// Execute aggregation
    fn execute_aggregate(
        &self,
        group_by: &[Expression],
        aggregates: &[AggregateItem],
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let input_rows = self.execute_node(input)?;

        if group_by.is_empty() {
            // No GROUP BY - single aggregate result
            let mut result_row = HashMap::new();

            for agg_item in aggregates {
                let value = Self::compute_aggregate(&agg_item.function, &agg_item.expression, &input_rows)?;
                let alias = agg_item
                    .alias
                    .clone()
                    .unwrap_or_else(|| format!("agg_{}", result_row.len()));
                result_row.insert(alias, value);
            }

            Ok(vec![result_row])
        } else {
            // GROUP BY aggregation - use string keys since Vec<Value> doesn't implement Hash
            let mut groups: HashMap<String, (Vec<Value>, Vec<BindingRow>)> = HashMap::new();

            // Group rows by group_by expressions
            for row in input_rows {
                let mut group_key = Vec::new();
                for expr in group_by {
                    let value = Self::evaluate_expression(expr, &row)?;
                    group_key.push(value);
                }

                // Create a string representation for grouping
                let key_str = format!("{:?}", group_key);
                groups
                    .entry(key_str)
                    .or_insert_with(|| (group_key.clone(), Vec::new()))
                    .1
                    .push(row);
            }

            // Compute aggregates for each group
            let mut results = Vec::new();
            for (_key_str, (group_key, group_rows)) in groups {
                let mut result_row = HashMap::new();

                // Add group_by values
                for (i, expr) in group_by.iter().enumerate() {
                    if let Expression::Variable(var) = expr {
                        result_row.insert(var.name.clone(), group_key[i].clone());
                    }
                }

                // Add aggregates
                for agg_item in aggregates {
                    let value = Self::compute_aggregate(&agg_item.function, &agg_item.expression, &group_rows)?;
                    let alias = agg_item
                        .alias
                        .clone()
                        .unwrap_or_else(|| format!("agg_{}", result_row.len()));
                    result_row.insert(alias, value);
                }

                results.push(result_row);
            }

            Ok(results)
        }
    }

    /// Execute sort
    fn execute_sort(
        &self,
        expressions: &[SortItem],
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let mut input_rows = self.execute_node(input)?;

        input_rows.sort_by(|a, b| {
            for sort_item in expressions {
                let a_val = Self::evaluate_expression(&sort_item.expression, a).unwrap_or(Value::Null);
                let b_val = Self::evaluate_expression(&sort_item.expression, b).unwrap_or(Value::Null);

                let cmp = Self::compare_values(&a_val, &b_val);
                let ordering = if sort_item.ascending {
                    cmp
                } else {
                    cmp.reverse()
                };

                if ordering != std::cmp::Ordering::Equal {
                    return ordering;
                }
            }
            std::cmp::Ordering::Equal
        });

        Ok(input_rows)
    }

    /// Execute limit with optional offset
    fn execute_limit(
        &self,
        count: usize,
        offset: Option<usize>,
        input: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let input_rows = self.execute_node(input)?;
        let offset_val = offset.unwrap_or(0);

        Ok(input_rows
            .into_iter()
            .skip(offset_val)
            .take(count)
            .collect())
    }

    /// Execute distinct (remove duplicates)
    fn execute_distinct(&self, input: &PhysicalNode) -> Result<ExecutionResult, ExecutionError> {
        let input_rows = self.execute_node(input)?;
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();

        for row in input_rows {
            // Create a sortable representation of the row for deduplication
            let mut keys: Vec<_> = row.keys().collect();
            keys.sort();
            let key_str = keys
                .iter()
                .map(|k| format!("{}={:?}", k, row.get(*k)))
                .collect::<Vec<_>>()
                .join(",");

            if seen.insert(key_str) {
                results.push(row);
            }
        }

        Ok(results)
    }

    /// Execute nested loop join
    fn execute_nested_loop_join(
        &self,
        _join_type: &crate::plan::logical::JoinType,
        condition: &Option<Expression>,
        left: &PhysicalNode,
        right: &PhysicalNode,
    ) -> Result<ExecutionResult, ExecutionError> {
        let left_rows = self.execute_node(left)?;
        let right_rows = self.execute_node(right)?;
        let mut results = Vec::new();

        for left_row in &left_rows {
            for right_row in &right_rows {
                // Merge rows
                let mut merged_row = left_row.clone();
                merged_row.extend(right_row.clone());

                // Check join condition
                if let Some(cond) = condition {
                    if Self::evaluate_condition(cond, &merged_row)? {
                        results.push(merged_row);
                    }
                } else {
                    // No condition - Cartesian product
                    results.push(merged_row);
                }
            }
        }

        Ok(results)
    }

    // ===== Helper Methods =====

    /// Evaluate a boolean condition expression
    fn evaluate_condition(
        expr: &Expression,
        row: &BindingRow,
    ) -> Result<bool, ExecutionError> {
        let value = Self::evaluate_expression(expr, row)?;
        match value {
            Value::Boolean(b) => Ok(b),
            _ => Ok(false), // Non-boolean values are falsy
        }
    }

    /// Evaluate an expression against a binding row
    fn evaluate_expression(
        expr: &Expression,
        row: &BindingRow,
    ) -> Result<Value, ExecutionError> {
        match expr {
            Expression::Literal(literal) => Ok(Self::literal_to_value(literal)),

            Expression::Variable(var) => row.get(&var.name).cloned().ok_or_else(|| {
                ExecutionError::RuntimeError(format!("Variable '{}' not found", var.name))
            }),

            Expression::PropertyAccess(prop_access) => {
                let obj_value = row.get(&prop_access.object).cloned().ok_or_else(|| {
                    ExecutionError::RuntimeError(format!(
                        "Object '{}' not found",
                        prop_access.object
                    ))
                })?;

                // Extract property from node or edge
                match obj_value {
                    Value::Node(node) => {
                        node.properties
                            .get(&prop_access.property)
                            .cloned()
                            .ok_or_else(|| {
                                ExecutionError::RuntimeError(format!(
                                    "Property '{}' not found",
                                    prop_access.property
                                ))
                            })
                    }
                    Value::Edge(edge) => {
                        edge.properties
                            .get(&prop_access.property)
                            .cloned()
                            .ok_or_else(|| {
                                ExecutionError::RuntimeError(format!(
                                    "Property '{}' not found",
                                    prop_access.property
                                ))
                            })
                    }
                    _ => Err(ExecutionError::RuntimeError(
                        "Expected node or edge value".to_string(),
                    )),
                }
            }

            Expression::Binary(binary_expr) => {
                let left = Self::evaluate_expression(&binary_expr.left, row)?;
                let right = Self::evaluate_expression(&binary_expr.right, row)?;

                match binary_expr.operator {
                    Operator::Equal => Ok(Value::Boolean(left == right)),
                    Operator::NotEqual => Ok(Value::Boolean(left != right)),
                    Operator::GreaterThan => {
                        Ok(Value::Boolean(Self::compare_values(&left, &right) == std::cmp::Ordering::Greater))
                    }
                    Operator::GreaterEqual => {
                        Ok(Value::Boolean(Self::compare_values(&left, &right) != std::cmp::Ordering::Less))
                    }
                    Operator::LessThan => {
                        Ok(Value::Boolean(Self::compare_values(&left, &right) == std::cmp::Ordering::Less))
                    }
                    Operator::LessEqual => {
                        Ok(Value::Boolean(Self::compare_values(&left, &right) != std::cmp::Ordering::Greater))
                    }
                    Operator::And => {
                        let left_bool = matches!(left, Value::Boolean(true));
                        let right_bool = matches!(right, Value::Boolean(true));
                        Ok(Value::Boolean(left_bool && right_bool))
                    }
                    Operator::Or => {
                        let left_bool = matches!(left, Value::Boolean(true));
                        let right_bool = matches!(right, Value::Boolean(true));
                        Ok(Value::Boolean(left_bool || right_bool))
                    }
                    _ => Err(ExecutionError::UnsupportedOperator(format!(
                        "Operator {:?} not supported",
                        binary_expr.operator
                    ))),
                }
            }

            Expression::Unary(unary_expr) => {
                let operand = Self::evaluate_expression(&unary_expr.expression, row)?;
                match unary_expr.operator {
                    Operator::Not => match operand {
                        Value::Boolean(b) => Ok(Value::Boolean(!b)),
                        _ => Ok(Value::Boolean(false)),
                    },
                    Operator::Minus => match operand {
                        Value::Number(n) => Ok(Value::Number(-n)),
                        _ => Err(ExecutionError::RuntimeError(
                            "Cannot negate non-number".to_string(),
                        )),
                    },
                    _ => Err(ExecutionError::UnsupportedOperator(format!(
                        "Unary operator {:?} not supported",
                        unary_expr.operator
                    ))),
                }
            }

            _ => Err(ExecutionError::UnsupportedOperator(format!(
                "Expression {:?} not yet supported",
                expr
            ))),
        }
    }

    /// Compute an aggregate function over rows
    fn compute_aggregate(
        function: &AggregateFunction,
        expr: &Expression,
        rows: &[BindingRow],
    ) -> Result<Value, ExecutionError> {
        match function {
            AggregateFunction::Count => Ok(Value::Number(rows.len() as f64)),

            AggregateFunction::Sum => {
                let mut sum = 0.0;
                for row in rows {
                    let value = Self::evaluate_expression(expr, row)?;
                    if let Value::Number(n) = value {
                        sum += n;
                    }
                }
                Ok(Value::Number(sum))
            }

            AggregateFunction::Avg => {
                if rows.is_empty() {
                    return Ok(Value::Null);
                }
                let mut sum = 0.0;
                let mut count = 0;
                for row in rows {
                    let value = Self::evaluate_expression(expr, row)?;
                    if let Value::Number(n) = value {
                        sum += n;
                        count += 1;
                    }
                }
                if count == 0 {
                    Ok(Value::Null)
                } else {
                    Ok(Value::Number(sum / count as f64))
                }
            }

            AggregateFunction::Min => {
                let mut min_value: Option<Value> = None;
                for row in rows {
                    let value = Self::evaluate_expression(expr, row)?;
                    if let Some(ref current_min) = min_value {
                        if Self::compare_values(&value, current_min) == std::cmp::Ordering::Less {
                            min_value = Some(value);
                        }
                    } else {
                        min_value = Some(value);
                    }
                }
                Ok(min_value.unwrap_or(Value::Null))
            }

            AggregateFunction::Max => {
                let mut max_value: Option<Value> = None;
                for row in rows {
                    let value = Self::evaluate_expression(expr, row)?;
                    if let Some(ref current_max) = max_value {
                        if Self::compare_values(&value, current_max) == std::cmp::Ordering::Greater {
                            max_value = Some(value);
                        }
                    } else {
                        max_value = Some(value);
                    }
                }
                Ok(max_value.unwrap_or(Value::Null))
            }

            AggregateFunction::Collect => {
                let mut values = Vec::new();
                for row in rows {
                    let value = Self::evaluate_expression(expr, row)?;
                    values.push(value);
                }
                Ok(Value::List(values))
            }
        }
    }

    /// Compare two values for ordering
    fn compare_values(a: &Value, b: &Value) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                if a < b {
                    Ordering::Less
                } else if a > b {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Boolean(a), Value::Boolean(b)) => a.cmp(b),
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
            _ => Ordering::Equal,
        }
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

    /// Convert a Node to a Value
    fn node_to_value(node: &Node) -> Value {
        Value::Node(node.clone())
    }

    /// Convert an Edge to a Value
    fn edge_to_value(edge: &Edge) -> Value {
        Value::Edge(edge.clone())
    }

    /// Convert a Value back to a Node (for expand operations)
    fn value_to_node(value: &Value) -> Result<Node, ExecutionError> {
        if let Value::Node(node) = value {
            Ok(node.clone())
        } else {
            Err(ExecutionError::RuntimeError(
                "Expected node value".to_string(),
            ))
        }
    }
}

impl PhysicalPlan {
    /// Execute this physical plan and return variable bindings
    pub fn execute(&self, graph: &GraphCache) -> Result<ExecutionResult, ExecutionError> {
        let executor = PhysicalExecutor::new(graph);
        executor.execute(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::GraphCache;

    #[test]
    fn test_node_seq_scan() {
        let mut graph = GraphCache::new();
        let node = Node {
            id: "node1".to_string(),
            labels: vec!["Person".to_string()],
            properties: HashMap::new(),
        };
        graph.add_node(node.clone()).unwrap();

        let scan = PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 1,
            estimated_cost: 1.0,
        };

        let plan = PhysicalPlan::new(scan);
        let executor = PhysicalExecutor::new(&graph);
        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result[0].contains_key("n"));
    }

    #[test]
    fn test_filter() {
        let mut graph = GraphCache::new();
        let mut props = HashMap::new();
        props.insert("age".to_string(), Value::Number(25.0));

        let node = Node {
            id: "node1".to_string(),
            labels: vec!["Person".to_string()],
            properties: props,
        };
        graph.add_node(node).unwrap();

        // Create a scan node
        let scan = PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 1,
            estimated_cost: 1.0,
        };

        // Create filter: n.age > 20
        let filter = PhysicalNode::Filter {
            condition: Expression::Binary(crate::ast::BinaryExpression {
                left: Box::new(Expression::PropertyAccess(crate::ast::PropertyAccess {
                    object: "n".to_string(),
                    property: "age".to_string(),
                    location: crate::ast::Location::default(),
                })),
                operator: Operator::GreaterThan,
                right: Box::new(Expression::Literal(Literal::Integer(20))),
                location: crate::ast::Location::default(),
            }),
            input: Box::new(scan),
            selectivity: 0.5,
            estimated_rows: 1,
            estimated_cost: 1.0,
        };

        let plan = PhysicalPlan::new(filter);
        let executor = PhysicalExecutor::new(&graph);
        let result = executor.execute(&plan).unwrap();

        assert_eq!(result.len(), 1);
    }
}
