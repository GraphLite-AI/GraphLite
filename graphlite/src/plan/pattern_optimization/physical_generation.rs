// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical Plan Generation for Pattern Optimization
//!
//! This module generates optimized physical plans for comma-separated patterns,
//! integrating with existing physical operators (HashJoin, NestedLoopJoin, PathTraversal).

use crate::ast::{Expression, PathPattern, PathType, Variable};
use crate::plan::logical::{JoinType, PathElement};
use crate::plan::pattern_optimization::{
    logical_integration::PatternOptimizationResult,
    pattern_analysis::{JoinStep, LinearPath, PatternPlanStrategy, TraversalStep},
};
use crate::plan::physical::PhysicalNode;

/// Physical plan generator for optimized pattern execution
///
/// **Planned Feature** - Physical plan generation for pattern optimization
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug)]
pub struct PhysicalPatternPlanGenerator {
    /// Configuration for plan generation
    config: PhysicalGenerationConfig,
}

/// Configuration for physical plan generation
///
/// **Planned Feature** - Configuration for physical plan generation
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PhysicalGenerationConfig {
    /// Enable aggressive optimization
    pub enable_aggressive_optimization: bool,
    /// Use index-backed traversal when available
    pub prefer_indexed_access: bool,
    /// Maximum depth for path traversal optimization
    pub max_traversal_depth: usize,
    /// Enable parallel join execution
    pub enable_parallel_joins: bool,
}

impl Default for PhysicalGenerationConfig {
    fn default() -> Self {
        Self {
            enable_aggressive_optimization: true,
            prefer_indexed_access: true,
            max_traversal_depth: 5,
            enable_parallel_joins: false, // Disabled by default for safety
        }
    }
}

impl PhysicalPatternPlanGenerator {
    /// Create a new physical plan generator
    pub fn new() -> Self {
        Self {
            config: PhysicalGenerationConfig::default(),
        }
    }

    /// Create generator with custom configuration
    pub fn with_config(config: PhysicalGenerationConfig) -> Self {
        Self { config }
    }

    /// Generate optimized physical plan from optimization result
    pub fn generate_physical_plan(
        &self,
        optimization_result: &PatternOptimizationResult,
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        if !optimization_result.optimized {
            // No optimization applied, return the base plan with Cartesian product behavior
            return Ok(base_input);
        }

        match &optimization_result.strategy {
            PatternPlanStrategy::PathTraversal(linear_path) => {
                self.generate_path_traversal_plan(linear_path, base_input)
            }
            PatternPlanStrategy::HashJoin {
                patterns,
                join_order,
                ..
            } => self.generate_hash_join_plan(patterns, join_order, base_input),
            PatternPlanStrategy::NestedLoopJoin { patterns, .. } => {
                self.generate_nested_loop_plan(patterns, base_input)
            }
            PatternPlanStrategy::CartesianProduct { patterns, .. } => {
                self.generate_cartesian_product_plan(patterns, base_input)
            }
        }
    }

    /// Generate physical plan for path traversal optimization (the key fix for comma-separated patterns)
    fn generate_path_traversal_plan(
        &self,
        linear_path: &LinearPath,
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        // This is the core implementation that fixes the comma-separated pattern bug
        // Instead of creating a Cartesian product, we create a connected path traversal

        let mut current_plan = base_input;
        let estimated_rows = current_plan.get_row_count();
        let base_cost = self.get_node_cost(&current_plan);

        // Convert the linear path into a series of connected path traversal operations
        for (i, step) in linear_path.steps.iter().enumerate() {
            let path_elements = self.create_path_elements_from_step(step)?;

            // Create path traversal node that connects patterns through shared variables
            let traversal_node = PhysicalNode::PathTraversal {
                path_type: self.determine_path_type(step),
                from_variable: step.from_var.clone(),
                to_variable: step.to_var.clone(),
                path_elements,
                input: Box::new(current_plan),
                estimated_rows: (estimated_rows as f64 * step.selectivity) as usize,
                estimated_cost: base_cost + (i as f64 * 100.0), // Incremental cost per step
            };

            current_plan = traversal_node;
        }

        Ok(current_plan)
    }

