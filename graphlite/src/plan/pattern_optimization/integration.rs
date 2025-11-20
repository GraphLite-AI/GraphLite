// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! End-to-End Integration for Pattern Optimization
//!
//! This module provides the complete integration of pattern optimization
//! into the query planning pipeline.

use crate::ast::ast::{MatchClause, Query};
use crate::plan::logical::LogicalPlan;
use crate::plan::pattern_optimization::{
    cost_estimation::StatisticsManager,
    logical_integration::{
        LogicalPatternOptimizer, OptimizationContext, PatternOptimizationResult,
    },
    physical_generation::{OptimizationImprovement, PhysicalGenerationConfig},
};
use crate::plan::physical::PhysicalNode;

/// Complete pattern optimization pipeline
#[derive(Debug)]
pub struct PatternOptimizationPipeline {
    /// Logical pattern optimizer
    logical_optimizer: LogicalPatternOptimizer,
    /// Statistics manager for cost estimation (planned feature - ROADMAP.md v0.3.0)
    #[allow(dead_code)]
    statistics_manager: StatisticsManager,
    /// Integration configuration
    config: IntegrationConfig,
    /// Optimization metrics
    metrics: OptimizationMetrics,
}

/// Configuration for the optimization pipeline
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    /// Enable pattern optimization globally
    pub enable_optimization: bool,
    /// Enable detailed optimization logging
    pub enable_logging: bool,
    /// Fallback to original behavior on optimization failure
    pub fallback_on_error: bool,
    /// Minimum improvement threshold to apply optimization
    pub min_improvement_threshold: f64,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self {
            enable_optimization: true,
            enable_logging: false,
            fallback_on_error: true,
            min_improvement_threshold: 10.0, // 10% improvement required
        }
    }
}

/// Metrics for optimization performance
#[derive(Debug, Clone, Default)]
pub struct OptimizationMetrics {
    /// Total queries processed
    pub queries_processed: u64,
    /// Queries that were optimized
    pub queries_optimized: u64,
    /// Total optimization time in milliseconds
    pub total_optimization_time_ms: u64,
    /// Average optimization time per query
    pub avg_optimization_time_ms: f64,
    /// Optimization success rate
    pub success_rate: f64,
    /// Total cost improvement percentage
    pub total_cost_improvement: f64,
    /// Total cardinality reduction percentage
    pub total_cardinality_reduction: f64,
}

impl OptimizationMetrics {
    /// Record a successful optimization
    pub fn record_success(
        &mut self,
        optimization_time_ms: u64,
        improvement: &OptimizationImprovement,
    ) {
        self.queries_processed += 1;
        self.queries_optimized += 1;
        self.total_optimization_time_ms += optimization_time_ms;
        self.avg_optimization_time_ms =
            self.total_optimization_time_ms as f64 / self.queries_processed as f64;
        self.success_rate = (self.queries_optimized as f64) / (self.queries_processed as f64);
        self.total_cost_improvement += improvement.cost_reduction_percentage;
        self.total_cardinality_reduction += improvement.cardinality_reduction_percentage;
    }

    /// Record a failed or skipped optimization
    pub fn record_skip(&mut self, optimization_time_ms: u64) {
        self.queries_processed += 1;
        self.total_optimization_time_ms += optimization_time_ms;
        self.avg_optimization_time_ms =
            self.total_optimization_time_ms as f64 / self.queries_processed as f64;
        self.success_rate = (self.queries_optimized as f64) / (self.queries_processed as f64);
    }

    /// Get average cost improvement per optimized query
    pub fn avg_cost_improvement(&self) -> f64 {
        if self.queries_optimized > 0 {
            self.total_cost_improvement / self.queries_optimized as f64
        } else {
            0.0
        }
    }

    /// Get average cardinality reduction per optimized query
    pub fn avg_cardinality_reduction(&self) -> f64 {
        if self.queries_optimized > 0 {
            self.total_cardinality_reduction / self.queries_optimized as f64
        } else {
            0.0
        }
    }
}

impl PatternOptimizationPipeline {
    /// Create a new optimization pipeline
    pub fn new() -> Self {
        Self {
            logical_optimizer: LogicalPatternOptimizer::new(),
            statistics_manager: StatisticsManager::new(),
            config: IntegrationConfig::default(),
            metrics: OptimizationMetrics::default(),
        }
    }

