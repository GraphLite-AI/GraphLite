// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical Planning Integration for Pattern Optimization
//!
//! This module integrates pattern optimization into the logical planning phase,
//! providing the main entry point for comma-separated pattern optimization.

use std::collections::HashMap;
use crate::ast::ast::{PathPattern, MatchClause};
use crate::plan::pattern_optimization::{
    pattern_analysis::{PatternConnectivity, PatternPlanStrategy},
    pattern_analyzer::PatternAnalyzer,
    cost_estimation::{StatisticsManager, ExecutionCost},
};
use crate::plan::logical::LogicalPlan;

/// Result of pattern optimization analysis
///
/// **Planned Feature** - Pattern optimization result for logical planning
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PatternOptimizationResult {
    /// The chosen execution strategy
    pub strategy: PatternPlanStrategy,
    /// Estimated cost of the strategy
    pub estimated_cost: ExecutionCost,
    /// Whether optimization was applied
    pub optimized: bool,
    /// Reason for the chosen strategy
    pub optimization_reason: String,
}

/// Main pattern optimizer for logical planning
///
/// **Planned Feature** - Logical-level pattern optimizer
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug)]
pub struct LogicalPatternOptimizer {
    /// Pattern analyzer for connectivity analysis
    pattern_analyzer: PatternAnalyzer,
    /// Statistics manager for cost estimation
    statistics_manager: StatisticsManager,
    /// Optimization settings
    config: OptimizationConfig,
}

/// Configuration for pattern optimization
///
/// **Planned Feature** - Configuration for optimization behavior
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Enable path traversal optimization
    pub enable_path_traversal: bool,
    /// Enable hash join optimization
    pub enable_hash_joins: bool,
    /// Minimum patterns required for optimization
    pub min_patterns_for_optimization: usize,
    /// Maximum patterns to consider for optimization
    pub max_patterns_for_optimization: usize,
    /// Cost threshold for applying optimization
    pub cost_improvement_threshold: f64,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_path_traversal: true,
            enable_hash_joins: true,
            min_patterns_for_optimization: 2,
            max_patterns_for_optimization: 10,
            cost_improvement_threshold: 0.1, // 10% improvement required
        }
    }
}

impl LogicalPatternOptimizer {
    /// Create a new logical pattern optimizer
    pub fn new() -> Self {
        Self {
            pattern_analyzer: PatternAnalyzer::new(),
            statistics_manager: StatisticsManager::new(),
            config: OptimizationConfig::default(),
        }
    }

    /// Create optimizer with custom configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Custom configuration for different optimization strategies
    pub fn with_config(config: OptimizationConfig) -> Self {
        Self {
            pattern_analyzer: PatternAnalyzer::new(),
            statistics_manager: StatisticsManager::new(),
            config,
        }
    }

    /// Main entry point for optimizing comma-separated patterns
    pub fn optimize_comma_separated_patterns(
        &mut self,
        patterns: &[PathPattern],
        context: &OptimizationContext,
    ) -> PatternOptimizationResult {
        // Skip optimization if not enough patterns
        if patterns.len() < self.config.min_patterns_for_optimization {
            return PatternOptimizationResult {
                strategy: PatternPlanStrategy::CartesianProduct {
                    patterns: patterns.to_vec(),
                    estimated_cost: 0.0,
                },
                estimated_cost: ExecutionCost::new(0.0, 0, 0, 1.0),
                optimized: false,
                optimization_reason: format!("Too few patterns ({}) for optimization", patterns.len()),
            };
        }

        // Skip optimization if too many patterns (avoid exponential complexity)
        if patterns.len() > self.config.max_patterns_for_optimization {
            return PatternOptimizationResult {
                strategy: PatternPlanStrategy::CartesianProduct {
                    patterns: patterns.to_vec(),
                    estimated_cost: 0.0,
                },
                estimated_cost: ExecutionCost::new(f64::INFINITY, u64::MAX, u64::MAX, 1.0),
                optimized: false,
                optimization_reason: format!("Too many patterns ({}) for optimization", patterns.len()),
            };
        }

        // Analyze pattern connectivity
        let connectivity = self.pattern_analyzer.analyze_patterns(patterns.to_vec());
        
        // Generate alternative execution strategies
        let strategies = self.generate_execution_strategies(&connectivity, context);
        
        // Estimate costs for each strategy
        let mut cost_estimator = self.statistics_manager.create_cost_estimator();
        let mut strategy_costs: Vec<(PatternPlanStrategy, ExecutionCost)> = Vec::new();
        
        for strategy in strategies {
            let cost = cost_estimator.estimate_cost(&strategy);
            strategy_costs.push((strategy, cost));
        }
        
        // Choose the best strategy
        self.choose_best_strategy(strategy_costs, context)
    }

