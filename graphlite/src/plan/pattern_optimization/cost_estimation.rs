// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Cost estimation for pattern optimization strategies
//!
//! This module provides cost estimation functionality for different pattern execution strategies.
//! It helps the query planner choose between path traversal, joins, and Cartesian products.

use crate::ast::PathPattern;
use crate::plan::pattern_optimization::pattern_analysis::{
    JoinStep, LinearPath, PatternPlanStrategy,
};
use std::collections::HashMap;

/// Statistics about graph data used for cost estimation
///
/// **Planned Feature** - Graph statistics collection for cost estimation
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct GraphStatistics {
    /// Number of nodes per label
    pub node_counts: HashMap<String, u64>,
    /// Number of relationships per type
    pub relationship_counts: HashMap<String, u64>,
    /// Average relationships per node by type
    pub avg_relationships_per_node: HashMap<String, f64>,
    /// Selectivity estimates for common patterns
    pub pattern_selectivity: HashMap<String, f64>,
}

/// Cost estimates for different execution strategies
///
/// **Planned Feature** - Execution cost estimation for strategy selection
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ExecutionCost {
    /// Estimated number of operations
    pub operations: f64,
    /// Estimated memory usage in bytes
    pub memory_bytes: u64,
    /// Estimated intermediate result size
    pub intermediate_results: u64,
    /// Confidence level (0.0 to 1.0) of the estimate
    pub confidence: f64,
}

impl ExecutionCost {
    pub fn new(
        operations: f64,
        memory_bytes: u64,
        intermediate_results: u64,
        confidence: f64,
    ) -> Self {
        Self {
            operations,
            memory_bytes,
            intermediate_results,
            confidence,
        }
    }

    /// Calculate total cost score for comparison (lower is better)
    pub fn total_cost(&self) -> f64 {
        // Weight operations heavily, include memory and intermediate results
        self.operations
            + (self.memory_bytes as f64 * 0.001)
            + (self.intermediate_results as f64 * 0.01)
    }

    /// Compare costs considering confidence levels
    #[allow(dead_code)] // ROADMAP v0.4.0 - Cost comparison for selecting optimal execution strategy
    pub fn is_better_than(&self, other: &ExecutionCost) -> bool {
        let self_adjusted = self.total_cost() / self.confidence.max(0.1);
        let other_adjusted = other.total_cost() / other.confidence.max(0.1);
        self_adjusted < other_adjusted
    }
}

/// Cost estimator for pattern execution strategies
///
/// **Planned Feature** - Cost-based optimizer for pattern execution
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
pub struct PlanCostEstimator {
    statistics: GraphStatistics,
    /// Cache of previously calculated costs
    cost_cache: HashMap<String, ExecutionCost>,
}

impl PlanCostEstimator {
    pub fn new(statistics: GraphStatistics) -> Self {
        Self {
            statistics,
            cost_cache: HashMap::new(),
        }
    }

    /// Estimate cost for a given pattern execution strategy
    pub fn estimate_cost(&mut self, strategy: &PatternPlanStrategy) -> ExecutionCost {
        match strategy {
            PatternPlanStrategy::PathTraversal(linear_path) => {
                self.estimate_path_traversal_cost(linear_path)
            }
            PatternPlanStrategy::HashJoin {
                patterns,
                join_order,
                ..
            } => self.estimate_hash_join_cost(patterns, join_order),
            PatternPlanStrategy::NestedLoopJoin { patterns, .. } => {
                self.estimate_nested_loop_join_cost(patterns)
            }
            PatternPlanStrategy::CartesianProduct { patterns, .. } => {
                self.estimate_cartesian_product_cost(patterns)
            }
        }
    }

    /// Estimate cost for path traversal strategy
    fn estimate_path_traversal_cost(&self, linear_path: &LinearPath) -> ExecutionCost {
        let mut total_operations = 0.0;
        let mut total_memory = 0u64;
        let mut current_cardinality = 1.0;

        for step in &linear_path.steps {
            // Estimate starting nodes for this step
            let start_nodes = if total_operations == 0.0 {
                // First step - use node count statistics
                // For now, use a default estimate since we don't have direct access to node labels
                1000.0
            } else {
                current_cardinality
            };

            // Estimate relationship traversal cost
            // Get the first label as the relationship type, or use default
            let default_type = "DEFAULT".to_string();
            let relationship_type = step.relationship.labels.first().unwrap_or(&default_type);
            let avg_relationships = self.get_avg_relationships(relationship_type).unwrap_or(2.0);
            let relationship_cost = start_nodes * avg_relationships;

            total_operations += relationship_cost;
            current_cardinality = relationship_cost;

            // Memory for intermediate results (assume 1KB per result)
            total_memory += (current_cardinality as u64) * 1024;
        }

        ExecutionCost::new(
            total_operations,
            total_memory,
            current_cardinality as u64,
            0.8, // High confidence for path traversal
        )
    }

