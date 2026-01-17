// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical plan builder - converts AST queries into logical plans
//!
//! This module handles the first phase of query planning: converting
//! the parsed AST into a logical execution plan.
//!
//! Extracted from optimizer.rs as part of Phase 3 refactoring.

use std::collections::HashMap;

use crate::ast::{
    BasicQuery, BinaryExpression, Expression, LetStatement, MatchClause, OrderClause,
    OrderDirection, PathPattern, PatternElement, Query, ReturnClause, SetOperation,
    SetOperationType, Variable,
};
use crate::plan::logical::{
    EntityType, JoinType, LogicalNode, LogicalPlan, ProjectExpression, SortExpression, VariableInfo,
};
use crate::plan::optimizer::PlanningError;

/// Builder for creating logical plans from AST queries
#[derive(Debug)]
pub struct LogicalBuilder {}

/// Planning context holds state during logical plan building
#[derive(Debug, Clone)]
pub struct PlanningContext {
    pub variables: HashMap<String, VariableInfo>,
    pub _next_variable_id: usize,
}

impl LogicalBuilder {
    /// Create a new logical builder
    pub fn new() -> Self {
        Self {}
    }

    /// Build a logical plan from a query
    pub fn build(&mut self, query: &Query) -> Result<LogicalPlan, PlanningError> {
        self.create_logical_plan(query)
    }

    // ========================================================================
    // Main Planning Methods (extracted from optimizer.rs lines 366-1470)
    // ========================================================================