    /// Generate possible execution strategies for the patterns
    fn generate_execution_strategies(
        &self,
        connectivity: &PatternConnectivity,
        context: &OptimizationContext,
    ) -> Vec<PatternPlanStrategy> {
        let mut strategies = Vec::new();
        
        // Always include Cartesian product as baseline
        strategies.push(PatternPlanStrategy::CartesianProduct {
            patterns: connectivity.patterns.clone(),
            estimated_cost: 0.0,
        });
        
        // Try path traversal optimization if enabled and applicable
        if self.config.enable_path_traversal {
            if let Some(linear_path) = self.pattern_analyzer.detect_linear_path(connectivity) {
                strategies.push(PatternPlanStrategy::PathTraversal(linear_path));
            }
        }
        
        // Try hash join optimization if enabled
        if self.config.enable_hash_joins && connectivity.patterns.len() >= 2 {
            if let Some(join_strategy) = self.generate_hash_join_strategy(connectivity, context) {
                strategies.push(join_strategy);
            }
        }
        
        // Add nested loop join as fallback for complex cases
        if connectivity.patterns.len() <= 4 { // Only for small pattern sets
            strategies.push(PatternPlanStrategy::NestedLoopJoin {
                patterns: connectivity.patterns.clone(),
                estimated_cost: 0.0,
            });
        }
        
        strategies
    }
    
    /// Generate hash join strategy if applicable
    fn generate_hash_join_strategy(
        &self,
        connectivity: &PatternConnectivity,
        _context: &OptimizationContext,
    ) -> Option<PatternPlanStrategy> {
        // For now, use simple join ordering based on pattern complexity
        let join_order = self.generate_join_order(connectivity);
        
        if join_order.is_empty() {
            return None;
        }
        
        Some(PatternPlanStrategy::HashJoin {
            patterns: connectivity.patterns.clone(),
            join_order,
            estimated_cost: 0.0,
        })
    }
    
    /// Generate join order for hash joins
    fn generate_join_order(&self, connectivity: &PatternConnectivity) -> Vec<crate::plan::pattern_optimization::pattern_analysis::JoinStep> {
        use crate::plan::pattern_optimization::pattern_analysis::{JoinStep, JoinType};
        
        let mut join_order = Vec::new();
        
        // Simple strategy: join patterns in order of shared variables
        for (var, pattern_indices) in &connectivity.shared_variables {
            if pattern_indices.len() >= 2 {
                // Create join step for patterns sharing this variable
                let join_step = JoinStep {
                    left_pattern_idx: pattern_indices[0],
                    right_pattern_idx: pattern_indices[1],
                    join_variables: vec![var.clone()],
                    join_type: JoinType::Hash,
                    estimated_cost: 0.1, // Default cost
                };
                join_order.push(join_step);
            }
        }
        
        join_order
    }
    