    /// Estimate cost for hash join strategy
    fn estimate_hash_join_cost(
        &self,
        patterns: &[PathPattern],
        join_order: &[JoinStep],
    ) -> ExecutionCost {
        let mut total_operations = 0.0;
        let mut total_memory = 0u64;
        let mut intermediate_size = 0u64;

        // Estimate cost for each pattern individually
        for pattern in patterns {
            let pattern_cost = self.estimate_single_pattern_cost(pattern);
            total_operations += pattern_cost.operations;
            total_memory += pattern_cost.memory_bytes;
            intermediate_size += pattern_cost.intermediate_results;
        }

        // Add join costs
        for _join_step in join_order {
            // Hash join cost is approximately O(n + m) where n, m are input sizes
            let join_cost = intermediate_size as f64 * 1.5;
            total_operations += join_cost;

            // Hash table memory overhead
            total_memory += intermediate_size * 512; // 512 bytes per hash entry
        }

        ExecutionCost::new(
            total_operations,
            total_memory,
            intermediate_size,
            0.7, // Good confidence for hash joins
        )
    }

    /// Estimate cost for nested loop join strategy
    fn estimate_nested_loop_join_cost(&self, patterns: &[PathPattern]) -> ExecutionCost {
        if patterns.len() < 2 {
            return self.estimate_single_pattern_cost(&patterns[0]);
        }

        // Nested loop join is O(n * m) - very expensive
        let mut total_operations = 1.0;
        let mut total_memory = 0u64;

        for pattern in patterns {
            let pattern_cost = self.estimate_single_pattern_cost(pattern);
            total_operations *= pattern_cost.intermediate_results as f64;
            total_memory += pattern_cost.memory_bytes;
        }

        ExecutionCost::new(
            total_operations,
            total_memory,
            total_operations as u64,
            0.9, // High confidence but terrible performance
        )
    }

    /// Estimate cost for Cartesian product strategy
    fn estimate_cartesian_product_cost(&self, patterns: &[PathPattern]) -> ExecutionCost {
        let mut total_operations = 1.0;
        let mut total_memory = 0u64;
        let mut result_cardinality = 1u64;

        for pattern in patterns {
            let pattern_cost = self.estimate_single_pattern_cost(pattern);
            total_operations *= pattern_cost.intermediate_results as f64;
            total_memory += pattern_cost.memory_bytes;
            result_cardinality *= pattern_cost.intermediate_results;
        }

        ExecutionCost::new(
            total_operations,
            total_memory,
            result_cardinality,
            0.95, // Very high confidence - we know it's expensive
        )
    }

    /// Estimate cost for a single pattern
    fn estimate_single_pattern_cost(&self, pattern: &PathPattern) -> ExecutionCost {
        // Simple estimation based on pattern complexity
        let base_operations = match pattern.elements.len() {
            0 | 1 => 100.0,
            2 => 1000.0,
            3 => 10000.0,
            _ => 100000.0,
        };

        // Estimate selectivity based on filters
        let selectivity = self.estimate_pattern_selectivity(pattern);
        let operations = base_operations * selectivity;
        let results = (operations * 0.1) as u64;

        ExecutionCost::new(
            operations,
            results * 1024, // 1KB per result
            results,
            0.6, // Medium confidence for single patterns
        )
    }

    /// Estimate selectivity of a pattern (0.0 to 1.0)
    fn estimate_pattern_selectivity(&self, _pattern: &PathPattern) -> f64 {
        // For now, use simple heuristics
        let base_selectivity = 0.1; // Assume 10% selectivity

        // Reduce selectivity based on number of constraints
        // For now, assume no additional constraints since we don't have direct access to filters
        let constraint_factor = 1.0;

        base_selectivity * constraint_factor
    }

