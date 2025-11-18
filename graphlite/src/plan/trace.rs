// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query plan tracing and explanation
//!
//! This module provides tracing capabilities to capture the query planning
//! process for debugging and explanation purposes.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use crate::plan::logical::{LogicalPlan, LogicalNode};
use crate::plan::physical::{PhysicalPlan, PhysicalNode};
use crate::plan::cost::CostEstimate;

/// Trace information for query planning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTrace {
    pub steps: Vec<TraceStep>,
    pub total_duration: Duration,
    pub logical_plan: LogicalPlan,
    pub physical_plan: PhysicalPlan,
}

/// Individual step in the planning process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub phase: PlanningPhase,
    pub description: String,
    pub duration: Duration,
    pub input_plan: Option<PlanSnapshot>,
    pub output_plan: Option<PlanSnapshot>,
    pub cost_estimate: Option<CostEstimate>,
    pub metadata: TraceMetadata,
}

/// Planning phases for tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanningPhase {
    Parsing,
    LogicalPlanGeneration,
    LogicalOptimization,
    PhysicalPlanGeneration,
    PhysicalOptimization,
    CostEstimation,
}

/// Snapshot of a plan at a particular step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlanSnapshot {
    Logical(LogicalNode),
    Physical(PhysicalNode),
}

/// Additional metadata for trace steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadata {
    pub optimization_applied: Option<String>,
    pub rule_name: Option<String>,
    pub variables_in_scope: Vec<String>,
    pub estimated_rows: Option<usize>,
    pub estimated_cost: Option<f64>,
}

/// Builder for creating plan traces
pub struct PlanTracer {
    steps: Vec<TraceStep>,
    start_time: Instant,
    current_step_start: Option<Instant>,
}

impl PlanTracer {
    /// Create a new plan tracer
    pub fn new() -> Self {
        Self {
            steps: Vec::new(),
            start_time: Instant::now(),
            current_step_start: None,
        }
    }

    /// Start tracing a new planning step
    pub fn start_step(&mut self, _phase: PlanningPhase, _description: String) {
        self.current_step_start = Some(Instant::now());
    }

    /// End the current step and record it
    pub fn end_step(
        &mut self,
        phase: PlanningPhase,
        description: String,
        input_plan: Option<PlanSnapshot>,
        output_plan: Option<PlanSnapshot>,
        cost_estimate: Option<CostEstimate>,
        metadata: TraceMetadata,
    ) {
        let duration = self.current_step_start
            .map(|start| start.elapsed())
            .unwrap_or_else(|| Duration::from_millis(0));

        let step = TraceStep {
            phase,
            description,
            duration,
            input_plan,
            output_plan,
            cost_estimate,
            metadata,
        };

        self.steps.push(step);
        self.current_step_start = None;
    }

    /// Finalize the trace with the final plans
    pub fn finalize(
        self,
        logical_plan: LogicalPlan,
        physical_plan: PhysicalPlan,
    ) -> PlanTrace {
        PlanTrace {
            steps: self.steps,
            total_duration: self.start_time.elapsed(),
            logical_plan,
            physical_plan,
        }
    }

    /// Trace a simple step with timing
    pub fn trace_step(
        &mut self,
        phase: PlanningPhase,
        description: String,
        metadata: TraceMetadata,
    ) {
        self.start_step(phase.clone(), description.clone());
        self.end_step(phase, description, None, None, None, metadata);
    }
}

impl PlanTrace {
    /// Format the trace in a graph-optimized format
    pub fn format_graph_plan(&self) -> String {
        let mut output = String::new();
        
        // Header with summary
        output.push_str("Query Plan Summary\n");
        output.push_str(&"=".repeat(50));
        output.push('\n');
        output.push_str(&format!("Total Cost: {:.1} | Estimated Rows: {} | Planning Time: {:.1}ms\n\n",
            self.physical_plan.estimated_cost,
            self.physical_plan.estimated_rows,
            self.total_duration.as_secs_f64() * 1000.0
        ));
        
        // Execution plan tree
        output.push_str("Execution Plan\n");
        output.push_str(&"=".repeat(50));
        output.push('\n');
        
        self.format_plan_node(&self.physical_plan.root, &mut output, 0, true);
        
        // Planning steps summary
        if !self.steps.is_empty() {
            output.push_str("\nPlanning Steps\n");
            output.push_str(&"-".repeat(30));
            output.push('\n');
            for (i, step) in self.steps.iter().enumerate() {
                output.push_str(&format!("{}. {} ({:.1}ms)\n", 
                    i + 1, step.description, step.duration.as_secs_f64() * 1000.0));
            }
        }
        
        output
    }