    /// Create pipeline with custom configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Custom configuration for optimization pipeline
    pub fn with_config(config: IntegrationConfig) -> Self {
        Self {
            logical_optimizer: LogicalPatternOptimizer::new(),
            statistics_manager: StatisticsManager::new(),
            config,
            metrics: OptimizationMetrics::default(),
        }
    }

    /// Main entry point: Optimize a complete query with MATCH clauses
    pub fn optimize_query(
        &mut self,
        query: &Query,
        _original_logical_plan: &LogicalPlan,
        original_physical_plan: &PhysicalNode,
    ) -> Result<OptimizationPipelineResult, String> {
        if !self.config.enable_optimization {
            return Ok(OptimizationPipelineResult::no_optimization(
                original_physical_plan.clone(),
                "Pattern optimization is disabled".to_string(),
            ));
        }

        let start_time = std::time::Instant::now();

        // Extract MATCH clauses from the query
        let match_clauses = self.extract_match_clauses(query)?;

        if match_clauses.is_empty() {
            let elapsed = start_time.elapsed().as_millis() as u64;
            self.metrics.record_skip(elapsed);
            return Ok(OptimizationPipelineResult::no_optimization(
                original_physical_plan.clone(),
                "No MATCH clauses found in query".to_string(),
            ));
        }

        // Check if any MATCH clause has comma-separated patterns (our target bug)
        let needs_optimization = match_clauses.iter().any(|clause| clause.patterns.len() > 1);

        if !needs_optimization {
            let elapsed = start_time.elapsed().as_millis() as u64;
            self.metrics.record_skip(elapsed);
            return Ok(OptimizationPipelineResult::no_optimization(
                original_physical_plan.clone(),
                "No comma-separated patterns found".to_string(),
            ));
        }

        // Create optimization context
        let context = self.create_optimization_context(query)?;

        // Apply optimization to each MATCH clause
        let mut optimized_plan = original_physical_plan.clone();
        let mut total_improvement = OptimizationImprovement {
            cost_reduction_percentage: 0.0,
            cardinality_reduction_percentage: 0.0,
            original_cost: 0.0,
            optimized_cost: 0.0,
            original_rows: 0,
            optimized_rows: 0,
        };

        let mut optimization_applied = false;
        let mut optimization_reasons = Vec::new();

        for match_clause in &match_clauses {
            if match_clause.patterns.len() > 1 {
                // This is where we fix the comma-separated pattern bug!
                match self.optimize_single_match_clause(match_clause, &optimized_plan, &context) {
                    Ok(result) => {
                        if result.optimization_result.optimized {
                            optimized_plan = result.optimized_physical_plan;
                            total_improvement =
                                self.combine_improvements(&total_improvement, &result.improvement);
                            optimization_applied = true;
                            optimization_reasons
                                .push(result.optimization_result.optimization_reason.clone());

                            if self.config.enable_logging {
                                log::debug!(
                                    "✅ Pattern optimization applied: {}",
                                    result.optimization_result.optimization_reason
                                );
                                log::debug!("   Improvement: {}", result.improvement.describe());
                            }
                        } else {
                            optimization_reasons
                                .push(result.optimization_result.optimization_reason.clone());
                        }
                    }
                    Err(e) => {
                        if self.config.fallback_on_error {
                            optimization_reasons
                                .push(format!("Optimization failed, using fallback: {}", e));
                        } else {
                            return Err(format!("Pattern optimization failed: {}", e));
                        }
                    }
                }
            }
        }

        let elapsed = start_time.elapsed().as_millis() as u64;

        if optimization_applied
            && total_improvement.is_significant_improvement(self.config.min_improvement_threshold)
        {
            // Record successful optimization
            self.metrics.record_success(elapsed, &total_improvement);

            Ok(OptimizationPipelineResult::optimized(
                optimized_plan,
                total_improvement,
                optimization_reasons.join("; "),
            ))
        } else {
            // Record skipped optimization
            self.metrics.record_skip(elapsed);

            Ok(OptimizationPipelineResult::no_optimization(
                original_physical_plan.clone(),
                format!(
                    "Optimization not beneficial: {}",
                    optimization_reasons.join("; ")
                ),
            ))
        }
    }

