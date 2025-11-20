// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Cost estimation and statistics for query planning
//!
//! This module provides cost models and statistics collection for optimizing
//! query execution plans based on data distribution and operator performance.

use crate::plan::physical::PhysicalNode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cost estimate for a query plan or operator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    /// CPU cost (processing cycles)
    pub cpu_cost: f64,
    /// I/O cost (disk reads/writes)
    pub io_cost: f64,
    /// Memory cost (bytes)
    pub memory_cost: f64,
    /// Network cost (bytes transferred)
    pub network_cost: f64,
    /// Total estimated execution time (seconds)
    pub total_time: f64,
}

/// Cost model for estimating query execution costs
#[derive(Debug, Clone)]
pub struct CostModel {
    /// CPU cost per row processed
    pub cpu_cost_per_row: f64,
    /// I/O cost per page read
    pub io_cost_per_page: f64,
    /// Memory cost per byte
    pub memory_cost_per_byte: f64,
}

/// Statistics about data distribution and access patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    /// Total number of nodes in the graph
    pub total_nodes: usize,
    /// Total number of edges in the graph
    pub total_edges: usize,
    /// Node count by label
    pub node_counts: HashMap<String, usize>,
    /// Edge count by label
    pub edge_counts: HashMap<String, usize>,
    /// Average degree (edges per node)
    pub average_degree: f64,
    /// Maximum degree
    pub max_degree: usize,
    /// Property selectivity estimates
    pub property_selectivity: HashMap<String, f64>,
    /// Index availability
    pub available_indices: Vec<IndexInfo>,
}

/// Information about available indices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub index_type: IndexType,
    pub entity_type: EntityType,
    pub properties: Vec<String>,
    pub cardinality: usize,
}

/// Types of indices available
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexType {
    Hash,
    BTree,
    Label,
    Property,
    Composite,
    // Full-text search indexes
    TextInverted, // Inverted index for full-text search
    TextBM25,     // BM25-scored inverted index
    TextNGram,    // N-gram index for fuzzy matching
}

/// Entity type for indices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Node,
    Edge,
}

impl CostEstimate {
    /// Create a new cost estimate
    pub fn new() -> Self {
        Self {
            cpu_cost: 0.0,
            io_cost: 0.0,
            memory_cost: 0.0,
            network_cost: 0.0,
            total_time: 0.0,
        }
    }

    /// Add another cost estimate to this one
    pub fn add(&mut self, other: &CostEstimate) {
        self.cpu_cost += other.cpu_cost;
        self.io_cost += other.io_cost;
        self.memory_cost += other.memory_cost;
        self.network_cost += other.network_cost;
        self.total_time += other.total_time;
    }

    /// Calculate total cost as weighted sum
    pub fn total_cost(&self) -> f64 {
        // Weights for different cost components
        let cpu_weight = 1.0;
        let io_weight = 10.0; // I/O is expensive
        let memory_weight = 0.1;
        let network_weight = 5.0;

        self.cpu_cost * cpu_weight
            + self.io_cost * io_weight
            + self.memory_cost * memory_weight
            + self.network_cost * network_weight
    }
}

impl Default for CostEstimate {
    fn default() -> Self {
        Self::new()
    }
}

impl CostModel {
    /// Create a new cost model with default values
    pub fn new() -> Self {
        Self {
            cpu_cost_per_row: 0.001,        // 1ms per 1000 rows
            io_cost_per_page: 0.01,         // 10ms per page
            memory_cost_per_byte: 0.000001, // Very cheap
        }
    }