    /// Format a single plan node with tree structure
    fn format_plan_node(&self, node: &PhysicalNode, output: &mut String, depth: usize, is_last: bool) {
        // Tree connector
        let prefix = if depth == 0 {
            "".to_string()
        } else {
            let mut p = String::new();
            for _i in 0..depth-1 {
                p.push_str("│   ");
            }
            if is_last {
                p.push_str("└── ");
            } else {
                p.push_str("├── ");
            }
            p
        };

        match node {
            PhysicalNode::NodeSeqScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NodeScan[{}:{}] → {} rows, cost: {:.1}\n",
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Variables: {}\n", " ".repeat(prefix.len()), variable));
                if !labels.is_empty() {
                    output.push_str(&format!("{}    Labels: {}\n", " ".repeat(prefix.len()), labels.join(", ")));
                }
            }
            PhysicalNode::NodeIndexScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NodeIndexScan[{}:{}] → {} rows, cost: {:.1}\n",
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Variables: {}\n", " ".repeat(prefix.len()), variable));
                output.push_str(&format!("{}    Index: {} labels\n", " ".repeat(prefix.len()), labels.len()));
            }
            PhysicalNode::EdgeSeqScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}EdgeScan[{}:{}] → {} rows, cost: {:.1}\n",
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Variables: {}\n", " ".repeat(prefix.len()), variable));
            }
            PhysicalNode::IndexedExpand { from_variable, to_variable, edge_labels, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}IndexedExpand[{} → {}:{}] → {} rows, cost: {:.1}\n",
                    prefix, from_variable, to_variable, edge_labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Expand: {} → {} via {}\n", 
                    " ".repeat(prefix.len()), from_variable, to_variable, edge_labels.join(", ")));
                output.push_str(&format!("{}    Method: Index lookup\n", " ".repeat(prefix.len())));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::HashExpand { from_variable, to_variable, edge_labels, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}HashExpand[{} → {}:{}] → {} rows, cost: {:.1}\n",
                    prefix, from_variable, to_variable, edge_labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Expand: {} → {} via {}\n", 
                    " ".repeat(prefix.len()), from_variable, to_variable, edge_labels.join(", ")));
                output.push_str(&format!("{}    Method: Hash join\n", " ".repeat(prefix.len())));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::Filter { condition, input, selectivity, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Filter[{:?}] → {} rows, cost: {:.1}\n",
                    prefix, condition, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Selectivity: {:.1}% ({:.3})\n", 
                    " ".repeat(prefix.len()), selectivity * 100.0, selectivity));
                output.push_str(&format!("{}    Condition: {:?}\n", " ".repeat(prefix.len()), condition));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::Project { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Project[{} columns] → {} rows, cost: {:.1}\n",
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Output: {} expressions\n", " ".repeat(prefix.len()), expressions.len()));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::InMemorySort { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Sort[{} columns] → {} rows, cost: {:.1}\n",
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Method: In-memory\n", " ".repeat(prefix.len())));
                output.push_str(&format!("{}    Columns: {}\n", " ".repeat(prefix.len()), expressions.len()));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::ExternalSort { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Sort[{} columns] → {} rows, cost: {:.1}\n",
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Method: External (disk-based)\n", " ".repeat(prefix.len())));
                output.push_str(&format!("{}    Columns: {}\n", " ".repeat(prefix.len()), expressions.len()));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::Limit { count, offset, input, estimated_rows, estimated_cost, .. } => {
                let offset_str = offset.map(|o| format!(" OFFSET {}", o)).unwrap_or_default();
                output.push_str(&format!("{}Limit[{}{}] → {} rows, cost: {:.1}\n",
                    prefix, count, offset_str, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Limit: {}{}\n", " ".repeat(prefix.len()), count, offset_str));
                self.format_plan_node(input, output, depth + 1, true);
            }
            PhysicalNode::ExistsSubquery { subplan, optimized, estimated_rows, estimated_cost, .. } => {
                let opt_str = if *optimized { " (early-term)" } else { "" };
                output.push_str(&format!("{}EXISTS{} → {} rows, cost: {:.1}\n",
                    prefix, opt_str, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Optimization: {}\n", 
                    " ".repeat(prefix.len()), if *optimized { "Early termination" } else { "Full evaluation" }));
                self.format_plan_node(subplan, output, depth + 1, true);
            }
            PhysicalNode::NotExistsSubquery { subplan, optimized, estimated_rows, estimated_cost, .. } => {
                let opt_str = if *optimized { " (early-term)" } else { "" };
                output.push_str(&format!("{}NOT EXISTS{} → {} rows, cost: {:.1}\n",
                    prefix, opt_str, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Optimization: {}\n", 
                    " ".repeat(prefix.len()), if *optimized { "Early termination" } else { "Full evaluation" }));
                self.format_plan_node(subplan, output, depth + 1, true);
            }
            PhysicalNode::InSubquery { expression, subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}IN SUBQUERY → {} rows, cost: {:.1}\n",
                    prefix, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Expression: {:?}\n", " ".repeat(prefix.len()), expression));
                self.format_plan_node(subplan, output, depth + 1, true);
            }
            PhysicalNode::NotInSubquery { expression, subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NOT IN SUBQUERY → {} rows, cost: {:.1}\n",
                    prefix, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Expression: {:?}\n", " ".repeat(prefix.len()), expression));
                self.format_plan_node(subplan, output, depth + 1, true);
            }
            PhysicalNode::ScalarSubquery { subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}SCALAR SUBQUERY → {} rows, cost: {:.1}\n",
                    prefix, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Returns: Single value\n", " ".repeat(prefix.len())));
                self.format_plan_node(subplan, output, depth + 1, true);
            }
            PhysicalNode::UnionAll { inputs, all, estimated_rows, estimated_cost, .. } => {
                let op_name = if *all { "UNION ALL" } else { "UNION" };
                output.push_str(&format!("{}{}[{} inputs] → {} rows, cost: {:.1}\n",
                    prefix, op_name, inputs.len(), estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Deduplication: {}\n",
                    " ".repeat(prefix.len()), if *all { "Disabled" } else { "Enabled" }));
                output.push_str(&format!("{}    Inputs:\n", " ".repeat(prefix.len())));
                for (i, input) in inputs.iter().enumerate() {
                    let is_last_input = i == inputs.len() - 1;
                    self.format_plan_node(input, output, depth + 1, is_last_input);
                }
            }
            PhysicalNode::Intersect { left, right, all, estimated_rows, estimated_cost, .. } => {
                let op_name = if *all { "INTERSECT ALL" } else { "INTERSECT" };
                output.push_str(&format!("{}{} → {} rows, cost: {:.1}\n",
                    prefix, op_name, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Deduplication: {}\n",
                    " ".repeat(prefix.len()), if *all { "Disabled" } else { "Enabled" }));
                output.push_str(&format!("{}    Left input:\n", " ".repeat(prefix.len())));
                self.format_plan_node(left, output, depth + 1, false);
                output.push_str(&format!("{}    Right input:\n", " ".repeat(prefix.len())));
                self.format_plan_node(right, output, depth + 1, true);
            }
            PhysicalNode::Except { left, right, all, estimated_rows, estimated_cost, .. } => {
                let op_name = if *all { "EXCEPT ALL" } else { "EXCEPT" };
                output.push_str(&format!("{}{} → {} rows, cost: {:.1}\n",
                    prefix, op_name, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Deduplication: {}\n",
                    " ".repeat(prefix.len()), if *all { "Disabled" } else { "Enabled" }));
                output.push_str(&format!("{}    Left input:\n", " ".repeat(prefix.len())));
                self.format_plan_node(left, output, depth + 1, false);
                output.push_str(&format!("{}    Right input:\n", " ".repeat(prefix.len())));
                self.format_plan_node(right, output, depth + 1, true);
            }
            PhysicalNode::SingleRow { estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}SingleRow[] → {} rows, cost: {:.1}\n",
                    prefix, estimated_rows, estimated_cost));
                output.push_str(&format!("{}    Operation: Produces exactly one empty row\n", " ".repeat(prefix.len())));
            }
            _ => {
                output.push_str(&format!("{}Other[{:?}]\n", prefix, node));
            }
        }
    }

    /// Format the trace for display
    pub fn format_trace(&self) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("Query Plan Trace (total: {:.2}ms)\n", 
            self.total_duration.as_secs_f64() * 1000.0));
        output.push_str(&"=".repeat(50));
        output.push('\n');

        for (i, step) in self.steps.iter().enumerate() {
            output.push_str(&format!(
                "{}. {} ({:.2}ms)\n",
                i + 1,
                step.description,
                step.duration.as_secs_f64() * 1000.0
            ));
            
            output.push_str(&format!("   Phase: {:?}\n", step.phase));
            
            if let Some(cost) = &step.cost_estimate {
                output.push_str(&format!("   Cost: {:.2}\n", cost.total_cost()));
            }
            
            if let Some(rows) = step.metadata.estimated_rows {
                output.push_str(&format!("   Estimated rows: {}\n", rows));
            }
            
            if let Some(optimization) = &step.metadata.optimization_applied {
                output.push_str(&format!("   Optimization: {}\n", optimization));
            }
            
            output.push('\n');
        }

        // Add logical plan
        output.push_str("Logical Plan:\n");
        output.push_str(&"-".repeat(20));
        output.push('\n');
        output.push_str(&self.format_logical_plan(&self.logical_plan.root, 0));
        output.push('\n');

        // Add physical plan
        output.push_str("Physical Plan:\n");
        output.push_str(&"-".repeat(20));
        output.push('\n');
        output.push_str(&self.format_physical_plan(&self.physical_plan.root, 0));
        output.push('\n');

        // Add summary statistics
        output.push_str("Summary:\n");
        output.push_str(&"-".repeat(20));
        output.push('\n');
        output.push_str(&format!("Total estimated cost: {:.2}\n", self.physical_plan.estimated_cost));
        output.push_str(&format!("Estimated rows: {}\n", self.physical_plan.estimated_rows));
        output.push_str(&format!("Planning steps: {}\n", self.steps.len()));

        output
    }

    /// Format logical plan as tree
    fn format_logical_plan(&self, node: &LogicalNode, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        let mut output = String::new();

        match node {
            LogicalNode::NodeScan { variable, labels, .. } => {
                output.push_str(&format!("{}NodeScan({}:{})\n", 
                    prefix, variable, labels.join("|")));
            }
            LogicalNode::EdgeScan { variable, labels, .. } => {
                output.push_str(&format!("{}EdgeScan({}:{})\n", 
                    prefix, variable, labels.join("|")));
            }
            LogicalNode::Expand { from_variable, to_variable, edge_labels, input, .. } => {
                output.push_str(&format!("{}Expand({} -> {}:{})\n", 
                    prefix, from_variable, to_variable, edge_labels.join("|")));
                output.push_str(&self.format_logical_plan(input, indent + 1));
            }
            LogicalNode::Filter { condition, input } => {
                output.push_str(&format!("{}Filter({:?})\n", prefix, condition));
                output.push_str(&self.format_logical_plan(input, indent + 1));
            }
            LogicalNode::Project { expressions, input } => {
                output.push_str(&format!("{}Project({} cols)\n", prefix, expressions.len()));
                output.push_str(&self.format_logical_plan(input, indent + 1));
            }
            LogicalNode::Sort { expressions, input } => {
                output.push_str(&format!("{}Sort({} cols)\n", prefix, expressions.len()));
                output.push_str(&self.format_logical_plan(input, indent + 1));
            }
            LogicalNode::Limit { count, offset, input } => {
                output.push_str(&format!("{}Limit({}, {:?})\n", prefix, count, offset));
                output.push_str(&self.format_logical_plan(input, indent + 1));
            }
            LogicalNode::SingleRow => {
                output.push_str(&format!("{}SingleRow()\n", prefix));
            }
            _ => {
                output.push_str(&format!("{}Other({:?})\n", prefix, node));
            }
        }

        output
    }

    /// Format physical plan as tree
    fn format_physical_plan(&self, node: &PhysicalNode, indent: usize) -> String {
        let prefix = "  ".repeat(indent);
        let mut output = String::new();

        match node {
            PhysicalNode::NodeSeqScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NodeSeqScan({}:{}) [rows={}, cost={:.2}]\n", 
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
            }
            PhysicalNode::NodeIndexScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NodeIndexScan({}:{}) [rows={}, cost={:.2}]\n", 
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
            }
            PhysicalNode::EdgeSeqScan { variable, labels, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}EdgeSeqScan({}:{}) [rows={}, cost={:.2}]\n", 
                    prefix, variable, labels.join("|"), estimated_rows, estimated_cost));
            }
            PhysicalNode::IndexedExpand { from_variable, to_variable, edge_labels, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}IndexedExpand({} -> {}:{}) [rows={}, cost={:.2}]\n", 
                    prefix, from_variable, to_variable, edge_labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::HashExpand { from_variable, to_variable, edge_labels, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}HashExpand({} -> {}:{}) [rows={}, cost={:.2}]\n", 
                    prefix, from_variable, to_variable, edge_labels.join("|"), estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::Filter { input, selectivity, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Filter(sel={:.2}) [rows={}, cost={:.2}]\n", 
                    prefix, selectivity, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::Project { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Project({} cols) [rows={}, cost={:.2}]\n", 
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::InMemorySort { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}InMemorySort({} cols) [rows={}, cost={:.2}]\n", 
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::ExternalSort { expressions, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}ExternalSort({} cols) [rows={}, cost={:.2}]\n", 
                    prefix, expressions.len(), estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::Limit { count, offset, input, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}Limit({}, {:?}) [rows={}, cost={:.2}]\n", 
                    prefix, count, offset, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(input, indent + 1));
            }
            PhysicalNode::ExistsSubquery { subplan, optimized, estimated_rows, estimated_cost, .. } => {
                let opt_str = if *optimized { " (early-term)" } else { "" };
                output.push_str(&format!("{}ExistsSubquery{} [rows={}, cost={:.2}]\n", 
                    prefix, opt_str, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(subplan, indent + 1));
            }
            PhysicalNode::NotExistsSubquery { subplan, optimized, estimated_rows, estimated_cost, .. } => {
                let opt_str = if *optimized { " (early-term)" } else { "" };
                output.push_str(&format!("{}NotExistsSubquery{} [rows={}, cost={:.2}]\n", 
                    prefix, opt_str, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(subplan, indent + 1));
            }
            PhysicalNode::InSubquery { expression, subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}InSubquery({:?}) [rows={}, cost={:.2}]\n", 
                    prefix, expression, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(subplan, indent + 1));
            }
            PhysicalNode::NotInSubquery { expression, subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}NotInSubquery({:?}) [rows={}, cost={:.2}]\n", 
                    prefix, expression, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(subplan, indent + 1));
            }
            PhysicalNode::ScalarSubquery { subplan, estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}ScalarSubquery [rows={}, cost={:.2}]\n", 
                    prefix, estimated_rows, estimated_cost));
                output.push_str(&self.format_physical_plan(subplan, indent + 1));
            }
            PhysicalNode::SingleRow { estimated_rows, estimated_cost, .. } => {
                output.push_str(&format!("{}SingleRow() [rows={}, cost={:.2}]\n", 
                    prefix, estimated_rows, estimated_cost));
            }
            _ => {
                output.push_str(&format!("{}Other({:?})\n", prefix, node));
            }
        }

        output
    }
}

impl Default for PlanTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceMetadata {
    /// Create empty metadata
    pub fn empty() -> Self {
        Self {
            optimization_applied: None,
            rule_name: None,
            variables_in_scope: Vec::new(),
            estimated_rows: None,
            estimated_cost: None,
        }
    }

    /// Create metadata with optimization info
    pub fn with_optimization(optimization: String) -> Self {
        Self {
            optimization_applied: Some(optimization),
            rule_name: None,
            variables_in_scope: Vec::new(),
            estimated_rows: None,
            estimated_cost: None,
        }
    }

    /// Create metadata with cost estimates
    pub fn with_estimates(rows: usize, cost: f64) -> Self {
        Self {
            optimization_applied: None,
            rule_name: None,
            variables_in_scope: Vec::new(),
            estimated_rows: Some(rows),
            estimated_cost: Some(cost),
        }
    }
}