    /// Optimize a single MATCH clause with comma-separated patterns
    fn optimize_single_match_clause(
        &mut self,
        match_clause: &MatchClause,
        base_physical_plan: &PhysicalNode,
        context: &OptimizationContext,
    ) -> Result<SingleMatchOptimizationResult, String> {
        // Step 1: Apply logical optimization
        let logical_result =
            crate::plan::pattern_optimization::logical_integration::optimize_match_clause_patterns(
                &mut self.logical_optimizer,
                match_clause,
                context,
            )?;

        // Step 2: Generate optimized physical plan
        let optimized_physical_plan = crate::plan::pattern_optimization::physical_generation::generate_optimized_physical_plan(
            &logical_result,
            base_physical_plan.clone(),
            Some(PhysicalGenerationConfig::default()),
        )?;

        // Step 3: Calculate improvement
        let improvement = crate::plan::pattern_optimization::physical_generation::estimate_optimization_improvement(
            base_physical_plan,
            &optimized_physical_plan,
        );

        Ok(SingleMatchOptimizationResult {
            optimization_result: logical_result,
            optimized_physical_plan,
            improvement,
        })
    }

    /// Extract MATCH clauses from a query
    fn extract_match_clauses(&self, query: &Query) -> Result<Vec<MatchClause>, String> {
        // Extract MATCH clauses from the query structure
        let mut match_clauses = Vec::new();

        match query {
            Query::Basic(basic_query) => {
                // Basic query has exactly one MATCH clause
                match_clauses.push(basic_query.match_clause.clone());
            }
            Query::SetOperation(set_op) => {
                // Set operations combine results from left and right queries
                // Extract MATCH clauses from both sides
                let left_clauses = self.extract_match_clauses(&set_op.left)?;
                let right_clauses = self.extract_match_clauses(&set_op.right)?;
                match_clauses.extend(left_clauses);
                match_clauses.extend(right_clauses);
            }
            Query::Limited {
                query: inner_query, ..
            } => {
                // Limited query wraps another query
                let inner_clauses = self.extract_match_clauses(inner_query)?;
                match_clauses.extend(inner_clauses);
            }
            Query::WithQuery(_) => {
                // WITH queries don't directly contain MATCH clauses at the top level
                // They use segments that might contain BasicQuery elements
                // For now, we'll skip optimization for these complex queries
            }
            Query::MutationPipeline(_) => {
                // Mutation pipelines can contain MATCH clauses in their segments
                // For now, we'll skip optimization for these complex queries
            }
            Query::Let(_)
            | Query::For(_)
            | Query::Filter(_)
            | Query::Return(_)
            | Query::Unwind(_) => {
                // These query types don't contain MATCH clauses
                // Return empty list
            }
        }

        Ok(match_clauses)
    }

    /// Create optimization context from query information
    fn create_optimization_context(&self, _query: &Query) -> Result<OptimizationContext, String> {
        // Create a basic optimization context
        // In a full implementation, this would extract hints, analyze available indexes, etc.
        Ok(OptimizationContext::default())
    }

    /// Combine two optimization improvements
    fn combine_improvements(
        &self,
        improvement1: &OptimizationImprovement,
        improvement2: &OptimizationImprovement,
    ) -> OptimizationImprovement {
        OptimizationImprovement {
            cost_reduction_percentage: improvement1.cost_reduction_percentage
                + improvement2.cost_reduction_percentage,
            cardinality_reduction_percentage: improvement1.cardinality_reduction_percentage
                + improvement2.cardinality_reduction_percentage,
            original_cost: improvement1.original_cost + improvement2.original_cost,
            optimized_cost: improvement1.optimized_cost + improvement2.optimized_cost,
            original_rows: improvement1.original_rows + improvement2.original_rows,
            optimized_rows: improvement1.optimized_rows + improvement2.optimized_rows,
        }
    }

    /// Get current optimization metrics
    pub fn get_metrics(&self) -> &OptimizationMetrics {
        &self.metrics
    }

    /// Reset optimization metrics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Metrics management for performance monitoring
    pub fn reset_metrics(&mut self) {
        self.metrics = OptimizationMetrics::default();
    }

    /// Update configuration
    pub fn update_config(&mut self, config: IntegrationConfig) {
        self.config = config;
    }

    /// Record query execution for statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Query profiling for adaptive optimization
    pub fn record_query_execution(&mut self, query_text: &str, result_count: u64) {
        self.statistics_manager
            .record_query_execution(query_text, result_count);
    }
}

/// Result of the complete optimization pipeline
#[derive(Debug, Clone)]
pub struct OptimizationPipelineResult {
    /// The optimized physical plan
    pub physical_plan: PhysicalNode,
    /// Whether optimization was applied
    pub optimized: bool,
    /// Performance improvement from optimization
    pub improvement: OptimizationImprovement,
    /// Explanation of what happened
    pub explanation: String,
}