    /// Choose the best execution strategy based on cost estimates
    fn choose_best_strategy(
        &self,
        strategy_costs: Vec<(PatternPlanStrategy, ExecutionCost)>,
        _context: &OptimizationContext,
    ) -> PatternOptimizationResult {
        if strategy_costs.is_empty() {
            return PatternOptimizationResult {
                strategy: PatternPlanStrategy::CartesianProduct {
                    patterns: vec![],
                    estimated_cost: 0.0,
                },
                estimated_cost: ExecutionCost::new(f64::INFINITY, u64::MAX, u64::MAX, 0.0),
                optimized: false,
                optimization_reason: "No strategies available".to_string(),
            };
        }
        
        // Find the strategy with the best cost
        let mut best_strategy_idx = 0;
        let mut best_cost_score = strategy_costs[0].1.total_cost();
        
        for (i, (_, cost)) in strategy_costs.iter().enumerate() {
            let cost_score = cost.total_cost();
            if cost_score < best_cost_score {
                best_strategy_idx = i;
                best_cost_score = cost_score;
            }
        }
        
        let best_strategy = &strategy_costs[best_strategy_idx];
        
        // Check if we found a better strategy than Cartesian product
        let cartesian_cost = strategy_costs.iter()
            .find(|(s, _)| matches!(s, PatternPlanStrategy::CartesianProduct { .. }))
            .map(|(_, cost)| cost.total_cost())
            .unwrap_or(f64::INFINITY);
            
        let optimized = best_cost_score < cartesian_cost * (1.0 - self.config.cost_improvement_threshold);
        
        let optimization_reason = if optimized {
            match &best_strategy.0 {
                PatternPlanStrategy::PathTraversal(_) => 
                    "Path traversal optimization applied for connected patterns".to_string(),
                PatternPlanStrategy::HashJoin { .. } => 
                    "Hash join optimization applied for shared variables".to_string(),
                PatternPlanStrategy::NestedLoopJoin { .. } => 
                    "Nested loop join selected for complex patterns".to_string(),
                PatternPlanStrategy::CartesianProduct { .. } => 
                    "Cartesian product is optimal for these patterns".to_string(),
            }
        } else {
            format!(
                "No significant improvement found (best: {:.2}, baseline: {:.2})", 
                best_cost_score, cartesian_cost
            )
        };
        
        PatternOptimizationResult {
            strategy: best_strategy.0.clone(),
            estimated_cost: best_strategy.1.clone(),
            optimized,
            optimization_reason,
        }
    }
    
    /// Update statistics from query execution
    #[allow(dead_code)] // ROADMAP v0.4.0 - Query profiling for adaptive optimization
    pub fn record_query_execution(&mut self, query: &str, result_count: u64) {
        self.statistics_manager.record_query_execution(query, result_count);
    }

    /// Get current optimization configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Configuration inspection for optimization tuning
    pub fn get_config(&self) -> &OptimizationConfig {
        &self.config
    }

    /// Update optimization configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Runtime configuration updates for adaptive optimization
    pub fn update_config(&mut self, config: OptimizationConfig) {
        self.config = config;
    }
}

/// Context information for pattern optimization
///
/// **Planned Feature** - Runtime context for optimization decisions
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OptimizationContext {
    /// Available indexes for optimization
    pub available_indexes: Vec<IndexInfo>,
    /// Query hints or preferences
    pub hints: HashMap<String, String>,
    /// Memory budget for the query
    pub memory_budget: Option<u64>,
    /// Performance requirements
    pub performance_requirements: PerformanceRequirements,
}

/// Information about available indexes
///
/// **Planned Feature** - Index information for optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct IndexInfo {
    /// Type of index (e.g., "btree", "hash", "fulltext")
    pub index_type: String,
    /// Node labels covered by the index
    pub node_labels: Vec<String>,
    /// Relationship types covered by the index
    pub relationship_types: Vec<String>,
    /// Properties covered by the index
    pub properties: Vec<String>,
    /// Estimated selectivity of the index
    pub selectivity: f64,
}

/// Performance requirements for query optimization
///
/// **Planned Feature** - Performance requirements for optimization decisions
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PerformanceRequirements {
    /// Maximum acceptable execution time in milliseconds
    pub max_execution_time_ms: Option<u64>,
    /// Maximum acceptable memory usage in bytes
    pub max_memory_bytes: Option<u64>,
    /// Priority: "speed" or "memory"
    pub optimization_priority: OptimizationPriority,
}

/// Optimization priority settings
///
/// **Planned Feature** - Optimization priority configuration
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum OptimizationPriority {
    /// Optimize for fastest execution time
    Speed,
    /// Optimize for lowest memory usage
    Memory,
    /// Balance between speed and memory
    Balanced,
}

impl Default for OptimizationContext {
    fn default() -> Self {
        Self {
            available_indexes: vec![],
            hints: HashMap::new(),
            memory_budget: None,
            performance_requirements: PerformanceRequirements {
                max_execution_time_ms: None,
                max_memory_bytes: None,
                optimization_priority: OptimizationPriority::Balanced,
            },
        }
    }
}