    /// Generate physical plan using existing HashJoin operators
    fn generate_hash_join_plan(
        &self,
        patterns: &[PathPattern],
        join_order: &[JoinStep],
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        if join_order.is_empty() {
            return Err("No join order specified for hash join plan".to_string());
        }

        let mut current_plan = base_input;
        let base_rows = current_plan.get_row_count();
        let base_cost = self.get_node_cost(&current_plan);

        // Execute joins in the specified order
        for (i, join_step) in join_order.iter().enumerate() {
            // Create build and probe sides from the patterns
            let build_pattern = patterns
                .get(join_step.left_pattern_idx)
                .ok_or("Invalid left pattern index")?;
            let probe_pattern = patterns
                .get(join_step.right_pattern_idx)
                .ok_or("Invalid right pattern index")?;

            // Generate sub-plans for build and probe sides
            let build_plan = self.create_pattern_subplan(build_pattern, current_plan.clone())?;
            let probe_plan = self.create_pattern_subplan(probe_pattern, current_plan.clone())?;

            // Create join keys from the shared variables
            let build_keys = self.create_join_keys(&join_step.join_variables, build_pattern)?;
            let probe_keys = self.create_join_keys(&join_step.join_variables, probe_pattern)?;

            // Create the hash join node
            let join_node = PhysicalNode::HashJoin {
                join_type: self.convert_join_type(&join_step.join_type),
                condition: None, // Join condition is expressed through keys
                build_keys,
                probe_keys,
                build: Box::new(build_plan),
                probe: Box::new(probe_plan),
                estimated_rows: (base_rows as f64 * join_step.estimated_cost) as usize,
                estimated_cost: base_cost + (i as f64 * 200.0), // Hash join cost
            };

            current_plan = join_node;
        }

        Ok(current_plan)
    }

    /// Generate physical plan using existing NestedLoopJoin operators
    fn generate_nested_loop_plan(
        &self,
        patterns: &[PathPattern],
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        if patterns.len() < 2 {
            return Ok(base_input);
        }

        let mut current_plan = base_input;
        let base_rows = current_plan.get_row_count();
        let base_cost = self.get_node_cost(&current_plan);

        // Create nested loop joins for each pattern pair
        for (i, pattern_pair) in patterns.windows(2).enumerate() {
            let left_pattern = &pattern_pair[0];
            let right_pattern = &pattern_pair[1];

            // Generate sub-plans
            let left_plan = self.create_pattern_subplan(left_pattern, current_plan.clone())?;
            let right_plan = self.create_pattern_subplan(right_pattern, current_plan.clone())?;

            // Create nested loop join
            let join_node = PhysicalNode::NestedLoopJoin {
                join_type: JoinType::Inner, // Default to inner join
                condition: self.create_join_condition(left_pattern, right_pattern)?,
                left: Box::new(left_plan),
                right: Box::new(right_plan),
                estimated_rows: base_rows * base_rows, // Nested loop can be expensive
                estimated_cost: base_cost + (i as f64 * 500.0), // Higher cost for nested loops
            };

            current_plan = join_node;
        }

        Ok(current_plan)
    }

    /// Generate physical plan for Cartesian product (fallback behavior)
    fn generate_cartesian_product_plan(
        &self,
        patterns: &[PathPattern],
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        // For Cartesian product, we use nested loop joins without join conditions
        // This maintains the original behavior but makes it explicit

        if patterns.len() < 2 {
            return Ok(base_input);
        }

        let mut current_plan = base_input;
        let base_rows = current_plan.get_row_count();
        let base_cost = self.get_node_cost(&current_plan);

        for (i, pattern_pair) in patterns.windows(2).enumerate() {
            let left_pattern = &pattern_pair[0];
            let right_pattern = &pattern_pair[1];

            let left_plan = self.create_pattern_subplan(left_pattern, current_plan.clone())?;
            let right_plan = self.create_pattern_subplan(right_pattern, current_plan.clone())?;

            // Cartesian product = nested loop join with no condition
            let cartesian_node = PhysicalNode::NestedLoopJoin {
                join_type: JoinType::Inner,
                condition: None, // No condition = Cartesian product
                left: Box::new(left_plan),
                right: Box::new(right_plan),
                estimated_rows: base_rows * base_rows,
                estimated_cost: base_cost + (i as f64 * 1000.0), // Very expensive
            };

            current_plan = cartesian_node;
        }

        Ok(current_plan)
    }