impl OptimizationPipelineResult {
    /// Create result for successful optimization
    pub fn optimized(
        physical_plan: PhysicalNode,
        improvement: OptimizationImprovement,
        explanation: String,
    ) -> Self {
        Self {
            physical_plan,
            optimized: true,
            improvement,
            explanation,
        }
    }

    /// Create result for no optimization
    pub fn no_optimization(physical_plan: PhysicalNode, explanation: String) -> Self {
        Self {
            physical_plan,
            optimized: false,
            improvement: OptimizationImprovement {
                cost_reduction_percentage: 0.0,
                cardinality_reduction_percentage: 0.0,
                original_cost: 0.0,
                optimized_cost: 0.0,
                original_rows: 0,
                optimized_rows: 0,
            },
            explanation,
        }
    }
}

/// Result of optimizing a single MATCH clause
#[derive(Debug, Clone)]
struct SingleMatchOptimizationResult {
    /// Result from logical optimization
    pub optimization_result: PatternOptimizationResult,
    /// Generated physical plan
    pub optimized_physical_plan: PhysicalNode,
    /// Performance improvement
    pub improvement: OptimizationImprovement,
}

/// Global pattern optimization manager
///
/// **Planned Feature** - Not yet integrated with query planner
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
pub struct GlobalPatternOptimizer {
    /// The main optimization pipeline
    pipeline: PatternOptimizationPipeline,
    /// Whether the optimizer is enabled globally
    enabled: bool,
}

impl GlobalPatternOptimizer {
    /// Create a new global optimizer
    pub fn new() -> Self {
        Self {
            pipeline: PatternOptimizationPipeline::new(),
            enabled: true,
        }
    }

    /// Enable or disable the global optimizer
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        let mut config = IntegrationConfig::default();
        config.enable_optimization = enabled;
        self.pipeline.update_config(config);
    }

    /// Check if optimizer is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Optimize a query (main integration point for the planner)
    pub fn optimize_query(
        &mut self,
        query: &Query,
        logical_plan: &LogicalPlan,
        physical_plan: &PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        if !self.enabled {
            return Ok(physical_plan.clone());
        }

        match self
            .pipeline
            .optimize_query(query, logical_plan, physical_plan)
        {
            Ok(result) => Ok(result.physical_plan),
            Err(e) => {
                // Log error but don't fail the query
                log::debug!("Pattern optimization error: {}", e);
                Ok(physical_plan.clone())
            }
        }
    }

    /// Get optimization metrics
    pub fn get_metrics(&self) -> &OptimizationMetrics {
        self.pipeline.get_metrics()
    }

    /// Print optimization statistics
    pub fn print_statistics(&self) {
        let metrics = self.get_metrics();
        log::debug!("\n🔍 Pattern Optimization Statistics:");
        log::debug!("   Queries Processed: {}", metrics.queries_processed);
        log::debug!("   Queries Optimized: {}", metrics.queries_optimized);
        log::debug!("   Success Rate: {:.1}%", metrics.success_rate * 100.0);
        log::debug!(
            "   Avg Optimization Time: {:.2}ms",
            metrics.avg_optimization_time_ms
        );
        log::debug!(
            "   Avg Cost Improvement: {:.1}%",
            metrics.avg_cost_improvement()
        );
        log::debug!(
            "   Avg Cardinality Reduction: {:.1}%",
            metrics.avg_cardinality_reduction()
        );

        if metrics.queries_optimized > 0 {
            log::debug!(
                "   🎉 Pattern optimization is working! Comma-separated pattern bug is fixed."
            );
        }
    }
}

/// Integration helper functions for the query planner
///
/// **Planned Feature** - Integration API for pattern optimization
/// See ROADMAP.md: "Pattern Optimization System"
#[allow(dead_code)]
pub mod integration_helpers {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    /// Global pattern optimizer instance
    ///
    /// **Planned Feature** - Will be used once pattern optimization is integrated
    #[allow(dead_code)]
    static GLOBAL_OPTIMIZER: Lazy<Mutex<GlobalPatternOptimizer>> =
        Lazy::new(|| Mutex::new(GlobalPatternOptimizer::new()));

    /// Enable pattern optimization globally
    ///
    /// **Planned Feature** - API to enable pattern optimization at runtime
    #[allow(dead_code)]
    pub fn enable_pattern_optimization() {
        if let Ok(mut optimizer) = GLOBAL_OPTIMIZER.lock() {
            optimizer.set_enabled(true);
            log::debug!(
                "✅ Pattern optimization enabled - comma-separated pattern bug fix is active"
            );
        }
    }

