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
    Document, Expression, Operator, Query,
};
use crate::plan::builders::{LogicalBuilder, PhysicalBuilder};
use crate::plan::cost::{CostEstimate, CostModel, Statistics};
use crate::plan::logical::{
    LogicalNode, LogicalPlan, VariableInfo,
};
use crate::plan::optimizers::{LogicalOptimizer, PhysicalOptimizer};
use crate::plan::pattern_optimization::integration::PatternOptimizationPipeline;
use crate::plan::physical::PhysicalPlan;
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
    /// Logical plan builder
    logical_builder: LogicalBuilder,
    /// Physical plan builder
    physical_builder: PhysicalBuilder,
    /// Logical plan optimizer
    logical_optimizer: LogicalOptimizer,
    /// Physical plan optimizer
    physical_optimizer: PhysicalOptimizer,
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
        let optimization_level = OptimizationLevel::Basic;
        let avoid_index_scan = true;

        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: optimization_level.clone(),
            avoid_index_scan,
            pattern_optimizer: PatternOptimizationPipeline::new(),
            logical_builder: LogicalBuilder::new(),
            physical_builder: PhysicalBuilder::new(),
            logical_optimizer: LogicalOptimizer::new(optimization_level),
            physical_optimizer: PhysicalOptimizer::new(avoid_index_scan),
        }
    }

    /// Create a query planner with specific optimization level
    #[allow(dead_code)] // ROADMAP v0.5.0 - Multi-level optimization strategies (None, Basic, Aggressive)
    pub fn with_optimization_level(level: OptimizationLevel) -> Self {
        let avoid_index_scan = true;

        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: level.clone(),
            avoid_index_scan,
            pattern_optimizer: PatternOptimizationPipeline::new(),
            logical_builder: LogicalBuilder::new(),
            physical_builder: PhysicalBuilder::new(),
            logical_optimizer: LogicalOptimizer::new(level),
            physical_optimizer: PhysicalOptimizer::new(avoid_index_scan),
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
        let optimization_level = OptimizationLevel::Basic;

        Self {
            cost_model: CostModel::new(),
            statistics: Statistics::new(),
            optimization_level: optimization_level.clone(),
            avoid_index_scan,
            pattern_optimizer: PatternOptimizationPipeline::new(),
            logical_builder: LogicalBuilder::new(),
            physical_builder: PhysicalBuilder::new(),
            logical_optimizer: LogicalOptimizer::new(optimization_level),
            physical_optimizer: PhysicalOptimizer::new(avoid_index_scan),
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
        // Delegate to LogicalBuilder
        self.logical_builder.build(query)
    }



    /// Optimize logical plan
    fn optimize_logical_plan(&self, plan: LogicalPlan) -> Result<LogicalPlan, PlanningError> {
        // Delegate to LogicalOptimizer
        self.logical_optimizer.optimize(plan)
    }


    /// Create physical plan from logical plan
    fn create_physical_plan(
        &self,
        logical_plan: LogicalPlan,
    ) -> Result<PhysicalPlan, PlanningError> {
        // Delegate to PhysicalBuilder
        self.physical_builder.build(logical_plan)
    }

    /// Optimize physical plan
    fn optimize_physical_plan(&self, plan: PhysicalPlan) -> Result<PhysicalPlan, PlanningError> {
        // Delegate to PhysicalOptimizer
        self.physical_optimizer.optimize(plan)
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

                // Text search is not supported in GraphLite, keep as Filter
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