    /// Estimate cost for a physical node
    pub fn estimate_node_cost(&self, node: &PhysicalNode, stats: &Statistics) -> CostEstimate {
        match node {
            PhysicalNode::NodeSeqScan {
                labels,
                estimated_rows,
                ..
            } => self.estimate_scan_cost(*estimated_rows, labels, stats, true),

            PhysicalNode::NodeIndexScan {
                labels,
                estimated_rows,
                ..
            } => self.estimate_scan_cost(*estimated_rows, labels, stats, false),

            PhysicalNode::EdgeSeqScan {
                labels,
                estimated_rows,
                ..
            } => self.estimate_scan_cost(*estimated_rows, labels, stats, true),

            PhysicalNode::IndexedExpand {
                input,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let expand_cost = self.estimate_expand_cost(*estimated_rows, false);
                cost.add(&expand_cost);
                cost
            }

            PhysicalNode::HashExpand {
                input,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let expand_cost = self.estimate_expand_cost(*estimated_rows, true);
                cost.add(&expand_cost);
                cost
            }

            PhysicalNode::Filter {
                input, selectivity, ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let filter_cost = self.estimate_filter_cost(input.get_row_count(), *selectivity);
                cost.add(&filter_cost);
                cost
            }

            PhysicalNode::Project {
                input,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let project_cost = self.estimate_project_cost(*estimated_rows);
                cost.add(&project_cost);
                cost
            }

            PhysicalNode::HashJoin {
                build,
                probe,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(build, stats);
                cost.add(&self.estimate_node_cost(probe, stats));
                let join_cost = self.estimate_join_cost(
                    build.get_row_count(),
                    probe.get_row_count(),
                    *estimated_rows,
                    JoinAlgorithm::Hash,
                );
                cost.add(&join_cost);
                cost
            }

            PhysicalNode::NestedLoopJoin {
                left,
                right,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(left, stats);
                cost.add(&self.estimate_node_cost(right, stats));
                let join_cost = self.estimate_join_cost(
                    left.get_row_count(),
                    right.get_row_count(),
                    *estimated_rows,
                    JoinAlgorithm::NestedLoop,
                );
                cost.add(&join_cost);
                cost
            }

            PhysicalNode::SortMergeJoin {
                left,
                right,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(left, stats);
                cost.add(&self.estimate_node_cost(right, stats));
                let join_cost = self.estimate_join_cost(
                    left.get_row_count(),
                    right.get_row_count(),
                    *estimated_rows,
                    JoinAlgorithm::SortMerge,
                );
                cost.add(&join_cost);
                cost
            }

            PhysicalNode::ExternalSort {
                input,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let sort_cost = self.estimate_sort_cost(*estimated_rows, true);
                cost.add(&sort_cost);
                cost
            }

            PhysicalNode::InMemorySort {
                input,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(input, stats);
                let sort_cost = self.estimate_sort_cost(*estimated_rows, false);
                cost.add(&sort_cost);
                cost
            }

            PhysicalNode::Limit { input, count, .. } => {
                // Limit can terminate early, so cost is proportional to limit
                let input_cost = self.estimate_node_cost(input, stats);
                let input_rows = input.get_row_count();
                let ratio = (*count as f64) / (input_rows as f64).max(1.0);

                CostEstimate {
                    cpu_cost: input_cost.cpu_cost * ratio,
                    io_cost: input_cost.io_cost * ratio,
                    memory_cost: input_cost.memory_cost,
                    network_cost: input_cost.network_cost * ratio,
                    total_time: input_cost.total_time * ratio,
                }
            }

            // Graph index scan - very efficient for graph operations
            PhysicalNode::GraphIndexScan { estimated_rows, .. } => {
                let base_cost = *estimated_rows as f64 * self.cpu_cost_per_row * 0.05; // Highly optimized
                let io_cost = (*estimated_rows / 10000) as f64 * self.io_cost_per_page * 0.1; // Minimal I/O

                CostEstimate {
                    cpu_cost: base_cost,
                    io_cost,
                    memory_cost: *estimated_rows as f64 * 100.0 * self.memory_cost_per_byte, // Graph cache
                    network_cost: 0.0,
                    total_time: base_cost + io_cost,
                }
            }

            // Index join - better than hash join for selective queries
            PhysicalNode::IndexJoin {
                left,
                right,
                estimated_rows,
                ..
            } => {
                let mut cost = self.estimate_node_cost(left, stats);
                cost.add(&self.estimate_node_cost(right, stats));

                let join_cost = self.estimate_join_cost(
                    left.get_row_count(),
                    right.get_row_count(),
                    *estimated_rows,
                    JoinAlgorithm::IndexNL, // Index nested loop
                );
                cost.add(&join_cost);
                cost
            }

            PhysicalNode::SingleRow { .. } => {
                // SingleRow is the cheapest possible operation
                CostEstimate {
                    cpu_cost: 0.0001,    // Minimal CPU - just creates one empty row
                    io_cost: 0.0,        // No I/O needed
                    memory_cost: 0.0001, // Tiny memory for one row
                    network_cost: 0.0,   // No network
                    total_time: 0.0001,  // Near-instant execution
                }
            }

            _ => CostEstimate::new(), // Default for unimplemented nodes
        }
    }