    /// Disable pattern optimization globally  
    pub fn disable_pattern_optimization() {
        if let Ok(mut optimizer) = GLOBAL_OPTIMIZER.lock() {
            optimizer.set_enabled(false);
            log::debug!("❌ Pattern optimization disabled");
        }
    }

    /// Check if pattern optimization is enabled
    pub fn is_pattern_optimization_enabled() -> bool {
        GLOBAL_OPTIMIZER
            .lock()
            .map(|opt| opt.is_enabled())
            .unwrap_or(false)
    }

    /// Optimize a query through the global optimizer
    pub fn optimize_query_global(
        query: &Query,
        logical_plan: &LogicalPlan,
        physical_plan: &PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        match GLOBAL_OPTIMIZER.lock() {
            Ok(mut optimizer) => optimizer.optimize_query(query, logical_plan, physical_plan),
            Err(_) => Ok(physical_plan.clone()), // Fallback on lock failure
        }
    }

    /// Print global optimization statistics
    pub fn print_global_statistics() {
        if let Ok(optimizer) = GLOBAL_OPTIMIZER.lock() {
            optimizer.print_statistics();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_creation() {
        let pipeline = PatternOptimizationPipeline::new();
        assert!(pipeline.config.enable_optimization);
        assert_eq!(pipeline.metrics.queries_processed, 0);
    }

    #[test]
    fn test_pipeline_with_config() {
        let config = IntegrationConfig {
            enable_optimization: false,
            enable_logging: true,
            fallback_on_error: false,
            min_improvement_threshold: 20.0,
        };

        let pipeline = PatternOptimizationPipeline::with_config(config.clone());
        assert!(!pipeline.config.enable_optimization);
        assert!(pipeline.config.enable_logging);
        assert!(!pipeline.config.fallback_on_error);
        assert_eq!(pipeline.config.min_improvement_threshold, 20.0);
    }

    #[test]
    fn test_optimization_metrics() {
        let mut metrics = OptimizationMetrics::default();

        let improvement = OptimizationImprovement {
            cost_reduction_percentage: 85.0,
            cardinality_reduction_percentage: 83.3,
            original_cost: 1000.0,
            optimized_cost: 150.0,
            original_rows: 18,
            optimized_rows: 3,
        };

        metrics.record_success(50, &improvement);

        assert_eq!(metrics.queries_processed, 1);
        assert_eq!(metrics.queries_optimized, 1);
        assert_eq!(metrics.success_rate, 1.0);
        assert_eq!(metrics.avg_optimization_time_ms, 50.0);
        assert_eq!(metrics.avg_cost_improvement(), 85.0);
        assert_eq!(metrics.avg_cardinality_reduction(), 83.3);
    }

    #[test]
    fn test_global_optimizer() {
        let mut optimizer = GlobalPatternOptimizer::new();
        assert!(optimizer.is_enabled());

        optimizer.set_enabled(false);
        assert!(!optimizer.is_enabled());

        optimizer.set_enabled(true);
        assert!(optimizer.is_enabled());
    }

    #[test]
    fn test_integration_helpers() {
        // Test enabling and disabling
        integration_helpers::enable_pattern_optimization();
        assert!(integration_helpers::is_pattern_optimization_enabled());

        integration_helpers::disable_pattern_optimization();
        assert!(!integration_helpers::is_pattern_optimization_enabled());

        // Re-enable for other tests
        integration_helpers::enable_pattern_optimization();
    }

    #[test]
    fn test_optimization_pipeline_result() {
        let physical_plan = PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 50.0,
        };

        let improvement = OptimizationImprovement {
            cost_reduction_percentage: 90.0,
            cardinality_reduction_percentage: 83.3,
            original_cost: 500.0,
            optimized_cost: 50.0,
            original_rows: 18,
            optimized_rows: 3,
        };

        let result = OptimizationPipelineResult::optimized(
            physical_plan.clone(),
            improvement.clone(),
            "Path traversal optimization applied".to_string(),
        );

        assert!(result.optimized);
        assert_eq!(result.improvement.cost_reduction_percentage, 90.0);
        assert!(result.explanation.contains("Path traversal"));

        let no_opt_result = OptimizationPipelineResult::no_optimization(
            physical_plan,
            "No optimization needed".to_string(),
        );

        assert!(!no_opt_result.optimized);
        assert_eq!(no_opt_result.improvement.cost_reduction_percentage, 0.0);
    }
}