/// Integration point for the logical planner
pub fn optimize_match_clause_patterns(
    optimizer: &mut LogicalPatternOptimizer,
    match_clause: &MatchClause,
    context: &OptimizationContext,
) -> Result<PatternOptimizationResult, String> {
    // Extract patterns from the match clause
    let patterns: Vec<PathPattern> = match_clause.patterns.clone();
    
    if patterns.is_empty() {
        return Ok(PatternOptimizationResult {
            strategy: PatternPlanStrategy::CartesianProduct {
                patterns: vec![],
                estimated_cost: 0.0,
            },
            estimated_cost: ExecutionCost::new(0.0, 0, 0, 1.0),
            optimized: false,
            optimization_reason: "No patterns to optimize".to_string(),
        });
    }
    
    // Apply pattern optimization
    let result = optimizer.optimize_comma_separated_patterns(&patterns, context);
    
    Ok(result)
}

/// Convert optimization result to logical plan modifications
#[allow(dead_code)] // ROADMAP v0.4.0 - Logical plan transformation for optimized pattern execution
pub fn apply_optimization_to_logical_plan(
    optimization_result: &PatternOptimizationResult,
    current_plan: &LogicalPlan,
) -> Result<LogicalPlan, String> {
    if !optimization_result.optimized {
        // No optimization applied, return original plan
        return Ok(current_plan.clone());
    }
    
    // For Phase 3, we'll create a placeholder for the optimized plan
    // Phase 4 will implement the actual physical plan generation
    let optimized_plan = current_plan.clone();
    
    // Add optimization metadata to the plan
    // This will be used by Phase 4 for physical plan generation
    match &optimization_result.strategy {
        PatternPlanStrategy::PathTraversal(_) => {
            // Mark the plan for path traversal optimization
            // Physical planner will implement the actual traversal logic
        }
        PatternPlanStrategy::HashJoin { .. } => {
            // Mark the plan for hash join optimization
            // Physical planner will implement the actual join logic
        }
        PatternPlanStrategy::NestedLoopJoin { .. } => {
            // Mark the plan for nested loop join optimization
        }
        PatternPlanStrategy::CartesianProduct { .. } => {
            // Use default Cartesian product behavior
        }
    }
    
    Ok(optimized_plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ast::Location;

    #[test]
    fn test_pattern_optimizer_creation() {
        let optimizer = LogicalPatternOptimizer::new();
        assert_eq!(optimizer.config.min_patterns_for_optimization, 2);
        assert!(optimizer.config.enable_path_traversal);
    }

    #[test]
    fn test_optimization_with_few_patterns() {
        let mut optimizer = LogicalPatternOptimizer::new();
        let context = OptimizationContext::default();
        
        // Test with single pattern - should not optimize
        let pattern = PathPattern {
            assignment: None,
            path_type: None,
            elements: vec![],
            location: Location::default(),
        };
        
        let result = optimizer.optimize_comma_separated_patterns(&[pattern], &context);
        assert!(!result.optimized);
        assert!(result.optimization_reason.contains("Too few patterns"));
    }

    #[test]
    fn test_optimization_with_many_patterns() {
        let mut optimizer = LogicalPatternOptimizer::new();
        let context = OptimizationContext::default();
        
        // Test with too many patterns - should not optimize
        let patterns: Vec<PathPattern> = (0..15).map(|_| PathPattern {
            assignment: None,
            path_type: None,
            elements: vec![],
            location: Location::default(),
        }).collect();
        
        let result = optimizer.optimize_comma_separated_patterns(&patterns, &context);
        assert!(!result.optimized);
        assert!(result.optimization_reason.contains("Too many patterns"));
    }

    #[test]
    fn test_optimization_context_default() {
        let context = OptimizationContext::default();
        assert_eq!(context.performance_requirements.optimization_priority, OptimizationPriority::Balanced);
        assert!(context.available_indexes.is_empty());
        assert!(context.hints.is_empty());
    }

    #[test]
    fn test_optimization_config_custom() {
        let config = OptimizationConfig {
            enable_path_traversal: false,
            enable_hash_joins: true,
            min_patterns_for_optimization: 3,
            max_patterns_for_optimization: 5,
            cost_improvement_threshold: 0.2,
        };
        
        let optimizer = LogicalPatternOptimizer::with_config(config.clone());
        assert_eq!(optimizer.config.min_patterns_for_optimization, 3);
        assert!(!optimizer.config.enable_path_traversal);
        assert!(optimizer.config.enable_hash_joins);
    }
}
