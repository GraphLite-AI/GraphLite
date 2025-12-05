// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query optimizer and planner
//!
//! This module provides the main query planning interface that converts
//! AST queries into optimized physical execution plans.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

use crate::ast::{
    BasicQuery, BinaryExpression, Document, Expression, LetStatement, MatchClause, Operator,
    OrderClause, OrderDirection, PathPattern, PatternElement, Query, ReturnClause, SetOperation,
    SetOperationType, Variable,
};
use crate::plan::cost::{CostEstimate, CostModel, Statistics};
use crate::plan::logical::{
    EntityType, JoinType, LogicalNode, LogicalPlan, ProjectExpression, SortExpression,
    TextSearchType, VariableInfo,
};
use crate::plan::pattern_optimization::integration::PatternOptimizationPipeline;
use crate::plan::physical::{PhysicalNode, PhysicalPlan};
use crate::plan::trace::{PlanTrace, PlanTracer, PlanningPhase, TraceMetadata};
use crate::storage::GraphCache;

/// Main query planner that orchestrates the planning process
#[derive(Debug)]
pub struct QueryPlanner {
    cost_model: CostModel,
    statistics: Statistics,
    optimization_level: OptimizationLevel,
    avoid_index_scan: bool,
    /// Pattern optimization pipeline for fixing comma-separated pattern bugs
    pattern_optimizer: PatternOptimizationPipeline,
}

/// Optimization levels for query planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationLevel {
    None,       // No optimization, direct translation
    Basic,      // Basic optimizations (predicate pushdown, projection elimination)
    Advanced,   // Advanced optimizations (join reordering, cost-based selection)
    Aggressive, // Aggressive optimizations (experimental features)
}

/// Planning errors
#[derive(Error, Debug)]
pub enum PlanningError {
    #[error("Invalid query structure: {0}")]
    InvalidQuery(String),

    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
}

/// Planning context holds state during planning
#[derive(Debug, Clone)]
struct PlanningContext {
    variables: HashMap<String, VariableInfo>,
    _next_variable_id: usize,
}

/// Query plan with alternatives for cost comparison
///
/// **Planned Feature** - Multiple plan alternatives for cost-based selection
/// See ROADMAP.md: "Advanced Query Optimizer"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct QueryPlanAlternatives {
    pub plans: Vec<PhysicalPlan>,
    pub best_plan: PhysicalPlan,
    pub planning_time_ms: u64,
}

