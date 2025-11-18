// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Type definitions for the indexing system

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use crate::storage::Value;

/// Unique identifier for an index
#[allow(dead_code)] // ROADMAP v0.4.0 - Index identifier type for metadata management
pub type IndexId = String;

/// Partition identifier for distributed indexes
#[allow(dead_code)] // ROADMAP v0.4.0 - Partition identifier for distributed indexing
pub type PartitionId = String;

/// Node identifier in graph indexes
#[allow(dead_code)] // ROADMAP v0.4.0 - Node identifier for graph index operations
pub type NodeId = String;

/// Index type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IndexType {
    Graph(GraphIndexType),
}

impl IndexType {
    /// Get the prefix for storage column families
    pub fn prefix(&self) -> &'static str {
        match self {
            IndexType::Graph(_) => "graph",
        }
    }
}

/// Graph index types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphIndexType {
    /// Adjacency list for neighbor traversal
    AdjacencyList,
    /// Pre-computed paths up to k-hops
    PathIndex,
    /// Reachability queries
    ReachabilityIndex,
    /// Subgraph patterns
    PatternIndex,
}

/// Index configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Index-specific parameters
    pub parameters: HashMap<String, Value>,

    /// Maximum memory usage in bytes
    pub max_memory_bytes: Option<usize>,

    /// Enable compression
    pub compression_enabled: bool,

    /// Partition strategy
    pub partition_strategy: Option<PartitionStrategy>,

    /// Maintenance schedule
    pub maintenance_interval_seconds: Option<u64>,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            parameters: HashMap::new(),
            max_memory_bytes: None,
            compression_enabled: false,
            partition_strategy: None,
            maintenance_interval_seconds: Some(3600), // 1 hour
        }
    }
}

impl IndexConfig {
    /// Create a new config with specific parameters
    pub fn with_parameters(parameters: HashMap<String, Value>) -> Self {
        Self {
            parameters,
            ..Default::default()
        }
    }

    /// Set memory limit
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.max_memory_bytes = Some(bytes);
        self
    }

    /// Enable compression
    pub fn with_compression(mut self) -> Self {
        self.compression_enabled = true;
        self
    }

    /// Get a parameter value
    pub fn get_parameter(&self, key: &str) -> Option<&Value> {
        self.parameters.get(key)
    }

    /// Get a parameter as integer
    pub fn get_int_parameter(&self, key: &str) -> Option<i64> {
        self.get_parameter(key).and_then(|v| match v {
            Value::Number(n) => Some(*n as i64),
            _ => None,
        })
    }

    /// Get a parameter as float
    pub fn get_float_parameter(&self, key: &str) -> Option<f64> {
        self.get_parameter(key).and_then(|v| match v {
            Value::Number(f) => Some(*f),
            _ => None,
        })
    }

    /// Get a parameter as string
    pub fn get_string_parameter(&self, key: &str) -> Option<&str> {
        self.get_parameter(key).and_then(|v| match v {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        })
    }
}

/// Partition strategy for distributed indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionStrategy {
    /// Hash-based partitioning
    Hash { num_partitions: usize },
    /// Range-based partitioning
    Range { boundaries: Vec<String> },
    /// Geographic partitioning
    Geographic { regions: Vec<String> },
}

/// Index statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatistics {
    /// Number of entries in the index
    pub entry_count: usize,

    /// Size of index in bytes
    pub size_bytes: usize,

    /// Memory usage in bytes
    pub memory_bytes: usize,

    /// Last update timestamp
    pub last_updated: DateTime<Utc>,

    /// Total number of queries
    pub query_count: u64,

    /// Average query time in milliseconds
    pub avg_query_time_ms: f64,

    /// 99th percentile query time
    pub p99_query_time_ms: f64,

    /// Cache hit rate (0.0 to 1.0)
    pub cache_hit_rate: f32,

    /// Index fragmentation (0.0 to 1.0)
    pub fragmentation: f32,

    /// Last maintenance timestamp
    pub last_maintenance: Option<DateTime<Utc>>,
}

impl Default for IndexStatistics {
    fn default() -> Self {
        Self {
            entry_count: 0,
            size_bytes: 0,
            memory_bytes: 0,
            last_updated: Utc::now(),
            query_count: 0,
            avg_query_time_ms: 0.0,
            p99_query_time_ms: 0.0,
            cache_hit_rate: 0.0,
            fragmentation: 0.0,
            last_maintenance: None,
        }
    }
}

impl IndexStatistics {
    /// Update query statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Query performance tracking for index monitoring
    pub fn record_query(&mut self, duration_ms: f64) {
        self.query_count += 1;

        // Simple moving average
        let weight = 1.0 / self.query_count as f64;
        self.avg_query_time_ms = self.avg_query_time_ms * (1.0 - weight) + duration_ms * weight;

        // Update p99 (simplified)
        if duration_ms > self.p99_query_time_ms {
            self.p99_query_time_ms = duration_ms;
        }
    }

    /// Update cache statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index cache hit recording (see ROADMAP.md §6)
    pub fn record_cache_hit(&mut self, hit: bool) {
        // Simple moving average for hit rate
        let weight = 0.01; // More stable than 1/n
        if hit {
            self.cache_hit_rate = self.cache_hit_rate * (1.0 - weight) + weight;
        } else {
            self.cache_hit_rate = self.cache_hit_rate * (1.0 - weight);
        }
    }

    /// Calculate queries per second
    #[allow(dead_code)] // ROADMAP v0.4.0 - QPS calculation for index performance metrics
    pub fn queries_per_second(&self) -> f64 {
        if self.avg_query_time_ms > 0.0 {
            1000.0 / self.avg_query_time_ms
        } else {
            0.0
        }
    }
}

/// Search query for any index type
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Unified search query interface for all index types
pub enum SearchQuery {
    /// Graph traversal query
    Graph(GraphQuery),
}

/// Graph query parameters
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Graph traversal query parameters for graph indexes
pub struct GraphQuery {
    /// Starting node(s)
    pub start_nodes: Vec<NodeId>,
    /// Maximum traversal depth
    pub max_depth: Option<usize>,
    /// Edge direction
    pub direction: Direction,
    /// Optional filter predicate
    pub filter: Option<String>,
}

/// Edge direction for graph traversals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}

/// Search result from any index
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Unified search result for index query responses
pub struct SearchResult {
    /// Result identifier
    pub id: String,
    /// Relevance score
    pub score: f32,
    /// Result metadata
    pub metadata: Option<HashMap<String, Value>>,
    /// Index type that produced this result
    pub source_index: IndexType,
}

/// Metadata filter for search operations
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Index metadata filtering (see ROADMAP.md §6)
pub struct MetadataFilter {
    /// Field name
    pub field: String,
    /// Filter operation
    pub operation: FilterOperation,
    /// Filter value
    pub value: Value,
}

/// Filter operations
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Filter operations for index queries (see ROADMAP.md §6)
pub enum FilterOperation {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    Contains,
    StartsWith,
    EndsWith,
    In(Vec<Value>),
    NotIn(Vec<Value>),
}