    /// Get average relationships per node for a relationship type
    fn get_avg_relationships(&self, rel_type: &str) -> Option<f64> {
        self.statistics
            .avg_relationships_per_node
            .get(rel_type)
            .copied()
    }

    /// Update statistics with new data
    #[allow(dead_code)] // ROADMAP v0.4.0 - Statistics refresh for adaptive cost estimation
    pub fn update_statistics(&mut self, new_stats: GraphStatistics) {
        self.statistics = new_stats;
        self.cost_cache.clear(); // Clear cache when stats change
    }

    /// Clear the cost cache
    #[allow(dead_code)] // ROADMAP v0.4.0 - Cache invalidation for statistics updates
    pub fn clear_cache(&mut self) {
        self.cost_cache.clear();
    }
}

/// Statistics manager for collecting graph statistics
///
/// **Planned Feature** - Statistics collection and management for query optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug)]
pub struct StatisticsManager {
    /// Current statistics
    statistics: GraphStatistics,
    /// Statistics update frequency
    update_threshold: u64,
    /// Counter for operations since last update
    operations_since_update: u64,
}

impl StatisticsManager {
    pub fn new() -> Self {
        Self {
            statistics: GraphStatistics::default(),
            update_threshold: 1000,
            operations_since_update: 0,
        }
    }

    /// Get current statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Statistics access for cost-based optimization
    pub fn get_statistics(&self) -> &GraphStatistics {
        &self.statistics
    }

    /// Record a query execution for statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Query profiling for statistics collection
    pub fn record_query_execution(&mut self, _query: &str, _result_count: u64) {
        self.operations_since_update += 1;

        if self.operations_since_update >= self.update_threshold {
            // In a real implementation, this would update statistics from the storage layer
            self.refresh_statistics();
        }
    }

    /// Refresh statistics from storage (placeholder)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Automatic statistics refresh from storage layer
    fn refresh_statistics(&mut self) {
        // TODO: Implement actual statistics collection from storage
        // For now, use default/estimated values

        // Reset counter
        self.operations_since_update = 0;

        // In Phase 3, this will integrate with the storage layer
        // to get actual node counts, relationship counts, etc.
    }

    /// Manually set statistics (for testing)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Test utilities for cost estimation
    pub fn set_statistics(&mut self, stats: GraphStatistics) {
        self.statistics = stats;
    }

    /// Get a cost estimator with current statistics
    pub fn create_cost_estimator(&self) -> PlanCostEstimator {
        PlanCostEstimator::new(self.statistics.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_cost_comparison() {
        let cost1 = ExecutionCost::new(1000.0, 1024, 100, 0.8);
        let cost2 = ExecutionCost::new(2000.0, 2048, 200, 0.9);

        assert!(cost1.is_better_than(&cost2));
        assert!(!cost2.is_better_than(&cost1));
    }

    #[test]
    fn test_cost_estimator_basic() {
        let mut stats = GraphStatistics::default();
        stats.node_counts.insert("Person".to_string(), 1000);
        stats.relationship_counts.insert("KNOWS".to_string(), 500);
        stats
            .avg_relationships_per_node
            .insert("KNOWS".to_string(), 2.0);

        let mut estimator = PlanCostEstimator::new(stats);

        // Test with a Cartesian product strategy (simpler to create)
        use crate::ast::{Location, PathPattern};

        let pattern = PathPattern {
            assignment: None,
            path_type: None,
            elements: vec![],
            location: Location::default(),
        };

        let strategy = PatternPlanStrategy::CartesianProduct {
            patterns: vec![pattern],
            estimated_cost: 0.0, // Use default cost value
        };
        let cost = estimator.estimate_cost(&strategy);

        assert!(cost.operations > 0.0);
        assert!(cost.confidence > 0.0);
    }

    #[test]
    fn test_statistics_manager() {
        let mut manager = StatisticsManager::new();
        let stats = manager.get_statistics();

        // Should start with empty statistics
        assert!(stats.node_counts.is_empty());
        assert!(stats.relationship_counts.is_empty());

        // Record some operations
        manager.record_query_execution("MATCH (n) RETURN n", 100);

        // Should work without errors
        let estimator = manager.create_cost_estimator();
        assert_eq!(estimator.statistics.node_counts.len(), 0);
    }
}