impl QueryPlanner {
    /// Create a new query planner with default settings
    pub fn new() -> Self {
        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: OptimizationLevel::Basic,
            avoid_index_scan: true, // Default to avoiding index scans
            pattern_optimizer: PatternOptimizationPipeline::new(),
        }
    }

    /// Create a query planner with specific optimization level
    #[allow(dead_code)] // ROADMAP v0.5.0 - Multi-level optimization strategies (None, Basic, Aggressive)
    pub fn with_optimization_level(level: OptimizationLevel) -> Self {
        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: level,
            avoid_index_scan: true, // Default to avoiding index scans
            pattern_optimizer: PatternOptimizationPipeline::new(),
        }
    }

    /// Update statistics from a graph
    #[allow(dead_code)] // ROADMAP v0.5.0 - Statistics-driven query optimization
    pub fn update_statistics(&mut self, graph: &GraphCache) {
        self.statistics.update_from_graph(graph);
    }

    /// Set whether to avoid index scans
    #[allow(dead_code)] // ROADMAP v0.5.0 - Runtime control of index scan preference
    pub fn set_avoid_index_scan(&mut self, avoid: bool) {
        self.avoid_index_scan = avoid;
    }

    /// Get whether index scans are avoided
    #[allow(dead_code)] // ROADMAP v0.5.0 - Index scan configuration inspection
    pub fn get_avoid_index_scan(&self) -> bool {
        self.avoid_index_scan
    }

    /// Optimize query plan with available indexes
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index-aware query optimization (see ROADMAP.md §6)
    pub fn optimize_with_indexes(
        &self,
        logical_plan: LogicalPlan,
        available_indexes: &[IndexInfo],
    ) -> Result<PhysicalPlan, PlanningError> {
        // Create index-aware optimizer
        let mut optimizer = IndexAwareOptimizer::new(available_indexes, &self.cost_model);

        // Apply index-aware transformations
        let optimized_logical = optimizer.apply_index_rules(logical_plan)?;

        // Convert to physical plan with index operations
        let physical_plan =
            self.create_physical_plan_with_indexes(optimized_logical, available_indexes)?;

        // Cost-based selection among alternatives
        let best_plan = optimizer.select_best_plan(physical_plan, &self.statistics)?;

        Ok(best_plan)
    }

    /// Create a query planner with index scan avoidance setting
    #[allow(dead_code)] // ROADMAP v0.4.0 - Configuration for index scan behavior
    pub fn with_index_scan_setting(avoid_index_scan: bool) -> Self {
        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: OptimizationLevel::Basic,
            avoid_index_scan,
            pattern_optimizer: PatternOptimizationPipeline::new(),
        }
    }

    /// Plan a query from AST document
    pub fn plan_query(&mut self, document: &Document) -> Result<PhysicalPlan, PlanningError> {
        // Extract query from document
        let query = match &document.statement {
            crate::ast::Statement::Query(q) => q,
            _ => {
                return Err(PlanningError::InvalidQuery(
                    "Document does not contain a query statement".to_string(),
                ))
            }
        };

        // Generate logical plan
        let logical_plan = self.create_logical_plan(query)?;

        // Optimize logical plan
        let mut optimized_logical = self.optimize_logical_plan(logical_plan)?;

        // Apply index-aware transformations (text_search, etc.)
        // Create a temporary IndexAwareOptimizer with empty indexes
        // This allows text_search transformation to work even without registered indexes
        let empty_indexes: Vec<IndexInfo> = vec![];
        let mut index_optimizer = IndexAwareOptimizer::new(&empty_indexes, &self.cost_model);
        optimized_logical = index_optimizer.apply_index_rules(optimized_logical)?;

        // Convert to physical plan
        let physical_plan = self.create_physical_plan(optimized_logical)?;

        // Optimize physical plan
        let optimized_physical = self.optimize_physical_plan(physical_plan)?;

        Ok(optimized_physical)
    }

    /// Plan a query with detailed tracing for EXPLAIN
    pub fn plan_query_with_trace(
        &mut self,
        document: &Document,
    ) -> Result<PlanTrace, PlanningError> {
        let mut tracer = PlanTracer::new();

        // Parse phase (already done, but record it)
        tracer.trace_step(
            PlanningPhase::Parsing,
            "Parse GQL query into AST".to_string(),
            TraceMetadata::empty(),
        );

        // Extract query from document
        let query = match &document.statement {
            crate::ast::Statement::Query(q) => q,
            _ => {
                return Err(PlanningError::InvalidQuery(
                    "Document does not contain a query statement".to_string(),
                ))
            }
        };

        // Generate logical plan
        tracer.start_step(
            PlanningPhase::LogicalPlanGeneration,
            "Create logical plan from AST".to_string(),
        );
        let logical_plan = self.create_logical_plan(query)?;
        tracer.end_step(
            PlanningPhase::LogicalPlanGeneration,
            "Logical plan created successfully".to_string(),
            None,
            None,
            None,
            TraceMetadata::with_estimates(logical_plan.root.estimate_cardinality(), 0.0),
        );

        // Optimize logical plan
        tracer.start_step(
            PlanningPhase::LogicalOptimization,
            "Apply logical optimizations".to_string(),
        );
        let optimized_logical = self.optimize_logical_plan(logical_plan.clone())?;
        tracer.end_step(
            PlanningPhase::LogicalOptimization,
            format!(
                "Applied {} optimization level",
                self.optimization_level_name()
            ),
            None,
            None,
            None,
            TraceMetadata::with_optimization(self.optimization_level_name()),
        );

        // Convert to physical plan
        tracer.start_step(
            PlanningPhase::PhysicalPlanGeneration,
            "Convert to physical plan".to_string(),
        );
        let physical_plan = self.create_physical_plan(optimized_logical.clone())?;
        tracer.end_step(
            PlanningPhase::PhysicalPlanGeneration,
            "Physical plan generated with operator selection".to_string(),
            None,
            None,
            Some(self.estimate_plan_cost(&physical_plan)),
            TraceMetadata::with_estimates(
                physical_plan.estimated_rows,
                physical_plan.estimated_cost,
            ),
        );

        // Optimize physical plan
        tracer.start_step(
            PlanningPhase::PhysicalOptimization,
            "Apply physical optimizations".to_string(),
        );
        let optimized_physical = self.optimize_physical_plan(physical_plan)?;
        tracer.end_step(
            PlanningPhase::PhysicalOptimization,
            format!(
                "Physical optimization complete (avoid_index_scan: {})",
                self.avoid_index_scan
            ),
            None,
            None,
            Some(self.estimate_plan_cost(&optimized_physical)),
            TraceMetadata::with_estimates(
                optimized_physical.estimated_rows,
                optimized_physical.estimated_cost,
            ),
        );

        // Final cost estimation
        tracer.trace_step(
            PlanningPhase::CostEstimation,
            format!(
                "Final cost estimation: {:.2}",
                optimized_physical.estimated_cost
            ),
            TraceMetadata::with_estimates(
                optimized_physical.estimated_rows,
                optimized_physical.estimated_cost,
            ),
        );

        Ok(tracer.finalize(optimized_logical, optimized_physical))
    }

    /// Get the name of the current optimization level
    fn optimization_level_name(&self) -> String {
        match self.optimization_level {
            OptimizationLevel::None => "None".to_string(),
            OptimizationLevel::Basic => "Basic".to_string(),
            OptimizationLevel::Advanced => "Advanced".to_string(),
            OptimizationLevel::Aggressive => "Aggressive".to_string(),
        }
    }

    /// Plan a query with multiple alternatives for comparison
    #[allow(dead_code)] // ROADMAP v0.3.0 - Multi-plan generation for cost-based optimization (see ROADMAP.md §5)
    pub fn plan_query_with_alternatives(
        &mut self,
        document: &Document,
    ) -> Result<QueryPlanAlternatives, PlanningError> {
        let _start_time = std::time::Instant::now();

        // Extract query from document
        let query = match &document.statement {
            crate::ast::Statement::Query(q) => q,
            _ => {
                return Err(PlanningError::InvalidQuery(
                    "Document does not contain a query statement".to_string(),
                ))
            }
        };

        // Generate logical plan
        let logical_plan = self.create_logical_plan(query)?;

        // Generate multiple physical plan alternatives
        let mut physical_plans = Vec::new();

        // Plan 1: Basic plan without heavy optimization
        let basic_physical = PhysicalPlan::from_logical(&logical_plan);
        physical_plans.push(basic_physical.clone());

        // Plan 2: Optimized plan
        let optimized_logical = self.optimize_logical_plan(logical_plan.clone())?;
        let optimized_physical = self.create_physical_plan(optimized_logical)?;
        physical_plans.push(optimized_physical.clone());

        // Plan 3: Alternative join orders (if applicable)
        if matches!(
            self.optimization_level,
            OptimizationLevel::Advanced | OptimizationLevel::Aggressive
        ) {
            if let Ok(alternative) = self.generate_join_alternatives(&logical_plan) {
                physical_plans.push(alternative);
            }
        }

        // Select best plan based on cost
        let best_plan = self.select_best_plan(&physical_plans)?;

        let planning_time = _start_time.elapsed().as_millis() as u64;

        Ok(QueryPlanAlternatives {
            plans: physical_plans,
            best_plan,
            planning_time_ms: planning_time,
        })
    }

    /// Create logical plan from query AST
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
                use crate::plan::logical::{EntityType, LogicalNode, LogicalPlan, VariableInfo};
                use std::collections::HashMap;

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

    /// Create logical plan for basic query
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

    /// Create logical plan for standalone RETURN query
    fn create_return_logical_plan(
        &self,
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

    /// Create logical plan for set operations
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

    /// Plan MATCH clause into logical operations
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

        // Handle single pattern case (existing functionality)
        if match_clause.patterns.len() == 1 {
            let pattern = &match_clause.patterns[0];

            // Extract variables from pattern
            self.extract_pattern_variables(pattern, context)?;

            // Convert pattern to logical plan
            let root_node =
                LogicalPlan::from_path_pattern(pattern).map_err(PlanningError::InvalidQuery)?;

            return Ok(LogicalPlan::new(root_node));
        }

        // 🔧 PATTERN OPTIMIZATION FIX: Replace Cartesian product with intelligent optimization
        // This is the core fix for the comma-separated pattern bug!

        // Check if pattern optimization should be applied
        if self.should_apply_pattern_optimization(match_clause) {
            // Try to optimize comma-separated patterns
            match self.optimize_comma_separated_patterns(match_clause, context) {
                Ok(optimized_plan) => {
                    log::debug!("✅ Pattern optimization applied! Comma-separated patterns optimized to avoid Cartesian product.");
                    return Ok(optimized_plan);
                }
                Err(e) => {
                    log::debug!(
                        "⚠️  Pattern optimization failed ({}), falling back to original behavior",
                        e
                    );
                    // Fall through to original Cartesian product logic
                }
            }
        }

        // Original logic (fallback): Handle multiple patterns - create cross-product joins
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

    /// Check if pattern optimization should be applied to this MATCH clause
    fn should_apply_pattern_optimization(&self, match_clause: &MatchClause) -> bool {
        // ⚡ CRITICAL BUG FIX: Apply pattern optimization at ALL levels since this fixes incorrect results
        // The comma-separated pattern bug creates wrong results, not just inefficient ones
        // This is a correctness fix, not just a performance optimization
        // Only optimize if we have multiple patterns (the bug condition)
        match_clause.patterns.len() > 1 &&
        // Don't optimize too many patterns (avoid exponential complexity)
        match_clause.patterns.len() <= 10
    }

    /// Optimize comma-separated patterns using our pattern optimization framework
    fn optimize_comma_separated_patterns(
        &mut self,
        match_clause: &MatchClause,
        context: &mut PlanningContext,
    ) -> Result<LogicalPlan, PlanningError> {
        // Extract variables from all patterns first
        for pattern in &match_clause.patterns {
            self.extract_pattern_variables(pattern, context)?;
        }

        // Create a dummy query for the optimization pipeline
        // (In a full implementation, this would be more sophisticated)
        let dummy_query = Query::Basic(BasicQuery {
            match_clause: match_clause.clone(),
            where_clause: None,
            return_clause: ReturnClause {
                distinct: crate::ast::DistinctQualifier::None,
                items: vec![],
                location: crate::ast::Location::default(),
            },
            group_clause: None,
            having_clause: None,
            order_clause: None,
            limit_clause: None,
            location: crate::ast::Location::default(),
        });

        // Create base logical plan from first pattern
        let base_pattern = &match_clause.patterns[0];
        let base_node =
            LogicalPlan::from_path_pattern(base_pattern).map_err(PlanningError::InvalidQuery)?;
        let base_logical_plan = LogicalPlan::new(base_node);

        // Create base physical plan for optimization
        let base_physical_plan = self.create_simple_physical_plan(&base_logical_plan)?;

        // Apply pattern optimization
        match self.pattern_optimizer.optimize_query(
            &dummy_query,
            &base_logical_plan,
            &base_physical_plan,
        ) {
            Ok(result) => {
                if result.optimized {
                    log::debug!("🎉 Pattern optimization successful: {}", result.explanation);
                    log::debug!(
                        "   Performance improvement: {}",
                        result.improvement.describe()
                    );

                    // Convert optimized physical plan back to logical plan
                    // For this demonstration, we'll create a simplified connected logical plan
                    self.create_optimized_logical_plan(match_clause)
                } else {
                    Err(PlanningError::InvalidQuery(format!(
                        "Pattern optimization skipped: {}",
                        result.explanation
                    )))
                }
            }
            Err(e) => Err(PlanningError::InvalidQuery(format!(
                "Pattern optimization error: {}",
                e
            ))),
        }
    }

    /// Create a simple physical plan for optimization analysis
    fn create_simple_physical_plan(
        &self,
        logical_plan: &LogicalPlan,
    ) -> Result<PhysicalNode, PlanningError> {
        // Convert the logical plan root node to a physical node
        self.logical_to_physical_node(&logical_plan.root)
    }

    /// Convert a logical node to a physical node
    fn logical_to_physical_node(
        &self,
        logical_node: &LogicalNode,
    ) -> Result<PhysicalNode, PlanningError> {
        match logical_node {
            LogicalNode::NodeScan {
                variable,
                labels,
                properties,
            } => {
                Ok(PhysicalNode::NodeSeqScan {
                    variable: variable.clone(),
                    labels: labels.clone(),
                    properties: properties.clone(),
                    estimated_rows: 1000,  // Default estimate
                    estimated_cost: 100.0, // Default cost
                })
            }
            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => {
                let input_physical = self.logical_to_physical_node(input)?;
                Ok(PhysicalNode::IndexedExpand {
                    from_variable: from_variable.clone(),
                    edge_variable: edge_variable.clone(),
                    to_variable: to_variable.clone(),
                    edge_labels: edge_labels.clone(),
                    direction: direction.clone(),
                    properties: properties.clone(),
                    input: Box::new(input_physical),
                    estimated_rows: 1000,
                    estimated_cost: 200.0,
                })
            }
            LogicalNode::SingleRow => {
                Ok(PhysicalNode::SingleRow {
                    estimated_rows: 1,   // Always exactly 1 row
                    estimated_cost: 1.0, // Minimal cost - cheapest possible operation
                })
            }
            _ => {
                // For other node types, return a simple scan as fallback
                Ok(PhysicalNode::NodeSeqScan {
                    variable: "fallback".to_string(),
                    labels: vec!["Node".to_string()],
                    properties: None,
                    estimated_rows: 1000,
                    estimated_cost: 100.0,
                })
            }
        }
    }

    /// Create an optimized logical plan that avoids Cartesian products
    fn create_optimized_logical_plan(
        &self,
        match_clause: &MatchClause,
    ) -> Result<LogicalPlan, PlanningError> {
        // This is a simplified implementation that creates connected joins
        // instead of Cartesian products when patterns share variables

        if match_clause.patterns.len() < 2 {
            return Err(PlanningError::InvalidQuery(
                "Need multiple patterns for optimization".to_string(),
            ));
        }

        // Start with the first pattern
        let first_pattern = &match_clause.patterns[0];
        let first_node =
            LogicalPlan::from_path_pattern(first_pattern).map_err(PlanningError::InvalidQuery)?;
        let mut current_plan = LogicalPlan::new(first_node);

        // 🔧 CRITICAL FIX: Extract variables from the first pattern (separate context)
        let mut first_context = PlanningContext {
            variables: HashMap::new(),
            _next_variable_id: 0,
        };
        self.extract_pattern_variables(first_pattern, &mut first_context)?;
        self.populate_plan_variables_from_context(&mut current_plan, &first_context);

        // Add subsequent patterns with intelligent joining
        for pattern in &match_clause.patterns[1..] {
            let pattern_node =
                LogicalPlan::from_path_pattern(pattern).map_err(PlanningError::InvalidQuery)?;
            let mut pattern_plan = LogicalPlan::new(pattern_node);

            // 🔧 CRITICAL FIX: Extract variables from this pattern (separate context)
            let mut pattern_context = PlanningContext {
                variables: HashMap::new(),
                _next_variable_id: 0,
            };
            self.extract_pattern_variables(pattern, &mut pattern_context)?;
            self.populate_plan_variables_from_context(&mut pattern_plan, &pattern_context);

            // 🔍 DEBUG: Let's see what variables are in each plan
            log::debug!(
                "🔍 Current plan variables: {:?}",
                current_plan.variables.keys().collect::<Vec<_>>()
            );
            log::debug!(
                "🔍 Pattern plan variables: {:?}",
                pattern_plan.variables.keys().collect::<Vec<_>>()
            );

            // Check if patterns share variables (connected patterns)
            let shared_vars = self.find_shared_variables(&current_plan, &pattern_plan);

            if !shared_vars.is_empty() {
                // 🔧 PATH TRAVERSAL APPROACH: Instead of join, create a single connected path
                log::debug!(
                    "🔗 Creating connected PATH TRAVERSAL for shared variables: {:?}",
                    shared_vars
                );
                log::debug!("🔍 This should convert comma-separated patterns into a single path");

                // Try to merge the patterns into a single path traversal
                // This is the natural approach for graph databases
                let merged_path_result =
                    self.merge_patterns_into_path(&current_plan, &pattern_plan, &shared_vars[0]);

                match merged_path_result {
                    Ok(merged_plan) => {
                        log::debug!("✅ Successfully merged patterns into connected path");
                        current_plan = merged_plan;
                    }
                    Err(e) => {
                        log::debug!(
                            "⚠️  Path merging failed ({}), falling back to inner join",
                            e
                        );
                        // Fall back to join approach
                        let join_condition =
                            self.create_join_condition_for_shared_vars(&shared_vars)?;

                        let join_node = LogicalNode::Join {
                            join_type: JoinType::Inner,
                            condition: join_condition,
                            left: Box::new(current_plan.root),
                            right: Box::new(pattern_plan.root),
                        };

                        // Merge variables from both plans
                        let mut merged_variables = current_plan.variables.clone();
                        for (name, info) in pattern_plan.variables {
                            merged_variables.insert(name, info);
                        }

                        current_plan = LogicalPlan {
                            root: join_node,
                            variables: merged_variables,
                        };
                    }
                }
            } else {
                // No shared variables - use cross product (original behavior)
                log::debug!("❌ No shared variables found, using cross product");

                let join_node = LogicalNode::Join {
                    join_type: JoinType::Cross,
                    condition: None,
                    left: Box::new(current_plan.root),
                    right: Box::new(pattern_plan.root),
                };

                let mut merged_variables = current_plan.variables.clone();
                for (name, info) in pattern_plan.variables {
                    merged_variables.insert(name, info);
                }

                current_plan = LogicalPlan {
                    root: join_node,
                    variables: merged_variables,
                };
            }
        }

        log::debug!(
            "🎯 OPTIMIZATION COMPLETE: Returning optimized plan with {} variables",
            current_plan.variables.len()
        );
        Ok(current_plan)
    }

    /// Find shared variables between two logical plans
    fn find_shared_variables(&self, plan1: &LogicalPlan, plan2: &LogicalPlan) -> Vec<String> {
        let mut shared = Vec::new();

        for var_name in plan1.variables.keys() {
            if plan2.variables.contains_key(var_name) {
                shared.push(var_name.clone());
            }
        }

        shared
    }

    /// Populate LogicalPlan variables from PlanningContext
    fn populate_plan_variables_from_context(
        &self,
        plan: &mut LogicalPlan,
        context: &PlanningContext,
    ) {
        for var_name in context.variables.keys() {
            // Create variable info based on variable name patterns
            let entity_type = if var_name.starts_with('r') {
                EntityType::Edge
            } else {
                EntityType::Node
            };

            let var_info = VariableInfo {
                name: var_name.clone(),
                entity_type,
                labels: vec![], // Would be populated from pattern analysis
                required_properties: vec![],
            };

            plan.add_variable(var_name.clone(), var_info);
        }
    }

    /// Merge two patterns connected by a shared variable into a connected chain of Expand nodes
    fn merge_patterns_into_path(
        &self,
        left_plan: &LogicalPlan,
        right_plan: &LogicalPlan,
        shared_var: &str,
    ) -> Result<LogicalPlan, String> {
        // The key insight: (a)-[r1]->(b), (b)-[r2]->(c) becomes a chain of Expand nodes
        log::debug!("🔍 Left plan root: {:?}", left_plan.root);
        log::debug!("🔍 Right plan root: {:?}", right_plan.root);

        // Instead of creating a PathTraversal, we chain the Expand nodes
        // This ensures proper execution through the existing Expand executor

        // Extract the right pattern's Expand node
        if let LogicalNode::Expand {
            from_variable,
            edge_variable,
            to_variable,
            edge_labels,
            direction,
            properties,
            ..
        } = &right_plan.root
        {
            // Verify the right pattern starts from the shared variable
            if from_variable != shared_var {
                return Err(format!(
                    "Right pattern doesn't start from shared variable {}",
                    shared_var
                ));
            }

            // Create a new Expand node that chains after the left pattern
            let chained_expand = LogicalNode::Expand {
                from_variable: from_variable.clone(),
                edge_variable: edge_variable.clone(),
                to_variable: to_variable.clone(),
                edge_labels: edge_labels.clone(),
                direction: direction.clone(),
                properties: properties.clone(),
                input: Box::new(left_plan.root.clone()), // Use left pattern as input
            };

            // Merge all variables
            let mut merged_variables = left_plan.variables.clone();
            for (name, info) in &right_plan.variables {
                merged_variables.insert(name.clone(), info.clone());
            }

            log::debug!(
                "✅ Created chained Expand nodes connecting patterns via: {}",
                shared_var
            );

            Ok(LogicalPlan {
                root: chained_expand,
                variables: merged_variables,
            })
        } else {
            // Fallback if pattern structure is unexpected
            Err("Right pattern is not an Expand node".to_string())
        }
    }

    /// Create join condition for shared variables
    fn create_join_condition_for_shared_vars(
        &self,
        shared_vars: &[String],
    ) -> Result<Option<Expression>, PlanningError> {
        if shared_vars.is_empty() {
            return Ok(None);
        }

        // 🔧 NATURAL JOIN APPROACH: Let the Inner join naturally join on shared variable names
        // Return None to let the physical execution engine perform a natural inner join
        // on variables with the same name
        let _var_name = &shared_vars[0];

        // Return None - let the Inner join type with shared variables handle the join logic
        Ok(None)
    }

    /// Extract variables from a path pattern
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

    /// Plan RETURN clause into projection expressions
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
    /// Plan GROUP BY clause with alias resolution from RETURN clause
    fn plan_group_clause_with_aliases(
        &self,
        group_clause: &crate::ast::GroupClause,
        return_clause: &crate::ast::ReturnClause,
        _context: &PlanningContext,
    ) -> Result<Vec<crate::ast::Expression>, PlanningError> {
        use crate::ast::{Expression, Variable};

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

    /// Resolve expressions in HAVING clauses with alias resolution from RETURN clause
    fn resolve_having_expression_with_aliases(
        &self,
        expr: &crate::ast::Expression,
        return_clause: &crate::ast::ReturnClause,
    ) -> crate::ast::Expression {
        use crate::ast::{Expression, FunctionCall, Variable};

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
                Expression::FunctionCall(FunctionCall {
                    name: func_call.name.clone(),
                    arguments: resolved_args,
                    distinct: func_call.distinct.clone(),
                    location: func_call.location.clone(),
                })
            }
            Expression::Binary(binary_expr) => Expression::Binary(crate::ast::BinaryExpression {
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

    /// Optimize logical plan
    fn optimize_logical_plan(&self, mut plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        match self.optimization_level {
            OptimizationLevel::None => Ok(plan),

            OptimizationLevel::Basic => {
                // Apply basic optimizations
                plan = self.apply_predicate_pushdown(plan)?;
                plan = self.apply_projection_elimination(plan)?;
                plan = self.apply_subquery_unnesting(plan)?; // Add basic subquery unnesting
                Ok(plan)
            }

            OptimizationLevel::Advanced | OptimizationLevel::Aggressive => {
                // Apply all basic optimizations
                plan = self.apply_predicate_pushdown(plan)?;
                plan = self.apply_projection_elimination(plan)?;

                // Apply advanced optimizations
                plan = self.apply_subquery_unnesting(plan)?; // Add subquery unnesting
                plan = self.apply_join_reordering(plan)?;

                Ok(plan)
            }
        }
    }

    /// Apply predicate pushdown optimization
    fn apply_predicate_pushdown(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Recursively optimize the logical plan tree
        let optimized_root = self.optimize_logical_node(plan.root)?;

        Ok(LogicalPlan {
            root: optimized_root,
            variables: plan.variables,
        })
    }

    /// Recursively optimize a logical node
    fn optimize_logical_node(&self, node: LogicalNode) -> Result<LogicalNode, PlanningError> {
        match node {
            LogicalNode::Union { inputs, all } => {
                // For UNION queries, we need to ensure each branch is properly optimized
                let mut optimized_inputs = Vec::new();
                for input in inputs {
                    optimized_inputs.push(self.optimize_logical_node(input)?);
                }

                Ok(LogicalNode::Union {
                    inputs: optimized_inputs,
                    all,
                })
            }

            LogicalNode::Filter { condition, input } => {
                // Recursively optimize the input first
                let optimized_input = self.optimize_logical_node(*input)?;

                // Try to push the filter down if possible
                match optimized_input {
                    LogicalNode::Union { inputs, all } => {
                        // Push the filter down to both sides of the UNION
                        let mut filtered_inputs = Vec::new();
                        for union_input in inputs {
                            filtered_inputs.push(LogicalNode::Filter {
                                condition: condition.clone(),
                                input: Box::new(union_input),
                            });
                        }

                        Ok(LogicalNode::Union {
                            inputs: filtered_inputs,
                            all,
                        })
                    }

                    LogicalNode::Join {
                        left,
                        right,
                        join_type,
                        condition: join_condition,
                    } => {
                        // For joins, we need to analyze which side the filter applies to
                        // For now, keep the filter above the join
                        Ok(LogicalNode::Filter {
                            condition,
                            input: Box::new(LogicalNode::Join {
                                left: Box::new(self.optimize_logical_node(*left)?),
                                right: Box::new(self.optimize_logical_node(*right)?),
                                join_type,
                                condition: join_condition,
                            }),
                        })
                    }

                    other => {
                        // For other node types, keep the filter as is but optimize the input
                        Ok(LogicalNode::Filter {
                            condition,
                            input: Box::new(other),
                        })
                    }
                }
            }

            LogicalNode::Join {
                left,
                right,
                join_type,
                condition,
            } => Ok(LogicalNode::Join {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                join_type,
                condition,
            }),

            LogicalNode::Project { expressions, input } => Ok(LogicalNode::Project {
                expressions,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Aggregate {
                group_by,
                aggregates,
                input,
            } => Ok(LogicalNode::Aggregate {
                group_by,
                aggregates,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Sort { expressions, input } => Ok(LogicalNode::Sort {
                expressions,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Limit {
                count,
                offset,
                input,
            } => Ok(LogicalNode::Limit {
                count,
                offset,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => Ok(LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input,
            } => Ok(LogicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Having { condition, input } => Ok(LogicalNode::Having {
                condition,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Distinct { input } => Ok(LogicalNode::Distinct {
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::GenericFunction {
                function_name,
                arguments,
                input,
            } => Ok(LogicalNode::GenericFunction {
                function_name,
                arguments,
                input: Box::new(self.optimize_logical_node(*input)?),
            }),

            LogicalNode::Intersect { left, right, all } => Ok(LogicalNode::Intersect {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                all,
            }),

            LogicalNode::Except { left, right, all } => Ok(LogicalNode::Except {
                left: Box::new(self.optimize_logical_node(*left)?),
                right: Box::new(self.optimize_logical_node(*right)?),
                all,
            }),

            // Leaf nodes and complex nodes that don't need recursion for now
            LogicalNode::NodeScan { .. }
            | LogicalNode::EdgeScan { .. }
            | LogicalNode::SingleRow
            | LogicalNode::Insert { .. }
            | LogicalNode::Delete { .. }
            | LogicalNode::Update { .. }
            | LogicalNode::ExistsSubquery { .. }
            | LogicalNode::NotExistsSubquery { .. }
            | LogicalNode::InSubquery { .. }
            | LogicalNode::NotInSubquery { .. }
            | LogicalNode::ScalarSubquery { .. }
            | LogicalNode::WithQuery { .. }
            | LogicalNode::Unwind { .. }
            | LogicalNode::TextSearch { .. } => Ok(node),
        }
    }

    /// Apply projection elimination optimization
    fn apply_projection_elimination(
        &self,
        plan: LogicalPlan,
    ) -> Result<LogicalPlan, PlanningError> {
        // TODO: Implement projection elimination
        // For now, return the plan unchanged
        Ok(plan)
    }

    /// Apply join reordering optimization
    fn apply_join_reordering(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // TODO: Implement join reordering based on cardinality estimates
        // For now, return the plan unchanged
        Ok(plan)
    }

    /// Apply subquery unnesting optimization
    fn apply_subquery_unnesting(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        let unnested_root = self.unnest_subqueries_in_node(plan.root)?;
        Ok(LogicalPlan::new(unnested_root))
    }

    /// Recursively unnest subqueries in a logical node
    fn unnest_subqueries_in_node(&self, node: LogicalNode) -> Result<LogicalNode, PlanningError> {
        match node {
            // EXISTS subquery can be converted to LEFT SEMI JOIN
            LogicalNode::ExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_exists_subquery(&subquery, &outer_variables) {
                    self.unnest_exists_subquery(*subquery, outer_variables)
                } else {
                    // Keep as subquery but unnest any nested subqueries
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::ExistsSubquery {
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // NOT EXISTS subquery can be converted to LEFT ANTI JOIN
            LogicalNode::NotExistsSubquery {
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_not_exists_subquery(&subquery, &outer_variables) {
                    self.unnest_not_exists_subquery(*subquery, outer_variables)
                } else {
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::NotExistsSubquery {
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // IN subquery can sometimes be converted to INNER JOIN
            LogicalNode::InSubquery {
                expression,
                subquery,
                outer_variables,
                ..
            } => {
                if self.can_unnest_in_subquery(&subquery, &outer_variables, &expression) {
                    self.unnest_in_subquery(*subquery, outer_variables, expression)
                } else {
                    let unnested_subquery = Box::new(self.unnest_subqueries_in_node(*subquery)?);
                    Ok(LogicalNode::InSubquery {
                        expression,
                        subquery: unnested_subquery,
                        outer_variables,
                    })
                }
            }

            // Recursively process nodes with inputs
            LogicalNode::Filter { condition, input } => {
                let unnested_input = Box::new(self.unnest_subqueries_in_node(*input)?);
                Ok(LogicalNode::Filter {
                    condition,
                    input: unnested_input,
                })
            }

            LogicalNode::Project { expressions, input } => {
                let unnested_input = Box::new(self.unnest_subqueries_in_node(*input)?);
                Ok(LogicalNode::Project {
                    expressions,
                    input: unnested_input,
                })
            }

            LogicalNode::Join {
                join_type,
                condition,
                left,
                right,
            } => {
                let unnested_left = Box::new(self.unnest_subqueries_in_node(*left)?);
                let unnested_right = Box::new(self.unnest_subqueries_in_node(*right)?);
                Ok(LogicalNode::Join {
                    join_type,
                    condition,
                    left: unnested_left,
                    right: unnested_right,
                })
            }

            // For all other nodes, return as-is (base case for recursion)
            _ => Ok(node),
        }
    }

    /// Check if EXISTS subquery can be unnested
    fn can_unnest_exists_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
    ) -> bool {
        // Basic unnesting is possible if:
        // 1. Subquery doesn't contain aggregation (would need HAVING)
        // 2. Subquery references outer variables (correlated)
        // 3. Subquery doesn't contain LIMIT/OFFSET

        !self.contains_aggregation(subquery)
            && !outer_variables.is_empty()
            && !self.contains_limit(subquery)
    }

    /// Check if NOT EXISTS subquery can be unnested
    fn can_unnest_not_exists_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
    ) -> bool {
        // Same conditions as EXISTS
        self.can_unnest_exists_subquery(subquery, outer_variables)
    }

    /// Check if IN subquery can be unnested
    fn can_unnest_in_subquery(
        &self,
        subquery: &LogicalNode,
        outer_variables: &[String],
        _expression: &Expression,
    ) -> bool {
        // IN subquery can be unnested if:
        // 1. No aggregation
        // 2. Correlated (references outer variables)
        // 3. No LIMIT/OFFSET
        // 4. Subquery returns unique values (to avoid duplicate results)

        !self.contains_aggregation(subquery)
            && !outer_variables.is_empty()
            && !self.contains_limit(subquery)
            && self.returns_unique_values(subquery)
    }

    /// Unnest EXISTS subquery to LEFT SEMI JOIN
    fn unnest_exists_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
    ) -> Result<LogicalNode, PlanningError> {
        // Convert to LEFT SEMI JOIN with correlation conditions
        let join_condition = self.build_correlation_condition(&outer_variables)?;

        // Create a placeholder for the outer query (would be provided by caller)
        // For now, create a simple node scan as the left side
        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::LeftSemi,
            condition: Some(join_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Unnest NOT EXISTS subquery to LEFT ANTI JOIN
    fn unnest_not_exists_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
    ) -> Result<LogicalNode, PlanningError> {
        let join_condition = self.build_correlation_condition(&outer_variables)?;

        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::LeftAnti,
            condition: Some(join_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Unnest IN subquery to INNER JOIN
    fn unnest_in_subquery(
        &self,
        subquery: LogicalNode,
        outer_variables: Vec<String>,
        expression: Expression,
    ) -> Result<LogicalNode, PlanningError> {
        // Build join condition combining correlation and IN expression
        let correlation_condition = self.build_correlation_condition(&outer_variables)?;
        let in_condition = self.build_in_join_condition(expression)?;

        // Combine conditions with AND
        let combined_condition = Expression::Binary(BinaryExpression {
            left: Box::new(correlation_condition),
            operator: Operator::And,
            right: Box::new(in_condition),
            location: crate::ast::Location::default(),
        });

        let outer_scan = LogicalNode::NodeScan {
            variable: "outer".to_string(),
            labels: vec![],
            properties: None,
        };

        Ok(LogicalNode::Join {
            join_type: JoinType::Inner,
            condition: Some(combined_condition),
            left: Box::new(outer_scan),
            right: Box::new(subquery),
        })
    }

    /// Build correlation condition from outer variables
    fn build_correlation_condition(
        &self,
        outer_variables: &[String],
    ) -> Result<Expression, PlanningError> {
        if outer_variables.is_empty() {
            return Err(PlanningError::InvalidQuery(
                "No correlation variables for join".to_string(),
            ));
        }

        // For simplicity, create an equality condition on the first outer variable
        // In practice, this would be more sophisticated
        let var_name = &outer_variables[0];
        Ok(Expression::Binary(BinaryExpression {
            left: Box::new(Expression::Variable(Variable {
                name: format!("outer.{}", var_name),
                location: crate::ast::Location::default(),
            })),
            operator: Operator::Equal,
            right: Box::new(Expression::Variable(Variable {
                name: format!("inner.{}", var_name),
                location: crate::ast::Location::default(),
            })),
            location: crate::ast::Location::default(),
        }))
    }

    /// Build join condition for IN expression
    fn build_in_join_condition(&self, expression: Expression) -> Result<Expression, PlanningError> {
        // Convert IN expression to equality for join
        // This is a simplified implementation
        Ok(expression)
    }

    /// Check if node contains aggregation
    fn contains_aggregation(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Aggregate { .. } => true,
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Distinct { input, .. }
            | LogicalNode::Limit { input, .. } => self.contains_aggregation(input),
            LogicalNode::Join { left, right, .. } => {
                self.contains_aggregation(left) || self.contains_aggregation(right)
            }
            _ => false,
        }
    }

    /// Check if node contains LIMIT
    fn contains_limit(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Limit { .. } => true,
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Distinct { input, .. } => self.contains_limit(input),
            LogicalNode::Join { left, right, .. } => {
                self.contains_limit(left) || self.contains_limit(right)
            }
            _ => false,
        }
    }

    /// Check if node returns unique values
    fn returns_unique_values(&self, node: &LogicalNode) -> bool {
        match node {
            LogicalNode::Distinct { .. } => true,
            LogicalNode::NodeScan { .. } => true, // Assume node scans return unique nodes
            LogicalNode::Filter { input, .. }
            | LogicalNode::Project { input, .. }
            | LogicalNode::Sort { input, .. }
            | LogicalNode::Limit { input, .. } => self.returns_unique_values(input),
            _ => false, // Conservative approach
        }
    }

    /// Check if projection expressions contain aggregate functions
    fn contains_aggregate_functions(&self, expressions: &[ProjectExpression]) -> bool {
        expressions
            .iter()
            .any(|expr| self.is_aggregate_expression(&expr.expression))
    }

    /// Check if an expression contains aggregate functions
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

    /// Extract non-aggregate expressions that should be included in GROUP BY
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

    /// Create physical plan from logical plan
    fn create_physical_plan(
        &self,
        logical_plan: LogicalPlan,
    ) -> Result<PhysicalPlan, PlanningError> {
        Ok(PhysicalPlan::from_logical(&logical_plan))
    }

    /// Optimize physical plan
    fn optimize_physical_plan(&self, plan: PhysicalPlan) -> Result<PhysicalPlan, PlanningError> {
        let mut optimized_plan = plan;

        // Apply index scan optimization based on setting
        if self.avoid_index_scan {
            optimized_plan = self.disable_index_scans(optimized_plan)?;
        }

        // TODO: Implement other physical optimizations like:
        // - Operator selection (hash vs nested loop join)
        // - Parallel execution planning
        Ok(optimized_plan)
    }

    /// Disable index scans in the physical plan
    pub fn disable_index_scans(&self, plan: PhysicalPlan) -> Result<PhysicalPlan, PlanningError> {
        let transformed_root = self.transform_node_disable_indexes(plan.root)?;
        Ok(PhysicalPlan::new(transformed_root))
    }

    /// Recursively transform physical nodes to disable index scans
    fn transform_node_disable_indexes(
        &self,
        node: PhysicalNode,
    ) -> Result<PhysicalNode, PlanningError> {
        match node {
            // Replace NodeIndexScan with NodeSeqScan
            PhysicalNode::NodeIndexScan {
                variable,
                labels,
                properties,
                estimated_rows,
                ..
            } => {
                // Sequential scan typically has higher cost than index scan
                let estimated_cost = estimated_rows as f64 * 0.1;
                Ok(PhysicalNode::NodeSeqScan {
                    variable,
                    labels,
                    properties,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Replace IndexedExpand with HashExpand (non-indexed expansion)
            PhysicalNode::IndexedExpand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
                estimated_rows,
                ..
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                let estimated_cost = estimated_rows as f64 * 0.3; // Higher cost without index
                Ok(PhysicalNode::HashExpand {
                    from_variable,
                    edge_variable,
                    to_variable,
                    edge_labels,
                    direction,
                    properties,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Recursively transform nodes with single input
            PhysicalNode::Filter {
                condition,
                input,
                selectivity,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Filter {
                    condition,
                    input: transformed_input,
                    selectivity,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Having {
                condition,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Having {
                    condition,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Project {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Project {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::HashAggregate {
                group_by,
                aggregates,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::HashAggregate {
                    group_by,
                    aggregates,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::SortAggregate {
                group_by,
                aggregates,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::SortAggregate {
                    group_by,
                    aggregates,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::ExternalSort {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::ExternalSort {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::InMemorySort {
                expressions,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::InMemorySort {
                    expressions,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Limit {
                count,
                offset,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Limit {
                    count,
                    offset,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Distinct {
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::Distinct {
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform nodes with two inputs (joins)
            PhysicalNode::HashJoin {
                join_type,
                condition,
                build_keys,
                probe_keys,
                build,
                probe,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_build = Box::new(self.transform_node_disable_indexes(*build)?);
                let transformed_probe = Box::new(self.transform_node_disable_indexes(*probe)?);
                Ok(PhysicalNode::HashJoin {
                    join_type,
                    condition,
                    build_keys,
                    probe_keys,
                    build: transformed_build,
                    probe: transformed_probe,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::NestedLoopJoin {
                join_type,
                condition,
                left,
                right,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::NestedLoopJoin {
                    join_type,
                    condition,
                    left: transformed_left,
                    right: transformed_right,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::SortMergeJoin {
                join_type,
                left_keys,
                right_keys,
                left,
                right,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::SortMergeJoin {
                    join_type,
                    left_keys,
                    right_keys,
                    left: transformed_left,
                    right: transformed_right,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform nodes with multiple inputs
            PhysicalNode::UnionAll {
                inputs,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let mut transformed_inputs = Vec::new();
                for input in inputs {
                    transformed_inputs.push(self.transform_node_disable_indexes(input)?);
                }
                Ok(PhysicalNode::UnionAll {
                    inputs: transformed_inputs,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Transform PathTraversal input
            PhysicalNode::PathTraversal {
                path_type,
                from_variable,
                to_variable,
                path_elements,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_input = Box::new(self.transform_node_disable_indexes(*input)?);
                Ok(PhysicalNode::PathTraversal {
                    path_type,
                    from_variable,
                    to_variable,
                    path_elements,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            // Nodes that are already non-indexed (no transformation needed)
            PhysicalNode::NodeSeqScan { .. }
            | PhysicalNode::EdgeSeqScan { .. }
            | PhysicalNode::HashExpand { .. }
            | PhysicalNode::GenericFunction { .. }
            | PhysicalNode::SingleRow { .. } => {
                Ok(node) // Already using appropriate scans / no transformation needed
            }

            // Handle subqueries recursively
            PhysicalNode::ExistsSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
                optimized,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::ExistsSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized,
                })
            }

            PhysicalNode::NotExistsSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
                optimized,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::NotExistsSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                    optimized,
                })
            }

            PhysicalNode::InSubquery {
                expression,
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::InSubquery {
                    expression,
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::NotInSubquery {
                expression,
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::NotInSubquery {
                    expression,
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::ScalarSubquery {
                subplan,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_subplan = Box::new(self.transform_node_disable_indexes(*subplan)?);
                Ok(PhysicalNode::ScalarSubquery {
                    subplan: transformed_subplan,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::WithQuery {
                original_query,
                estimated_rows,
                estimated_cost,
            } => {
                // WITH queries don't use index scans in their current implementation
                // Just return them as-is
                Ok(PhysicalNode::WithQuery {
                    original_query,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Unwind {
                expression,
                variable,
                input,
                estimated_rows,
                estimated_cost,
            } => {
                // UNWIND doesn't use index scans
                // Transform the optional input if present
                let transformed_input = if let Some(input_node) = input {
                    Some(Box::new(self.transform_node_disable_indexes(*input_node)?))
                } else {
                    None
                };

                Ok(PhysicalNode::Unwind {
                    expression,
                    variable,
                    input: transformed_input,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::GraphIndexScan {
                parameters,
                estimated_rows,
                estimated_cost,
                ..
            } => {
                // Convert graph index scan to sequential traversal
                Ok(PhysicalNode::NodeSeqScan {
                    variable: "graph_fallback".to_string(),
                    labels: vec!["*".to_string()],
                    properties: Some(parameters),
                    estimated_rows,
                    estimated_cost: estimated_cost * 15.0, // Higher cost without graph index
                })
            }

            PhysicalNode::TextIndexScan {
                field,
                query,
                estimated_rows,
                estimated_cost,
                ..
            } => {
                // Fallback for text index: use a generic sequential scan with parameters
                let mut properties: std::collections::HashMap<String, crate::ast::Expression> =
                    std::collections::HashMap::new();
                properties.insert(
                    "field".to_string(),
                    crate::ast::Expression::Literal(crate::ast::Literal::String(field.clone())),
                );
                properties.insert(
                    "query".to_string(),
                    crate::ast::Expression::Literal(crate::ast::Literal::String(query.clone())),
                );

                Ok(PhysicalNode::NodeSeqScan {
                    variable: "text_fallback".to_string(),
                    labels: vec!["*".to_string()],
                    properties: Some(properties),
                    estimated_rows: estimated_rows,
                    estimated_cost: estimated_cost * 15.0,
                })
            }

            PhysicalNode::IndexJoin {
                left,
                right,
                join_type,
                join_condition,
                estimated_rows,
                estimated_cost,
                ..
            } => {
                // Convert index join to nested loop join
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);

                Ok(PhysicalNode::NestedLoopJoin {
                    join_type,
                    condition: Some(join_condition),
                    left: transformed_left,
                    right: transformed_right,
                    estimated_rows,
                    estimated_cost: estimated_cost * 5.0, // Higher cost without index
                })
            }

            // Data modification operations - pass through unchanged
            PhysicalNode::Insert { .. }
            | PhysicalNode::Update { .. }
            | PhysicalNode::Delete { .. } => Ok(node),

            // Set operations - recursively transform inputs (only Intersect and Except, UnionAll handled above)
            PhysicalNode::Intersect {
                left,
                right,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::Intersect {
                    left: transformed_left,
                    right: transformed_right,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }

            PhysicalNode::Except {
                left,
                right,
                all,
                estimated_rows,
                estimated_cost,
            } => {
                let transformed_left = Box::new(self.transform_node_disable_indexes(*left)?);
                let transformed_right = Box::new(self.transform_node_disable_indexes(*right)?);
                Ok(PhysicalNode::Except {
                    left: transformed_left,
                    right: transformed_right,
                    all,
                    estimated_rows,
                    estimated_cost,
                })
            }
        }
    }

    /// Generate alternative join orders
    #[allow(dead_code)] // ROADMAP v0.3.0 - Join reordering for cost-based optimization (see ROADMAP.md §5)
    fn generate_join_alternatives(
        &self,
        logical_plan: &LogicalPlan,
    ) -> Result<PhysicalPlan, PlanningError> {
        // TODO: Implement join reordering alternatives
        // For now, return basic physical plan
        Ok(PhysicalPlan::from_logical(logical_plan))
    }

    /// Select best plan from alternatives based on cost
    #[allow(dead_code)] // ROADMAP v0.3.0 - Cost-based plan selection from alternatives (see ROADMAP.md §5)
    fn select_best_plan(&self, plans: &[PhysicalPlan]) -> Result<PhysicalPlan, PlanningError> {
        if plans.is_empty() {
            return Err(PlanningError::InvalidQuery(
                "No plans generated".to_string(),
            ));
        }

        // Find plan with lowest cost
        let best = plans
            .iter()
            .min_by(|a, b| {
                a.get_estimated_cost()
                    .partial_cmp(&b.get_estimated_cost())
                    .unwrap()
            })
            .unwrap();

        Ok(best.clone())
    }

    /// Estimate cost for a physical plan
    pub fn estimate_plan_cost(&self, plan: &PhysicalPlan) -> CostEstimate {
        self.cost_model
            .estimate_node_cost(&plan.root, &self.statistics)
    }

    /// Get planning statistics
    #[allow(dead_code)] // ROADMAP v0.3.0 - Statistics accessor for cost model (see ROADMAP.md §5)
    pub fn get_statistics(&self) -> &Statistics {
        &self.statistics
    }

    /// Get cost model
    #[allow(dead_code)] // ROADMAP v0.3.0 - Cost model accessor for plan estimation (see ROADMAP.md §5)
    pub fn get_cost_model(&self) -> &CostModel {
        &self.cost_model
    }

    /// Create logical plan for UNWIND statement
    fn create_unwind_logical_plan(
        &self,
        unwind_stmt: &crate::ast::UnwindStatement,
    ) -> Result<LogicalPlan, PlanningError> {
        use crate::plan::logical::LogicalNode;

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
            crate::plan::logical::VariableInfo {
                name: unwind_stmt.variable.clone(),
                entity_type: crate::plan::logical::EntityType::Node, // Treat unwound values as nodes for now
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
}

/// Information about available indexes for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub index_type: IndexType,
    pub table: String,
    pub columns: Vec<String>,
    pub properties: HashMap<String, String>,
    pub statistics: Option<IndexStatistics>,
}

/// Type of index for optimization decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    /// Text search index (inverted, n-gram, etc.)
    Text {
        analyzer: String,
        features: Vec<String>,
    },
    /// Graph structure index (adjacency, paths, etc.)
    Graph {
        operation: String,
        max_depth: Option<usize>,
    },
    /// Traditional B-tree or hash index
    Standard { unique: bool },
}

/// Statistics for an index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub selectivity: f64,
    pub avg_access_time_ms: f64,
}

/// Index-aware optimizer for transforming logical plans
struct IndexAwareOptimizer<'a> {
    _available_indexes: &'a [IndexInfo],
    _cost_model: &'a CostModel,
}

impl<'a> IndexAwareOptimizer<'a> {
    fn new(available_indexes: &'a [IndexInfo], cost_model: &'a CostModel) -> Self {
        Self {
            _available_indexes: available_indexes,
            _cost_model: cost_model,
        }
    }

    /// Apply index-aware optimization rules to logical plan
    fn apply_index_rules(&mut self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        let mut optimized = plan;

        // Convert text matches to full-text search
        optimized = self.transform_text_search(optimized)?;

        // Convert graph patterns to graph index operations
        optimized = self.transform_graph_operations(optimized)?;

        // Convert selective filters to index scans
        optimized = self.transform_selective_filters(optimized)?;

        Ok(optimized)
    }

    /// Transform text search patterns to full-text search
    fn transform_text_search(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Transform the root node to detect and convert text search patterns
        let optimized_root = self.transform_text_search_node(plan.root)?;

        Ok(LogicalPlan {
            root: optimized_root,
            variables: plan.variables,
        })
    }

    /// Recursively transform nodes to detect text search patterns
    fn transform_text_search_node(&self, node: LogicalNode) -> Result<LogicalNode, PlanningError> {
        match node {
            // Transform Filter nodes that contain text search predicates
            LogicalNode::Filter { condition, input } => {
                // First, recursively transform the input
                let transformed_input = Box::new(self.transform_text_search_node(*input)?);

                // Check if this filter contains a text-search predicate we can push down
                if let Some((variable, query, field, min_score)) =
                    self.extract_text_search_predicate(&condition)
                {
                    // Infer search type from condition shape
                    let search_type = match &condition {
                        Expression::FunctionCall(func)
                            if func.name.eq_ignore_ascii_case("text_search") =>
                        {
                            TextSearchType::BM25
                        }
                        Expression::FunctionCall(func)
                            if func.name.eq_ignore_ascii_case("fuzzy_match") =>
                        {
                            TextSearchType::NGram
                        }
                        Expression::Binary(binary)
                            if matches!(binary.operator, Operator::Matches) =>
                        {
                            TextSearchType::Boolean
                        }
                        Expression::Binary(binary)
                            if matches!(binary.operator, Operator::FuzzyEqual) =>
                        {
                            TextSearchType::NGram
                        }
                        _ => TextSearchType::BM25,
                    };

                    return Ok(LogicalNode::TextSearch {
                        variable,
                        field,
                        query,
                        search_type,
                        min_score: if min_score > 0.0 {
                            Some(min_score)
                        } else {
                            None
                        },
                        limit: None,
                        input: transformed_input,
                    });
                }

                // Otherwise keep as a regular Filter
                Ok(LogicalNode::Filter {
                    condition,
                    input: transformed_input,
                })
            }

            // Recursively transform other node types
            LogicalNode::Project { expressions, input } => Ok(LogicalNode::Project {
                expressions,
                input: Box::new(self.transform_text_search_node(*input)?),
            }),

            LogicalNode::Join {
                left,
                right,
                join_type,
                condition,
            } => Ok(LogicalNode::Join {
                left: Box::new(self.transform_text_search_node(*left)?),
                right: Box::new(self.transform_text_search_node(*right)?),
                join_type,
                condition,
            }),

            LogicalNode::Union { inputs, all } => {
                let transformed_inputs: Result<Vec<_>, _> = inputs
                    .into_iter()
                    .map(|input| self.transform_text_search_node(input))
                    .collect();
                Ok(LogicalNode::Union {
                    inputs: transformed_inputs?,
                    all,
                })
            }

            LogicalNode::Aggregate {
                group_by,
                aggregates,
                input,
            } => Ok(LogicalNode::Aggregate {
                group_by,
                aggregates,
                input: Box::new(self.transform_text_search_node(*input)?),
            }),

            LogicalNode::Sort { expressions, input } => Ok(LogicalNode::Sort {
                expressions,
                input: Box::new(self.transform_text_search_node(*input)?),
            }),

            LogicalNode::Limit {
                count,
                offset,
                input,
            } => Ok(LogicalNode::Limit {
                count,
                offset,
                input: Box::new(self.transform_text_search_node(*input)?),
            }),

            LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input,
            } => Ok(LogicalNode::Expand {
                from_variable,
                edge_variable,
                to_variable,
                edge_labels,
                direction,
                properties,
                input: Box::new(self.transform_text_search_node(*input)?),
            }),

            // Leaf nodes and other nodes that don't need transformation
            other => Ok(other),
        }
    }

    /// Extract text search predicate from expression
    /// Returns (variable, query, field, min_score) if this is a text search predicate
    #[allow(dead_code)] // ROADMAP v0.6.0 - Full-text search index optimization
    fn extract_text_search_predicate(
        &self,
        expr: &Expression,
    ) -> Option<(String, String, String, f64)> {
        match expr {
            // Match: TEXT_SEARCH(doc.content, 'query') - standalone boolean predicate (Phase 4)
            Expression::FunctionCall(func)
                if func.name.eq_ignore_ascii_case("text_search") && func.arguments.len() >= 2 =>
            {
                // Extract field from first argument (property access)
                if let (Some((variable, field)), Some(query)) = (
                    self.extract_property_access(&func.arguments[0]),
                    self.extract_string_literal(&func.arguments[1]),
                ) {
                    // Check for optional min_score as 3rd argument
                    let min_score = if func.arguments.len() >= 3 {
                        self.extract_number_literal(&func.arguments[2])
                            .unwrap_or(0.0)
                    } else {
                        0.0 // No minimum score filter
                    };
                    return Some((variable, query, field, min_score));
                }
            }

            // Match: text_search(doc.content, 'query') > 5.0 - with explicit score threshold
            Expression::Binary(binary)
                if matches!(
                    binary.operator,
                    Operator::GreaterThan | Operator::GreaterEqual
                ) =>
            {
                // Check if left side is text_search() function call
                if let Expression::FunctionCall(func) = &*binary.left {
                    if func.name.eq_ignore_ascii_case("text_search") && func.arguments.len() >= 2 {
                        // Extract field from first argument (property access)
                        if let (Some((variable, field)), Some(query), Some(min_score)) = (
                            self.extract_property_access(&func.arguments[0]),
                            self.extract_string_literal(&func.arguments[1]),
                            self.extract_number_literal(&binary.right),
                        ) {
                            return Some((variable, query, field, min_score));
                        }
                    }
                }
            }

            // Match: fuzzy_match(person.name, 'query', 2) > 0.7
            Expression::Binary(binary)
                if matches!(
                    binary.operator,
                    Operator::GreaterThan | Operator::GreaterEqual
                ) =>
            {
                if let Expression::FunctionCall(func) = &*binary.left {
                    if func.name.eq_ignore_ascii_case("fuzzy_match") && func.arguments.len() >= 2 {
                        if let (Some((variable, field)), Some(query), Some(min_score)) = (
                            self.extract_property_access(&func.arguments[0]),
                            self.extract_string_literal(&func.arguments[1]),
                            self.extract_number_literal(&binary.right),
                        ) {
                            return Some((variable, query, field, min_score));
                        }
                    }
                }
            }

            // Match: doc.content MATCHES 'query'
            Expression::Binary(binary) if matches!(binary.operator, Operator::Matches) => {
                if let (Some((variable, field)), Some(query)) = (
                    self.extract_property_access(&binary.left),
                    self.extract_string_literal(&binary.right),
                ) {
                    return Some((variable, query, field, 0.0)); // No min_score for MATCHES
                }
            }

            // Match: doc.content ~= 'query' (fuzzy match operator)
            Expression::Binary(binary) if matches!(binary.operator, Operator::FuzzyEqual) => {
                if let (Some((variable, field)), Some(query)) = (
                    self.extract_property_access(&binary.left),
                    self.extract_string_literal(&binary.right),
                ) {
                    return Some((variable, query, field, 0.0)); // No min_score for fuzzy operator
                }
            }

            _ => {}
        }

        None
    }

    /// Extract property access from expression (e.g., doc.content -> ("doc", "content"))
    #[allow(dead_code)] // ROADMAP v0.6.0 - Property access analysis for index selection
    fn extract_property_access(&self, expr: &Expression) -> Option<(String, String)> {
        if let Expression::PropertyAccess(prop_access) = expr {
            return Some((prop_access.object.clone(), prop_access.property.clone()));
        }
        None
    }

    /// Extract string literal from expression
    #[allow(dead_code)] // ROADMAP v0.6.0 - Literal extraction for predicate analysis
    fn extract_string_literal(&self, expr: &Expression) -> Option<String> {
        use crate::ast::{Expression, Literal};

        if let Expression::Literal(Literal::String(s)) = expr {
            return Some(s.clone());
        }
        None
    }

    /// Extract number literal from expression
    #[allow(dead_code)] // ROADMAP v0.6.0 - Numeric literal extraction for cost estimation
    fn extract_number_literal(&self, expr: &Expression) -> Option<f64> {
        use crate::ast::{Expression, Literal};

        match expr {
            Expression::Literal(Literal::Float(f)) => Some(*f),
            Expression::Literal(Literal::Integer(i)) => Some(*i as f64),
            _ => None,
        }
    }

    /// Transform graph traversal patterns to use graph indexes
    fn transform_graph_operations(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Look for path patterns, neighbor queries, etc.
        // Convert to GraphIndexScan operations
        Ok(plan)
    }

    /// Transform selective filters to index scans
    fn transform_selective_filters(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Analyze filter predicates and check if indexes can help
        // Replace sequential scans with index scans where beneficial
        Ok(plan)
    }

    /// Select best plan among alternatives using cost-based optimization
    #[allow(dead_code)] // ROADMAP v0.5.0 - Cost-based plan selection from multiple alternatives
    fn select_best_plan(
        &self,
        plan: PhysicalPlan,
        _statistics: &Statistics,
    ) -> Result<PhysicalPlan, PlanningError> {
        // For now, just return the plan
        // In practice, would generate multiple alternatives and pick lowest cost
        Ok(plan)
    }

    /// Check if an index is applicable for a given predicate
    #[allow(dead_code)] // ROADMAP v0.5.0 - Intelligent index selection for query predicates
    fn find_applicable_index(&self, _predicate: &Expression) -> Option<&IndexInfo> {
        // Analyze predicate and match against available indexes
        // This is where the intelligence of index selection happens
        None
    }
}

impl QueryPlanner {
    /// Create physical plan with index operations awareness
    #[allow(dead_code)] // ROADMAP v0.5.0 - Index-aware physical plan generation
    fn create_physical_plan_with_indexes(
        &self,
        logical_plan: LogicalPlan,
        _available_indexes: &[IndexInfo],
    ) -> Result<PhysicalPlan, PlanningError> {
        // For now, use the regular physical plan creation
        // In a full implementation, this would generate index-specific physical operators
        self.create_physical_plan(logical_plan)
    }
}

impl Default for QueryPlanner {
    fn default() -> Self {
        Self::new()
    }
}