    /// Helper: Create path elements from a traversal step
    fn create_path_elements_from_step(
        &self,
        step: &TraversalStep,
    ) -> Result<Vec<PathElement>, String> {
        // Convert the relationship pattern into path elements
        let path_element = PathElement {
            edge_variable: Some(step.from_var.clone()),
            node_variable: step.to_var.clone(),
            edge_labels: step.relationship.labels.clone(),
            direction: step.relationship.direction.clone(),
            quantifier: step.relationship.quantifier.clone(),
        };

        Ok(vec![path_element])
    }

    /// Helper: Determine path type from traversal step
    fn determine_path_type(&self, _step: &TraversalStep) -> PathType {
        // For now, use simple path type
        // In the future, this could be more sophisticated based on step characteristics
        PathType::SimplePath
    }

    /// Helper: Create a sub-plan for a single pattern
    fn create_pattern_subplan(
        &self,
        _pattern: &PathPattern,
        base_input: PhysicalNode,
    ) -> Result<PhysicalNode, String> {
        // For now, return the base input
        // In a full implementation, this would create a sub-plan specific to the pattern
        Ok(base_input)
    }

    /// Helper: Create join keys from shared variables
    fn create_join_keys(
        &self,
        variables: &[String],
        _pattern: &PathPattern,
    ) -> Result<Vec<Expression>, String> {
        // Create expressions for the join variables
        let mut keys = Vec::new();
        for var in variables {
            // Create a variable reference expression
            let variable = Variable {
                name: var.clone(),
                location: crate::ast::Location::default(),
            };
            let key_expr = Expression::Variable(variable);
            keys.push(key_expr);
        }
        Ok(keys)
    }

    /// Helper: Convert join type from pattern analysis to physical
    fn convert_join_type(
        &self,
        join_type: &crate::plan::pattern_optimization::pattern_analysis::JoinType,
    ) -> JoinType {
        match join_type {
            crate::plan::pattern_optimization::pattern_analysis::JoinType::Hash => JoinType::Inner,
            crate::plan::pattern_optimization::pattern_analysis::JoinType::NestedLoop => {
                JoinType::Inner
            }
            crate::plan::pattern_optimization::pattern_analysis::JoinType::IndexLookup => {
                JoinType::Inner
            }
        }
    }

    /// Helper: Create join condition between two patterns
    fn create_join_condition(
        &self,
        _left_pattern: &PathPattern,
        _right_pattern: &PathPattern,
    ) -> Result<Option<Expression>, String> {
        // For now, return None (no explicit condition)
        // In a full implementation, this would analyze patterns to find shared variables
        Ok(None)
    }

    /// Get the current configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Configuration inspection for physical plan generation
    pub fn get_config(&self) -> &PhysicalGenerationConfig {
        &self.config
    }

    /// Update configuration
    #[allow(dead_code)] // ROADMAP v0.4.0 - Runtime configuration updates for physical plan tuning
    pub fn update_config(&mut self, config: PhysicalGenerationConfig) {
        self.config = config;
    }