    /// Estimate scan cost
    fn estimate_scan_cost(
        &self,
        rows: usize,
        _labels: &[String],
        _stats: &Statistics,
        is_sequential: bool,
    ) -> CostEstimate {
        let base_cpu_cost = rows as f64 * self.cpu_cost_per_row;
        let cpu_multiplier = if is_sequential { 1.0 } else { 0.3 }; // Index scan is faster

        // Estimate I/O based on whether we have indices
        let io_cost = if is_sequential {
            // Sequential scan reads all data
            (rows / 1000) as f64 * self.io_cost_per_page // Assume 1000 rows per page
        } else {
            // Index scan is more selective
            (rows / 10000) as f64 * self.io_cost_per_page
        };

        CostEstimate {
            cpu_cost: base_cpu_cost * cpu_multiplier,
            io_cost,
            memory_cost: (rows * 100) as f64 * self.memory_cost_per_byte, // 100 bytes per row
            network_cost: 0.0,
            total_time: base_cpu_cost * cpu_multiplier + io_cost,
        }
    }

    /// Estimate expand (traversal) cost
    fn estimate_expand_cost(&self, rows: usize, use_hash: bool) -> CostEstimate {
        let base_cost = rows as f64 * self.cpu_cost_per_row * 2.0; // Traversal is more expensive
        let memory_multiplier = if use_hash { 2.0 } else { 1.0 }; // Hash tables use more memory

        CostEstimate {
            cpu_cost: base_cost,
            io_cost: (rows / 5000) as f64 * self.io_cost_per_page, // Less I/O than scan
            memory_cost: (rows * 50) as f64 * self.memory_cost_per_byte * memory_multiplier,
            network_cost: 0.0,
            total_time: base_cost,
        }
    }

    /// Estimate filter cost
    fn estimate_filter_cost(&self, input_rows: usize, _selectivity: f64) -> CostEstimate {
        let cpu_cost = input_rows as f64 * self.cpu_cost_per_row * 0.5; // Filtering is cheap

        CostEstimate {
            cpu_cost,
            io_cost: 0.0,     // No additional I/O
            memory_cost: 0.0, // No additional memory
            network_cost: 0.0,
            total_time: cpu_cost,
        }
    }

    /// Estimate projection cost
    fn estimate_project_cost(&self, rows: usize) -> CostEstimate {
        let cpu_cost = rows as f64 * self.cpu_cost_per_row * 0.2; // Very cheap

        CostEstimate {
            cpu_cost,
            io_cost: 0.0,
            memory_cost: 0.0,
            network_cost: 0.0,
            total_time: cpu_cost,
        }
    }

    /// Estimate join cost
    fn estimate_join_cost(
        &self,
        left_rows: usize,
        right_rows: usize,
        _output_rows: usize,
        algorithm: JoinAlgorithm,
    ) -> CostEstimate {
        let (cpu_multiplier, memory_multiplier) = match algorithm {
            JoinAlgorithm::Hash => (1.5, 2.0), // Hash join: build hash table, probe
            JoinAlgorithm::NestedLoop => (left_rows as f64, 0.1), // Nested loop: O(n*m)
            JoinAlgorithm::SortMerge => {
                ((left_rows as f64).log2() + (right_rows as f64).log2(), 1.0)
            } // Sort merge: sort both sides
            JoinAlgorithm::IndexNL => (0.8, 0.5), // Index nested loop: much better than regular NL
        };

        let base_cost = (left_rows + right_rows) as f64 * self.cpu_cost_per_row;

        CostEstimate {
            cpu_cost: base_cost * cpu_multiplier,
            io_cost: ((left_rows + right_rows) / 1000) as f64 * self.io_cost_per_page,
            memory_cost: (left_rows.max(right_rows) * 100) as f64
                * self.memory_cost_per_byte
                * memory_multiplier,
            network_cost: 0.0,
            total_time: base_cost * cpu_multiplier,
        }
    }