    /// Create logical plan from query AST
    ///
    /// Originally: optimizer.rs line 366
    fn create_logical_plan(&mut self, query: &Query) -> Result<LogicalPlan, PlanningError> {
        match query {
            Query::Basic(basic_query) => self.create_basic_logical_plan(basic_query),
            Query::SetOperation(set_op) => self.create_set_operation_plan(set_op),
            Query::Limited {
                query,
                order_clause,
                limit_clause,
            } => {
                let mut plan = self.create_logical_plan(query)?;

                // Add ORDER BY if present
                if let Some(order) = order_clause {
                    let sort_expressions: Vec<_> = order
                        .items
                        .iter()
                        .map(|item| SortExpression {
                            expression: item.expression.clone(),
                            ascending: matches!(
                                item.direction,
                                crate::ast::OrderDirection::Ascending
                            ),
                        })
                        .collect();

                    plan = plan.apply_sort(sort_expressions);
                }

                // Add LIMIT if present
                if let Some(limit) = limit_clause {
                    plan = plan.apply_limit(limit.count, limit.offset);
                }

                Ok(plan)
            }
            Query::WithQuery(with_query) => {
                // Create a special logical plan node that preserves the original WITH query
                // Create a WithQuery logical node that preserves the original structure
                let with_node = LogicalNode::WithQuery {
                    original_query: Box::new(with_query.clone()),
                };

                // Extract variables from the WITH query for the logical plan
                let mut variables = HashMap::new();

                // Add variables from MATCH clauses
                for segment in &with_query.segments {
                    // Extract variables from match patterns (simplified)
                    for pattern in &segment.match_clause.patterns {
                        for element in &pattern.elements {
                            match element {
                                crate::ast::PatternElement::Node(node) => {
                                    if let Some(var_name) = &node.identifier {
                                        variables.insert(
                                            var_name.clone(),
                                            VariableInfo {
                                                name: var_name.clone(),
                                                entity_type: EntityType::Node,
                                                labels: node.labels.clone(),
                                                required_properties: Vec::new(),
                                            },
                                        );
                                    }
                                }
                                crate::ast::PatternElement::Edge(edge) => {
                                    if let Some(var_name) = &edge.identifier {
                                        variables.insert(
                                            var_name.clone(),
                                            VariableInfo {
                                                name: var_name.clone(),
                                                entity_type: EntityType::Edge,
                                                labels: edge.labels.clone(),
                                                required_properties: Vec::new(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                // Add variables from WITH clauses and RETURN clause
                for segment in &with_query.segments {
                    if let Some(with_clause) = &segment.with_clause {
                        for with_item in &with_clause.items {
                            if let Some(alias) = &with_item.alias {
                                variables.insert(
                                    alias.clone(),
                                    VariableInfo {
                                        name: alias.clone(),
                                        entity_type: EntityType::Node, // Default for computed values
                                        labels: Vec::new(),
                                        required_properties: Vec::new(),
                                    },
                                );
                            }
                        }
                    }
                }

                // Add variables from final RETURN clause
                for return_item in &with_query.final_return.items {
                    if let Some(alias) = &return_item.alias {
                        variables.insert(
                            alias.clone(),
                            VariableInfo {
                                name: alias.clone(),
                                entity_type: EntityType::Node,
                                labels: Vec::new(),
                                required_properties: Vec::new(),
                            },
                        );
                    }
                }

                Ok(LogicalPlan {
                    root: with_node,
                    variables,
                })
            }
            Query::Let(let_stmt) => self.create_let_logical_plan(let_stmt),
            Query::For(_) => {
                // FOR statements don't need optimization yet
                Err(PlanningError::UnsupportedFeature(
                    "FOR queries not yet implemented".to_string(),
                ))
            }
            Query::Filter(_) => {
                // FILTER statements don't need optimization yet
                Err(PlanningError::UnsupportedFeature(
                    "FILTER queries not yet implemented".to_string(),
                ))
            }
            Query::Return(return_query) => self.create_return_logical_plan(return_query),
            Query::Unwind(unwind_stmt) => self.create_unwind_logical_plan(unwind_stmt),
            Query::MutationPipeline(pipeline) => {
                self.create_mutation_pipeline_logical_plan(pipeline)
            }
        }
    }

    // ========================================================================
    // Helper Methods (to be extracted)
    // ========================================================================

    /// Create logical plan for basic query
    /// Originally: optimizer.rs line 512
    fn create_basic_logical_plan(
        &mut self,
        query: &BasicQuery,
    ) -> Result<LogicalPlan, PlanningError> {
        let mut context = PlanningContext {
            variables: HashMap::new(),
            _next_variable_id: 0,
        };

        // Process MATCH clause
        let mut logical_plan = self.plan_match_clause(&query.match_clause, &mut context)?;

        // Process WHERE clause
        if let Some(where_clause) = &query.where_clause {
            logical_plan = logical_plan.apply_filter(where_clause.condition.clone());
        }

        // Process GROUP BY clause (must come before RETURN for aggregation)
        if let Some(group_clause) = &query.group_clause {
            let project_expressions = self.plan_return_clause(&query.return_clause, &context)?;
            let group_expressions =
                self.plan_group_clause_with_aliases(group_clause, &query.return_clause, &context)?;

            // Check if there are any aggregate functions in the project expressions
            let has_aggregates = self.contains_aggregate_functions(&project_expressions);

            // Apply aggregation
            logical_plan = logical_plan
                .apply_aggregation(group_expressions.clone(), project_expressions.clone());

            // If there are no aggregates, we need to add a projection node to ensure
            // the GROUP BY columns are properly projected in the output
            if !has_aggregates {
                // Project the expressions from RETURN clause after grouping
                logical_plan = logical_plan.apply_projection(project_expressions);
            }
        } else {
            // Process RETURN clause - check for implicit aggregation
            let project_expressions = self.plan_return_clause(&query.return_clause, &context)?;

            // Check if RETURN clause contains aggregate functions
            let has_aggregates = self.contains_aggregate_functions(&project_expressions);

            if has_aggregates {
                // Check for mixed expressions (both aggregate and non-aggregate)
                let non_aggregate_expressions =
                    self.extract_non_aggregate_expressions(&project_expressions);

                if !non_aggregate_expressions.is_empty() {
                    // Mixed expressions - add implicit GROUP BY for non-aggregate expressions
                    logical_plan = logical_plan
                        .apply_aggregation(non_aggregate_expressions, project_expressions);
                } else {
                    // Pure aggregation - empty GROUP BY (implicit aggregation)
                    let empty_group_expressions = Vec::new();
                    logical_plan = logical_plan
                        .apply_aggregation(empty_group_expressions, project_expressions);
                }
            } else {
                // Normal projection
                logical_plan = logical_plan.apply_projection(project_expressions);
            }

            // Apply DISTINCT if specified
            if query.return_clause.distinct == crate::ast::DistinctQualifier::Distinct {
                logical_plan = logical_plan.apply_distinct();
            }
        }

        // Process HAVING clause (must come after GROUP BY)
        if let Some(having_clause) = &query.having_clause {
            if query.group_clause.is_none() {
                return Err(PlanningError::InvalidQuery(
                    "HAVING clause requires GROUP BY clause".to_string(),
                ));
            }
            // Resolve aliases in HAVING clause expressions
            let resolved_condition = self.resolve_having_expression_with_aliases(
                &having_clause.condition,
                &query.return_clause,
            );
            logical_plan = logical_plan.apply_having(resolved_condition);
        }

        // Process ORDER BY clause
        if let Some(order_clause) = &query.order_clause {
            let sort_expressions = self.plan_order_clause(order_clause, &context)?;
            logical_plan = logical_plan.apply_sort(sort_expressions);
        }

        // Process LIMIT clause
        if let Some(limit_clause) = &query.limit_clause {
            logical_plan = logical_plan.apply_limit(limit_clause.count, limit_clause.offset);
        }

        // Add variable information to the plan
        for (name, info) in context.variables {
            logical_plan.add_variable(name, info);
        }

        Ok(logical_plan)
    }

    /// Create logical plan for LET statement
    /// Originally: optimizer.rs line 616
    fn create_let_logical_plan(
        &mut self,
        let_stmt: &LetStatement,
    ) -> Result<LogicalPlan, PlanningError> {
        let mut context = PlanningContext {
            variables: HashMap::new(),
            _next_variable_id: 0,
        };

        // LET statements define variables that can be used in subsequent queries
        // For now, we'll create a simple projection plan that evaluates the expressions
        // and makes them available as variables

        let mut project_expressions = Vec::new();

        // Process each variable definition
        for var_def in &let_stmt.variable_definitions {
            // Add the variable to context
            context.variables.insert(
                var_def.variable_name.clone(),
                VariableInfo {
                    name: var_def.variable_name.clone(),
                    entity_type: EntityType::Node, // LET variables can hold any value
                    labels: vec![],
                    required_properties: vec![],
                },
            );

            // Create a projection expression for this variable
            project_expressions.push(ProjectExpression {
                expression: var_def.expression.clone(),
                alias: Some(var_def.variable_name.clone()),
            });
        }

        // Create a logical plan that produces exactly one row for LET statements
        let single_row_node = LogicalNode::SingleRow;
        let mut logical_plan = LogicalPlan::new(single_row_node);

        // Apply projection to compute the variable expressions
        logical_plan = logical_plan.apply_projection(project_expressions);

        // Add variable information to the plan
        for (name, info) in context.variables {
            logical_plan.add_variable(name, info);
        }

        Ok(logical_plan)
    }

    /// Create logical plan for RETURN statement
    /// Originally: optimizer.rs line 667
    fn create_return_logical_plan(
        &mut self,
        return_query: &crate::ast::ReturnQuery,
    ) -> Result<LogicalPlan, PlanningError> {
        let context = PlanningContext {
            variables: HashMap::new(),
            _next_variable_id: 0,
        };

        // Start with a SingleRow node for standalone RETURN statements
        // These queries don't need to scan any graph data
        let single_row_node = LogicalNode::SingleRow;
        let mut logical_plan = LogicalPlan::new(single_row_node);

        // Process RETURN clause - check for implicit aggregation
        let project_expressions = self.plan_return_clause(&return_query.return_clause, &context)?;

        // Check if RETURN clause contains aggregate functions
        let has_aggregates = self.contains_aggregate_functions(&project_expressions);

        if let Some(group_clause) = &return_query.group_clause {
            // Explicit GROUP BY - always apply aggregation with alias resolution
            let group_expressions = self.plan_group_clause_with_aliases(
                group_clause,
                &return_query.return_clause,
                &context,
            )?;
            logical_plan = logical_plan.apply_aggregation(group_expressions, project_expressions);
        } else if has_aggregates {
            // Implicit aggregation - apply aggregation with empty GROUP BY
            let empty_group_expressions = Vec::new();
            logical_plan =
                logical_plan.apply_aggregation(empty_group_expressions, project_expressions);
        } else {
            // Normal projection
            logical_plan = logical_plan.apply_projection(project_expressions);
        }

        // Apply DISTINCT if specified
        if return_query.return_clause.distinct == crate::ast::DistinctQualifier::Distinct {
            logical_plan = logical_plan.apply_distinct();
        }

        // Process HAVING clause if present
        if let Some(having_clause) = &return_query.having_clause {
            // Resolve aliases in HAVING clause expressions
            let resolved_condition = self.resolve_having_expression_with_aliases(
                &having_clause.condition,
                &return_query.return_clause,
            );
            logical_plan = logical_plan.apply_having(resolved_condition);
        }

        // Process ORDER BY clause if present
        if let Some(order_clause) = &return_query.order_clause {
            let sort_expressions: Vec<_> = order_clause
                .items
                .iter()
                .map(|item| SortExpression {
                    expression: item.expression.clone(),
                    ascending: matches!(item.direction, crate::ast::OrderDirection::Ascending),
                })
                .collect();
            logical_plan = logical_plan.apply_sort(sort_expressions);
        }

        // Process LIMIT clause if present
        if let Some(limit_clause) = &return_query.limit_clause {
            logical_plan = logical_plan.apply_limit(limit_clause.count, limit_clause.offset);
        }

        Ok(logical_plan)
    }

    /// Create logical plan for set operation (UNION, etc.)
    /// Originally: optimizer.rs line 742
    fn create_set_operation_plan(
        &mut self,
        set_op: &SetOperation,
    ) -> Result<LogicalPlan, PlanningError> {
        // Create plans for left and right queries
        let left_plan = self.create_logical_plan(&set_op.left)?;
        let right_plan = self.create_logical_plan(&set_op.right)?;

        // Apply the set operation
        let mut plan = match set_op.operation {
            SetOperationType::Union => left_plan.apply_union(right_plan, false),
            SetOperationType::UnionAll => left_plan.apply_union(right_plan, true),
            SetOperationType::Intersect => left_plan.apply_intersect(right_plan, false),
            SetOperationType::IntersectAll => left_plan.apply_intersect(right_plan, true),
            SetOperationType::Except => left_plan.apply_except(right_plan, false),
            SetOperationType::ExceptAll => left_plan.apply_except(right_plan, true),
        };

        // Apply ORDER BY if present
        if let Some(order_clause) = &set_op.order_clause {
            let sort_expressions: Vec<_> = order_clause
                .items
                .iter()
                .map(|item| SortExpression {
                    expression: item.expression.clone(),
                    ascending: matches!(item.direction, crate::ast::OrderDirection::Ascending),
                })
                .collect();
            plan = plan.apply_sort(sort_expressions);
        }

        // Apply LIMIT if present
        if let Some(limit_clause) = &set_op.limit_clause {
            plan = plan.apply_limit(limit_clause.count, limit_clause.offset);
        }

        Ok(plan)
    }

    /// Create logical plan for UNWIND statement
    /// Originally: optimizer.rs line 2700
    fn create_unwind_logical_plan(
        &mut self,
        unwind_stmt: &crate::ast::UnwindStatement,
    ) -> Result<LogicalPlan, PlanningError> {
        // Create the UNWIND logical node
        let unwind_node = LogicalNode::Unwind {
            expression: unwind_stmt.expression.clone(),
            variable: unwind_stmt.variable.clone(),
            input: None, // Standalone UNWIND has no input
        };

        // Create variable info for the unwound variable
        let mut variables = HashMap::new();
        variables.insert(
            unwind_stmt.variable.clone(),
            VariableInfo {
                name: unwind_stmt.variable.clone(),
                entity_type: EntityType::Node, // Treat unwound values as nodes for now
                labels: vec![],
                required_properties: vec![],
            },
        );

        Ok(LogicalPlan {
            root: unwind_node,
            variables,
        })
    }

    /// Create logical plan for mutation pipeline
    /// Originally: optimizer.rs line 2732
    fn create_mutation_pipeline_logical_plan(
        &mut self,
        pipeline: &crate::ast::MutationPipeline,
    ) -> Result<LogicalPlan, PlanningError> {
        // For now, treat as a basic query using the first segment
        if let Some(first_segment) = pipeline.segments.first() {
            // Create a basic query from the first segment
            let basic_query = crate::ast::BasicQuery {
                match_clause: first_segment.match_clause.clone(),
                where_clause: first_segment.where_clause.clone(),
                return_clause: crate::ast::ReturnClause {
                    distinct: crate::ast::DistinctQualifier::None,
                    items: vec![crate::ast::ReturnItem {
                        expression: crate::ast::Expression::Variable(crate::ast::Variable {
                            name: "*".to_string(),
                            location: Default::default(),
                        }),
                        alias: None,
                        location: Default::default(),
                    }],
                    location: Default::default(),
                },
                group_clause: None,
                having_clause: None,
                order_clause: None,
                limit_clause: None,
                location: Default::default(),
            };

            self.create_basic_logical_plan(&basic_query)
        } else {
            Err(PlanningError::InvalidQuery(
                "Mutation pipeline requires at least one segment".to_string(),
            ))
        }
    }

    // ========================================================================
    // Planning Helper Methods (to be extracted)
    // ========================================================================

    /// Plan MATCH clause into logical operations
    /// Originally: optimizer.rs line 782
    ///
    /// Note: This is a simplified version that creates basic logical plans.
    /// Pattern optimization (comma-separated pattern bug fix) is handled
    /// separately in the optimizer using the PatternOptimizationPipeline.
    fn plan_match_clause(
        &mut self,
        match_clause: &MatchClause,
        context: &mut PlanningContext,
    ) -> Result<LogicalPlan, PlanningError> {
        if match_clause.patterns.is_empty() {
            return Err(PlanningError::InvalidQuery(
                "Empty MATCH clause".to_string(),
            ));
        }

        // Handle single pattern case
        if match_clause.patterns.len() == 1 {
            let pattern = &match_clause.patterns[0];

            // Extract variables from pattern
            self.extract_pattern_variables(pattern, context)?;

            // Convert pattern to logical plan
            let root_node =
                LogicalPlan::from_path_pattern(pattern).map_err(PlanningError::InvalidQuery)?;

            return Ok(LogicalPlan::new(root_node));
        }

        // Handle multiple patterns - create cross-product joins
        // Note: The optimizer can later optimize these using pattern analysis
        let mut current_plan: Option<LogicalPlan> = None;

        for pattern in &match_clause.patterns {
            // Extract variables from this pattern
            self.extract_pattern_variables(pattern, context)?;

            // Convert pattern to logical plan node
            let pattern_node =
                LogicalPlan::from_path_pattern(pattern).map_err(PlanningError::InvalidQuery)?;
            let pattern_plan = LogicalPlan::new(pattern_node);

            match current_plan {
                None => {
                    // First pattern becomes the base plan
                    current_plan = Some(pattern_plan);
                }
                Some(existing_plan) => {
                    // Create cross-product join with previous patterns
                    let join_node = LogicalNode::Join {
                        join_type: JoinType::Cross, // Cross product for independent patterns
                        condition: None,            // No join condition for cross product
                        left: Box::new(existing_plan.root),
                        right: Box::new(pattern_plan.root),
                    };

                    // Merge variables from both plans
                    let mut merged_variables = existing_plan.variables.clone();
                    for (name, info) in pattern_plan.variables {
                        merged_variables.insert(name, info);
                    }

                    current_plan = Some(LogicalPlan {
                        root: join_node,
                        variables: merged_variables,
                    });
                }
            }
        }

        current_plan.ok_or_else(|| PlanningError::InvalidQuery("No patterns processed".to_string()))
    }

    /// Plan RETURN clause
    /// Originally: optimizer.rs line 1307
    fn plan_return_clause(
        &self,
        return_clause: &ReturnClause,
        _context: &PlanningContext,
    ) -> Result<Vec<ProjectExpression>, PlanningError> {
        let mut expressions = Vec::new();

        for item in &return_clause.items {
            expressions.push(ProjectExpression {
                expression: item.expression.clone(),
                alias: item.alias.clone(),
            });
        }

        Ok(expressions)
    }

    /// Extract pattern variables into context
    /// Originally: optimizer.rs line 1272
    fn extract_pattern_variables(
        &self,
        pattern: &PathPattern,
        context: &mut PlanningContext,
    ) -> Result<(), PlanningError> {
        for element in &pattern.elements {
            match element {
                PatternElement::Node(node) => {
                    if let Some(identifier) = &node.identifier {
                        let var_info = VariableInfo {
                            name: identifier.clone(),
                            entity_type: EntityType::Node,
                            labels: node.labels.clone(),
                            required_properties: vec![], // TODO: Extract from properties
                        };
                        context.variables.insert(identifier.clone(), var_info);
                    }
                }
                PatternElement::Edge(edge) => {
                    if let Some(identifier) = &edge.identifier {
                        let var_info = VariableInfo {
                            name: identifier.clone(),
                            entity_type: EntityType::Edge,
                            labels: edge.labels.clone(),
                            required_properties: vec![], // TODO: Extract from properties
                        };
                        context.variables.insert(identifier.clone(), var_info);
                    }
                }
            }
        }
        Ok(())
    }

    /// Plan GROUP BY clause with aliases
    /// Originally: optimizer.rs line 1324
    fn plan_group_clause_with_aliases(
        &self,
        group_clause: &crate::ast::GroupClause,
        return_clause: &ReturnClause,
        _context: &PlanningContext,
    ) -> Result<Vec<Expression>, PlanningError> {
        let mut resolved_expressions = Vec::new();

        for group_expr in &group_clause.expressions {
            match group_expr {
                Expression::Variable(Variable { name, .. }) => {
                    // Try to find this variable name as an alias in the RETURN clause
                    let mut found_alias = false;
                    for return_item in &return_clause.items {
                        if let Some(alias) = &return_item.alias {
                            if alias == name {
                                // Found the alias! Use the actual expression instead of the variable
                                resolved_expressions.push(return_item.expression.clone());
                                found_alias = true;
                                break;
                            }
                        }
                    }

                    if !found_alias {
                        // Alias not found, keep the original expression (might be a real variable)
                        resolved_expressions.push(group_expr.clone());
                    }
                }
                _ => {
                    // Non-variable expression, use as-is
                    resolved_expressions.push(group_expr.clone());
                }
            }
        }

        Ok(resolved_expressions)
    }

    /// Plan ORDER BY clause
    /// Originally: optimizer.rs line 1449
    fn plan_order_clause(
        &self,
        order_clause: &OrderClause,
        _context: &PlanningContext,
    ) -> Result<Vec<SortExpression>, PlanningError> {
        let mut sort_expressions = Vec::new();

        for item in &order_clause.items {
            sort_expressions.push(SortExpression {
                expression: item.expression.clone(),
                ascending: match item.direction {
                    OrderDirection::Ascending => true,
                    OrderDirection::Descending => false,
                },
            });
        }

        Ok(sort_expressions)
    }

    /// Check if expressions contain aggregate functions
    /// Originally: optimizer.rs line 2019
    fn contains_aggregate_functions(&self, expressions: &[ProjectExpression]) -> bool {
        expressions
            .iter()
            .any(|expr| self.is_aggregate_expression(&expr.expression))
    }

    /// Check if an expression contains aggregate functions
    /// Originally: optimizer.rs line 2026
    fn is_aggregate_expression(&self, expr: &Expression) -> bool {
        match expr {
            Expression::FunctionCall(func_call) => {
                // Check if this is an aggregate function (case insensitive)
                matches!(
                    func_call.name.to_uppercase().as_str(),
                    "COUNT" | "SUM" | "AVG" | "AVERAGE" | "MIN" | "MAX" | "COLLECT"
                )
            }
            Expression::Binary(binary) => {
                // Recursively check operands
                self.is_aggregate_expression(&binary.left)
                    || self.is_aggregate_expression(&binary.right)
            }
            Expression::Case(_case_expr) => {
                // Check all branches of CASE expression - simplified for now
                // Note: The exact structure depends on how CaseExpression is defined
                // For safety, return false for now - this can be expanded later
                false
            }
            // Add other expression types that could contain function calls
            _ => false,
        }
    }

    /// Extract non-aggregate expressions
    /// Originally: optimizer.rs line 2052
    fn extract_non_aggregate_expressions(
        &self,
        expressions: &[ProjectExpression],
    ) -> Vec<Expression> {
        let mut group_expressions = Vec::new();

        for expr in expressions {
            self.collect_non_aggregate_subexpressions(&expr.expression, &mut group_expressions);
        }

        // Note: We skip deduplication for now since Expression doesn't implement PartialEq
        // In practice, duplicates are rare and don't affect correctness
        group_expressions
    }

    /// Recursively collect non-aggregate sub-expressions from a given expression
    /// Originally: optimizer.rs line 2068
    fn collect_non_aggregate_subexpressions(
        &self,
        expr: &Expression,
        group_expressions: &mut Vec<Expression>,
    ) {
        match expr {
            Expression::FunctionCall(func_call) => {
                // If it's an aggregate function, don't add it to GROUP BY
                if matches!(
                    func_call.name.to_uppercase().as_str(),
                    "COUNT" | "SUM" | "AVG" | "AVERAGE" | "MIN" | "MAX" | "COLLECT"
                ) {
                    return;
                }
                // Non-aggregate function - add the whole expression
                group_expressions.push(expr.clone());
            }
            Expression::Binary(binary) => {
                // For binary expressions, check if they contain aggregates
                if !self.is_aggregate_expression(expr) {
                    // If the whole expression is non-aggregate, add it
                    group_expressions.push(expr.clone());
                } else {
                    // If it contains aggregates, recursively check parts
                    self.collect_non_aggregate_subexpressions(&binary.left, group_expressions);
                    self.collect_non_aggregate_subexpressions(&binary.right, group_expressions);
                }
            }
            Expression::ArrayIndex(_array_index) => {
                // Array indexing is typically non-aggregate
                group_expressions.push(expr.clone());
            }
            // For simple expressions (variables, literals, etc.), add them to GROUP BY
            Expression::Variable(_) | Expression::Literal(_) => {
                group_expressions.push(expr.clone());
            }
            // For more complex expressions, be conservative and add them
            _ => {
                if !self.is_aggregate_expression(expr) {
                    group_expressions.push(expr.clone());
                }
            }
        }
    }

    /// Resolve HAVING expression with aliases
    /// Originally: optimizer.rs line 1366
    fn resolve_having_expression_with_aliases(
        &self,
        expr: &Expression,
        return_clause: &ReturnClause,
    ) -> Expression {
        match expr {
            Expression::Variable(Variable { name, .. }) => {
                // Try to find this variable name as an alias in the RETURN clause
                for return_item in &return_clause.items {
                    if let Some(alias) = &return_item.alias {
                        if alias == name {
                            // Found the alias! Use a variable reference to the alias instead
                            return Expression::Variable(Variable {
                                name: alias.clone(),
                                location: crate::ast::Location::default(),
                            });
                        }
                    }
                }
                // Not found as alias, keep as-is
                expr.clone()
            }
            Expression::FunctionCall(func_call) => {
                // Check if this function call appears in the RETURN clause by comparing manually
                for return_item in &return_clause.items {
                    if let Expression::FunctionCall(return_func_call) = &return_item.expression {
                        // Compare function name and arguments
                        if return_func_call.name == func_call.name
                            && return_func_call.arguments.len() == func_call.arguments.len()
                        {
                            // For now, assume they match if name and arg count match
                            // (A more sophisticated comparison would be needed for complex cases)
                            if let Some(alias) = &return_item.alias {
                                return Expression::Variable(Variable {
                                    name: alias.clone(),
                                    location: crate::ast::Location::default(),
                                });
                            }
                        }
                    }
                }
                // Recursively resolve arguments
                let mut resolved_args = Vec::new();
                for arg in &func_call.arguments {
                    resolved_args
                        .push(self.resolve_having_expression_with_aliases(arg, return_clause));
                }
                Expression::FunctionCall(crate::ast::FunctionCall {
                    name: func_call.name.clone(),
                    arguments: resolved_args,
                    distinct: func_call.distinct.clone(),
                    location: func_call.location.clone(),
                })
            }
            Expression::Binary(binary_expr) => Expression::Binary(BinaryExpression {
                left: Box::new(
                    self.resolve_having_expression_with_aliases(&binary_expr.left, return_clause),
                ),
                operator: binary_expr.operator.clone(),
                right: Box::new(
                    self.resolve_having_expression_with_aliases(&binary_expr.right, return_clause),
                ),
                location: binary_expr.location.clone(),
            }),
            Expression::Unary(unary_expr) => {
                Expression::Unary(crate::ast::UnaryExpression {
                    operator: unary_expr.operator.clone(),
                    expression: Box::new(self.resolve_having_expression_with_aliases(
                        &unary_expr.expression,
                        return_clause,
                    )),
                    location: unary_expr.location.clone(),
                })
            }
            _ => {
                // For other expressions, return as-is
                expr.clone()
            }
        }
    }
}

impl Default for LogicalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanningContext {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            _next_variable_id: 0,
        }
    }
}

impl Default for PlanningContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expression, Literal, Location, Variable};

    fn dummy_location() -> Location {
        Location {
            line: 1,
            column: 1,
            offset: 0,
        }
    }

    #[test]
    fn test_logical_builder_creation() {
        let builder = LogicalBuilder::new();
        // Just verify it can be created without panic
        assert_eq!(
            std::mem::size_of_val(&builder),
            std::mem::size_of::<LogicalBuilder>()
        );
    }

    #[test]
    fn test_planning_context_creation() {
        let context = PlanningContext::new();
        assert!(context.variables.is_empty());
    }

    #[test]
    fn test_contains_aggregate_functions_with_count() {
        let builder = LogicalBuilder::new();
        let expressions = vec![ProjectExpression {
            expression: Expression::FunctionCall(crate::ast::FunctionCall {
                name: "COUNT".to_string(),
                arguments: vec![Expression::Variable(Variable {
                    name: "n".to_string(),
                    location: dummy_location(),
                })],
                distinct: crate::ast::DistinctQualifier::None,
                location: dummy_location(),
            }),
            alias: Some("count".to_string()),
        }];

        assert!(builder.contains_aggregate_functions(&expressions));
    }

    #[test]
    fn test_contains_aggregate_functions_with_sum() {
        let builder = LogicalBuilder::new();
        let expressions = vec![ProjectExpression {
            expression: Expression::FunctionCall(crate::ast::FunctionCall {
                name: "SUM".to_string(),
                arguments: vec![Expression::Variable(Variable {
                    name: "n".to_string(),
                    location: dummy_location(),
                })],
                distinct: crate::ast::DistinctQualifier::None,
                location: dummy_location(),
            }),
            alias: Some("total".to_string()),
        }];

        assert!(builder.contains_aggregate_functions(&expressions));
    }

    #[test]
    fn test_contains_aggregate_functions_without_aggregates() {
        let builder = LogicalBuilder::new();
        let expressions = vec![ProjectExpression {
            expression: Expression::Variable(Variable {
                name: "n".to_string(),
                location: dummy_location(),
            }),
            alias: None,
        }];

        assert!(!builder.contains_aggregate_functions(&expressions));
    }

    #[test]
    fn test_is_aggregate_expression_count() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "COUNT".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_avg() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "AVG".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_min() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "MIN".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_max() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "MAX".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_collect() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "COLLECT".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_case_insensitive() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "count".to_string(), // lowercase
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_non_aggregate() {
        let builder = LogicalBuilder::new();
        let expr = Expression::FunctionCall(crate::ast::FunctionCall {
            name: "toUpper".to_string(),
            arguments: vec![],
            distinct: crate::ast::DistinctQualifier::None,
            location: dummy_location(),
        });

        assert!(!builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_variable() {
        let builder = LogicalBuilder::new();
        let expr = Expression::Variable(Variable {
            name: "n".to_string(),
            location: dummy_location(),
        });

        assert!(!builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_is_aggregate_expression_literal() {
        let builder = LogicalBuilder::new();
        let expr = Expression::Literal(Literal::Integer(42));

        assert!(!builder.is_aggregate_expression(&expr));
    }

    #[test]
    fn test_extract_non_aggregate_expressions_mixed() {
        let builder = LogicalBuilder::new();
        let expressions = vec![
            ProjectExpression {
                expression: Expression::Variable(Variable {
                    name: "n".to_string(),
                    location: dummy_location(),
                }),
                alias: None,
            },
            ProjectExpression {
                expression: Expression::FunctionCall(crate::ast::FunctionCall {
                    name: "COUNT".to_string(),
                    arguments: vec![],
                    distinct: crate::ast::DistinctQualifier::None,
                    location: dummy_location(),
                }),
                alias: Some("count".to_string()),
            },
        ];

        let non_aggregates = builder.extract_non_aggregate_expressions(&expressions);
        assert_eq!(non_aggregates.len(), 1);
    }

    #[test]
    fn test_extract_non_aggregate_expressions_all_aggregates() {
        let builder = LogicalBuilder::new();
        let expressions = vec![ProjectExpression {
            expression: Expression::FunctionCall(crate::ast::FunctionCall {
                name: "COUNT".to_string(),
                arguments: vec![],
                distinct: crate::ast::DistinctQualifier::None,
                location: dummy_location(),
            }),
            alias: Some("count".to_string()),
        }];

        let non_aggregates = builder.extract_non_aggregate_expressions(&expressions);
        assert_eq!(non_aggregates.len(), 0);
    }

    #[test]
    fn test_extract_non_aggregate_expressions_all_non_aggregates() {
        let builder = LogicalBuilder::new();
        let expressions = vec![
            ProjectExpression {
                expression: Expression::Variable(Variable {
                    name: "n".to_string(),
                    location: dummy_location(),
                }),
                alias: None,
            },
            ProjectExpression {
                expression: Expression::Literal(Literal::Integer(42)),
                alias: None,
            },
        ];

        let non_aggregates = builder.extract_non_aggregate_expressions(&expressions);
        assert_eq!(non_aggregates.len(), 2);
    }

    // Note: Full query building tests are thoroughly covered in 461 existing integration tests.
    // Unit tests above focus on pure functions without complex AST construction.
}