    /// Helper: Get cost from a physical node (since get_estimated_cost method doesn't exist)
    fn get_node_cost(&self, node: &PhysicalNode) -> f64 {
        // Extract cost from the node based on its variant
        match node {
            PhysicalNode::NodeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NodeIndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::EdgeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::IndexedExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::PathTraversal { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Filter { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Project { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NestedLoopJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::SortMergeJoin { estimated_cost, .. } => *estimated_cost,
            _ => 100.0, // Default cost for other node types
        }
    }
}

/// Integration point for physical planning
pub fn generate_optimized_physical_plan(
    optimization_result: &PatternOptimizationResult,
    base_input: PhysicalNode,
    config: Option<PhysicalGenerationConfig>,
) -> Result<PhysicalNode, String> {
    let generator = if let Some(config) = config {
        PhysicalPatternPlanGenerator::with_config(config)
    } else {
        PhysicalPatternPlanGenerator::new()
    };

    generator.generate_physical_plan(optimization_result, base_input)
}

/// Utility function to estimate the improvement of an optimized plan
pub fn estimate_optimization_improvement(
    original_plan: &PhysicalNode,
    optimized_plan: &PhysicalNode,
) -> OptimizationImprovement {
    // Helper function to get cost from any physical node
    let get_cost = |node: &PhysicalNode| -> f64 {
        match node {
            PhysicalNode::NodeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NodeIndexScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::EdgeSeqScan { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::IndexedExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashExpand { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::PathTraversal { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Filter { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::Project { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::HashJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::NestedLoopJoin { estimated_cost, .. } => *estimated_cost,
            PhysicalNode::SortMergeJoin { estimated_cost, .. } => *estimated_cost,
            _ => 100.0,
        }
    };

    let original_cost = get_cost(original_plan);
    let optimized_cost = get_cost(optimized_plan);
    let original_rows = original_plan.get_row_count();
    let optimized_rows = optimized_plan.get_row_count();

    let cost_improvement = if original_cost > 0.0 {
        (original_cost - optimized_cost) / original_cost
    } else {
        0.0
    };

    let cardinality_improvement = if original_rows > 0 {
        (original_rows as f64 - optimized_rows as f64) / original_rows as f64
    } else {
        0.0
    };

    OptimizationImprovement {
        cost_reduction_percentage: cost_improvement * 100.0,
        cardinality_reduction_percentage: cardinality_improvement * 100.0,
        original_cost,
        optimized_cost,
        original_rows,
        optimized_rows,
    }
}

/// Information about optimization improvements
///
/// **Planned Feature** - Optimization improvement tracking and reporting
/// See ROADMAP.md: "Pattern Optimization System"
/// Target: v0.3.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OptimizationImprovement {
    /// Percentage reduction in execution cost
    pub cost_reduction_percentage: f64,
    /// Percentage reduction in result cardinality  
    pub cardinality_reduction_percentage: f64,
    /// Original execution cost
    pub original_cost: f64,
    /// Optimized execution cost
    pub optimized_cost: f64,
    /// Original result row count
    pub original_rows: usize,
    /// Optimized result row count
    pub optimized_rows: usize,
}

impl OptimizationImprovement {
    /// Check if the optimization provides significant improvement
    pub fn is_significant_improvement(&self, threshold: f64) -> bool {
        self.cost_reduction_percentage >= threshold
            || self.cardinality_reduction_percentage >= threshold
    }

    /// Get a human-readable description of the improvement
    pub fn describe(&self) -> String {
        if self.cost_reduction_percentage > 0.0 || self.cardinality_reduction_percentage > 0.0 {
            format!(
                "Optimization reduces cost by {:.1}% ({:.2} → {:.2}) and cardinality by {:.1}% ({} → {} rows)",
                self.cost_reduction_percentage, self.original_cost, self.optimized_cost,
                self.cardinality_reduction_percentage, self.original_rows, self.optimized_rows
            )
        } else {
            "No significant improvement from optimization".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physical_generator_creation() {
        let generator = PhysicalPatternPlanGenerator::new();
        assert!(generator.config.enable_aggressive_optimization);
        assert!(generator.config.prefer_indexed_access);
    }

    #[test]
    fn test_physical_generator_custom_config() {
        let config = PhysicalGenerationConfig {
            enable_aggressive_optimization: false,
            prefer_indexed_access: false,
            max_traversal_depth: 3,
            enable_parallel_joins: true,
        };

        let generator = PhysicalPatternPlanGenerator::with_config(config.clone());
        assert!(!generator.config.enable_aggressive_optimization);
        assert!(!generator.config.prefer_indexed_access);
        assert_eq!(generator.config.max_traversal_depth, 3);
        assert!(generator.config.enable_parallel_joins);
    }

    #[test]
    fn test_optimization_improvement_calculation() {
        // Create mock physical nodes for testing
        let original_plan = PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 1000,
            estimated_cost: 500.0,
        };

        let optimized_plan = PhysicalNode::NodeSeqScan {
            variable: "n".to_string(),
            labels: vec!["Person".to_string()],
            properties: None,
            estimated_rows: 100,
            estimated_cost: 50.0,
        };

        let improvement = estimate_optimization_improvement(&original_plan, &optimized_plan);

        assert!((improvement.cost_reduction_percentage - 90.0).abs() < 0.1);
        assert!((improvement.cardinality_reduction_percentage - 90.0).abs() < 0.1);
        assert!(improvement.is_significant_improvement(50.0));
    }

    #[test]
    fn test_optimization_improvement_description() {
        let improvement = OptimizationImprovement {
            cost_reduction_percentage: 85.5,
            cardinality_reduction_percentage: 83.3,
            original_cost: 1000.0,
            optimized_cost: 145.0,
            original_rows: 18,
            optimized_rows: 3,
        };

        let description = improvement.describe();
        assert!(description.contains("85.5%"));
        assert!(description.contains("83.3%"));
        assert!(description.contains("18 → 3 rows"));
    }
}