    /// Estimate sort cost
    fn estimate_sort_cost(&self, rows: usize, external: bool) -> CostEstimate {
        let n_log_n = rows as f64 * (rows as f64).log2();
        let cpu_cost = n_log_n * self.cpu_cost_per_row * 0.01; // Sort constant

        let (io_multiplier, memory_multiplier) = if external {
            (3.0, 0.5) // External sort: multiple I/O passes, less memory
        } else {
            (0.0, 2.0) // In-memory sort: no extra I/O, more memory
        };

        CostEstimate {
            cpu_cost,
            io_cost: (rows / 1000) as f64 * self.io_cost_per_page * io_multiplier,
            memory_cost: (rows * 100) as f64 * self.memory_cost_per_byte * memory_multiplier,
            network_cost: 0.0,
            total_time: cpu_cost,
        }
    }
}

/// Join algorithm types for cost estimation
#[derive(Debug, Clone)]
enum JoinAlgorithm {
    Hash,
    NestedLoop,
    SortMerge,
    IndexNL, // Index Nested Loop
}

impl Default for CostModel {
    fn default() -> Self {
        Self::new()
    }
}

impl Statistics {
    /// Create new statistics from graph data
    pub fn new() -> Self {
        Self {
            total_nodes: 0,
            total_edges: 0,
            node_counts: HashMap::new(),
            edge_counts: HashMap::new(),
            average_degree: 0.0,
            max_degree: 0,
            property_selectivity: HashMap::new(),
            available_indices: Vec::new(),
        }
    }

    /// Update statistics with graph information
    #[allow(dead_code)] // ROADMAP v0.5.0 - Dynamic statistics collection from graph storage
    pub fn update_from_graph(&mut self, graph: &crate::storage::GraphCache) {
        // Get basic counts from graph stats
        let stats = graph.stats();
        self.total_nodes = stats.node_count;
        self.total_edges = stats.edge_count;

        // Calculate average degree
        self.average_degree = if self.total_nodes > 0 {
            (2 * self.total_edges) as f64 / self.total_nodes as f64
        } else {
            0.0
        };

        // Add default label indices (these would be available in the storage)
        self.available_indices.push(IndexInfo {
            name: "node_labels".to_string(),
            index_type: IndexType::Label,
            entity_type: EntityType::Node,
            properties: vec![],
            cardinality: self.total_nodes,
        });

        self.available_indices.push(IndexInfo {
            name: "edge_labels".to_string(),
            index_type: IndexType::Label,
            entity_type: EntityType::Edge,
            properties: vec![],
            cardinality: self.total_edges,
        });

        // Set default property selectivity estimates
        self.property_selectivity.insert("id".to_string(), 1.0); // Unique
        self.property_selectivity.insert("label".to_string(), 0.1); // 10% selectivity
        self.property_selectivity
            .insert("risk_score".to_string(), 0.5); // 50% selectivity
        self.property_selectivity.insert("amount".to_string(), 0.3); // 30% selectivity
    }

    /// Get selectivity for a property
    #[allow(dead_code)] // ROADMAP v0.5.0 - Property selectivity for cardinality estimation
    pub fn get_property_selectivity(&self, property: &str) -> f64 {
        self.property_selectivity
            .get(property)
            .copied()
            .unwrap_or(0.5)
    }

    /// Check if an index is available for given properties
    #[allow(dead_code)] // ROADMAP v0.5.0 - Index availability checking for plan optimization
    pub fn has_index(&self, entity_type: &EntityType, properties: &[String]) -> bool {
        self.available_indices.iter().any(|index| {
            matches!(&index.entity_type, et if std::mem::discriminant(et) == std::mem::discriminant(entity_type)) &&
            properties.iter().all(|prop| index.properties.contains(prop))
        })
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Self::new()
    }
}
