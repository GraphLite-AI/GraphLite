// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Subquery result caching for nested query optimization

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::{CacheEntryMetadata, CacheKey, CacheLevel, CacheValue};
use crate::exec::{QueryResult, Row};
use crate::storage::Value;

/// Types of subquery results that can be cached
#[derive(Debug, Clone)]
pub enum SubqueryResult {
    /// Boolean result for EXISTS/NOT EXISTS subqueries
    Boolean(bool),
    /// Scalar result for single-value subqueries  
    Scalar(Option<Value>),
    /// Set result for IN/NOT IN subqueries (stores hash set of values for fast lookup)
    Set(Vec<Value>),
    /// Full result set for complex subqueries
    FullResult(QueryResult),
}

impl SubqueryResult {
    /// Check if this result matches a value (for IN/NOT IN operations)
    pub fn contains_value(&self, value: &Value) -> Option<bool> {
        match self {
            SubqueryResult::Set(values) => Some(values.contains(value)),
            SubqueryResult::Boolean(exists) => Some(*exists),
            SubqueryResult::Scalar(Some(scalar_value)) => Some(scalar_value == value),
            SubqueryResult::Scalar(None) => Some(false),
            SubqueryResult::FullResult(result) => {
                // Check if value exists in any row/column of the result
                Some(result.rows.iter().any(|row| {
                    row.positional_values.contains(value) || row.values.values().any(|v| v == value)
                }))
            }
        }
    }

    /// Get boolean result for EXISTS operations
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            SubqueryResult::Boolean(b) => Some(*b),
            SubqueryResult::Scalar(Some(_)) => Some(true),
            SubqueryResult::Scalar(None) => Some(false),
            SubqueryResult::Set(values) => Some(!values.is_empty()),
            SubqueryResult::FullResult(result) => Some(!result.rows.is_empty()),
        }
    }

    /// Get scalar result for single-value subqueries
    pub fn as_scalar(&self) -> Option<Value> {
        match self {
            SubqueryResult::Scalar(value) => value.clone(),
            SubqueryResult::Boolean(b) => Some(Value::Boolean(*b)),
            SubqueryResult::Set(values) => values.first().cloned(),
            SubqueryResult::FullResult(result) => result
                .rows
                .first()
                .and_then(|row| row.positional_values.first().cloned()),
        }
    }
}

impl CacheValue for SubqueryResult {
    fn size_bytes(&self) -> usize {
        match self {
            SubqueryResult::Boolean(_) => std::mem::size_of::<bool>(),
            SubqueryResult::Scalar(Some(value)) => {
                std::mem::size_of::<Value>()
                    + match value {
                        Value::String(s) => s.len(),
                        _ => 0,
                    }
            }
            SubqueryResult::Scalar(None) => std::mem::size_of::<Option<Value>>(),
            SubqueryResult::Set(values) => {
                values.len() * std::mem::size_of::<Value>()
                    + values
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s.len(),
                            _ => 0,
                        })
                        .sum::<usize>()
            }
            SubqueryResult::FullResult(result) => {
                let base_size = std::mem::size_of::<QueryResult>();
                let rows_size = result.rows.len() * std::mem::size_of::<Row>();
                let variables_size = result.variables.iter().map(|var| var.len()).sum::<usize>();
                base_size + rows_size + variables_size
            }
        }
    }

    fn is_valid(&self) -> bool {
        // All subquery results are valid by default
        // Could add additional validation logic here
        true
    }
}

/// Cache key for subquery results
#[derive(Debug, Clone)]
pub struct SubqueryCacheKey {
    /// Hash of the subquery AST structure (normalized)
    pub subquery_hash: u64,
    /// Parameters/variables from the outer query that affect this subquery
    pub outer_variables: Vec<(String, Value)>,
    /// Graph version for invalidation
    pub graph_version: u64,
    /// Schema version for invalidation  
    pub schema_version: u64,
    /// Type of subquery operation
    pub subquery_type: SubqueryType,
}

impl PartialEq for SubqueryCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.subquery_hash == other.subquery_hash
            && self.graph_version == other.graph_version
            && self.schema_version == other.schema_version
            && self.subquery_type == other.subquery_type
            && self.outer_variables.len() == other.outer_variables.len()
            && self
                .outer_variables
                .iter()
                .zip(&other.outer_variables)
                .all(|(a, b)| a.0 == b.0 && values_equal(&a.1, &b.1))
    }
}

impl Eq for SubqueryCacheKey {}

impl Hash for SubqueryCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subquery_hash.hash(state);
        self.graph_version.hash(state);
        self.schema_version.hash(state);
        self.subquery_type.hash(state);

        for (name, value) in &self.outer_variables {
            name.hash(state);
            hash_value(value, state);
        }
    }
}

// Helper function to compare Values (since Value doesn't implement Eq)
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
        (Value::Boolean(a), Value::Boolean(b)) => a == b,
        (Value::Null, Value::Null) => true,
        (Value::DateTime(a), Value::DateTime(b)) => a == b,
        (Value::DateTimeWithFixedOffset(a), Value::DateTimeWithFixedOffset(b)) => a == b,
        (Value::DateTimeWithNamedTz(tz_a, dt_a), Value::DateTimeWithNamedTz(tz_b, dt_b)) => {
            tz_a == tz_b && dt_a == dt_b
        }
        (Value::TimeWindow(a), Value::TimeWindow(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(x, y)| values_equal(x, y))
        }
        (Value::Vector(a), Value::Vector(b)) => a == b,
        (Value::Path(a), Value::Path(b)) => a == b,
        _ => false,
    }
}

// Helper function to hash Values (since Value doesn't implement Hash)
fn hash_value<H: Hasher>(value: &Value, state: &mut H) {
    match value {
        Value::String(s) => {
            0u8.hash(state);
            s.hash(state);
        }
        Value::Number(n) => {
            1u8.hash(state);
            n.to_bits().hash(state);
        }
        Value::Boolean(b) => {
            2u8.hash(state);
            b.hash(state);
        }
        Value::Null => {
            3u8.hash(state);
        }
        Value::DateTime(dt) => {
            4u8.hash(state);
            dt.timestamp().hash(state);
            dt.timestamp_subsec_nanos().hash(state);
        }
        Value::DateTimeWithFixedOffset(dt) => {
            5u8.hash(state);
            dt.timestamp().hash(state);
            dt.timestamp_subsec_nanos().hash(state);
            dt.offset().local_minus_utc().hash(state);
        }
        Value::DateTimeWithNamedTz(tz, dt) => {
            6u8.hash(state);
            tz.hash(state);
            dt.timestamp().hash(state);
            dt.timestamp_subsec_nanos().hash(state);
        }
        Value::TimeWindow(tw) => {
            7u8.hash(state);
            tw.hash(state);
        }
        Value::Array(arr) => {
            8u8.hash(state);
            arr.len().hash(state);
            for item in arr {
                hash_value(item, state);
            }
        }
        Value::Vector(vec) => {
            9u8.hash(state);
            vec.len().hash(state);
            for &val in vec {
                val.to_bits().hash(state);
            }
        }
        Value::Path(path) => {
            10u8.hash(state);
            // PathValue doesn't implement Hash, so hash its string representation
            format!("{:?}", path).hash(state);
        }
        Value::List(list) => {
            11u8.hash(state);
            list.len().hash(state);
            for item in list {
                hash_value(item, state);
            }
        }
        Value::Node(node) => {
            12u8.hash(state);
            node.id.hash(state);
            node.labels.hash(state);
            node.properties.len().hash(state);
            for (key, value) in &node.properties {
                key.hash(state);
                hash_value(value, state);
            }
        }
        Value::Edge(edge) => {
            13u8.hash(state);
            edge.id.hash(state);
            edge.from_node.hash(state);
            edge.to_node.hash(state);
            edge.label.hash(state);
            edge.properties.len().hash(state);
            for (key, value) in &edge.properties {
                key.hash(state);
                hash_value(value, state);
            }
        }
        Value::Temporal(temporal) => {
            14u8.hash(state);
            // Hash the temporal value - we'll hash its debug representation for now
            format!("{:?}", temporal).hash(state);
        }
    }
}

/// Types of subquery operations
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum SubqueryType {
    Exists,
    NotExists,
    In,
    NotIn,
    Scalar,
    Correlated,
}

impl CacheKey for SubqueryCacheKey {
    fn cache_key(&self) -> String {
        format!(
            "subquery:{}:{}:{}:{:?}",
            self.subquery_hash, self.graph_version, self.schema_version, self.subquery_type
        )
    }

    fn tags(&self) -> Vec<String> {
        let mut tags = vec![
            format!("graph_version:{}", self.graph_version),
            format!("schema_version:{}", self.schema_version),
            format!("subquery_type:{:?}", self.subquery_type),
            format!("subquery_hash:{}", self.subquery_hash),
        ];

        // Add tags for outer variables that might affect invalidation
        for (var_name, _) in &self.outer_variables {
            tags.push(format!("outer_var:{}", var_name));
        }

        tags
    }
}

/// Cache entry for subquery results
#[derive(Debug, Clone)]
pub struct SubqueryCacheEntry {
    pub result: SubqueryResult,
    pub execution_time: Duration,
    #[allow(dead_code)]
    // ROADMAP v0.5.0 - Tracks correlation complexity for cache eviction policies. Currently set (line 447) but not yet used in eviction scoring. Will be used for cost-based eviction when correlated subquery optimization is implemented.
    pub outer_variable_count: usize,
    pub metadata: CacheEntryMetadata,
    pub hit_count: u64,
    pub last_hit: Instant,
    pub complexity_score: f64, // Higher score = more expensive to compute
}

impl CacheValue for SubqueryCacheEntry {
    fn size_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.result.size_bytes()
    }

    fn is_valid(&self) -> bool {
        !self.metadata.is_expired() && self.result.is_valid()
    }
}

/// Statistics for subquery cache
#[derive(Debug, Default, Clone)]
pub struct SubqueryCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub total_execution_time_saved_ms: u64,
    pub boolean_cache_hits: u64, // EXISTS/NOT EXISTS
    pub scalar_cache_hits: u64,  // Scalar subqueries
    pub set_cache_hits: u64,     // IN/NOT IN
    pub full_result_cache_hits: u64,
    pub current_entries: usize,
    pub memory_bytes: usize,
    pub invalidations: u64,
}

impl SubqueryCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn average_time_saved_ms(&self) -> f64 {
        if self.hits == 0 {
            0.0
        } else {
            self.total_execution_time_saved_ms as f64 / self.hits as f64
        }
    }
}

/// Subquery result cache implementation
pub struct SubqueryCache {
    entries: Arc<RwLock<HashMap<SubqueryCacheKey, SubqueryCacheEntry>>>,
    max_entries: usize,
    max_memory_bytes: usize,
    current_memory: Arc<RwLock<usize>>,
    stats: Arc<RwLock<SubqueryCacheStats>>,
    default_ttl: Duration,

    // Specialized indices for fast lookups
    boolean_index: Arc<RwLock<HashMap<u64, Vec<SubqueryCacheKey>>>>, // subquery_hash -> keys
    scalar_index: Arc<RwLock<HashMap<u64, Vec<SubqueryCacheKey>>>>,  // subquery_hash -> keys
}

impl SubqueryCache {
    pub fn new(max_entries: usize, max_memory_bytes: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            max_memory_bytes,
            current_memory: Arc::new(RwLock::new(0)),
            stats: Arc::new(RwLock::new(SubqueryCacheStats::default())),
            default_ttl,
            boolean_index: Arc::new(RwLock::new(HashMap::new())),
            scalar_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get cached subquery result
    pub fn get(&self, key: &SubqueryCacheKey) -> Option<SubqueryResult> {
        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_valid() {
                // Update access info
                entry.metadata.update_access();
                entry.hit_count += 1;
                entry.last_hit = Instant::now();

                // Update stats
                {
                    let mut stats = self.stats.write().unwrap();
                    stats.hits += 1;
                    stats.total_execution_time_saved_ms += entry.execution_time.as_millis() as u64;

                    // Track by subquery type
                    match &entry.result {
                        SubqueryResult::Boolean(_) => stats.boolean_cache_hits += 1,
                        SubqueryResult::Scalar(_) => stats.scalar_cache_hits += 1,
                        SubqueryResult::Set(_) => stats.set_cache_hits += 1,
                        SubqueryResult::FullResult(_) => stats.full_result_cache_hits += 1,
                    }
                }

                Some(entry.result.clone())
            } else {
                // Remove expired entry
                let removed_entry = entries.remove(key).unwrap();
                let size = removed_entry.size_bytes();

                {
                    let mut current_memory = self.current_memory.write().unwrap();
                    *current_memory = current_memory.saturating_sub(size);
                }

                {
                    let mut stats = self.stats.write().unwrap();
                    stats.misses += 1;
                    stats.current_entries = entries.len();
                    stats.memory_bytes = *self.current_memory.read().unwrap();
                }

                None
            }
        } else {
            // Cache miss
            let mut stats = self.stats.write().unwrap();
            stats.misses += 1;
            None
        }
    }

    /// Insert subquery result into cache
    pub fn insert(
        &self,
        key: SubqueryCacheKey,
        result: SubqueryResult,
        execution_time: Duration,
        complexity_score: f64,
    ) {
        let entry = SubqueryCacheEntry {
            result: result.clone(),
            execution_time,
            outer_variable_count: key.outer_variables.len(),
            metadata: CacheEntryMetadata::new(0, CacheLevel::L1)
                .with_ttl(self.default_ttl)
                .with_tags(key.tags()),
            hit_count: 0,
            last_hit: Instant::now(),
            complexity_score,
        };

        let size = entry.size_bytes();

        // Evict if necessary
        self.evict_if_needed(size);

        // Update indices
        self.update_indices(&key, true);

        // Insert entry
        {
            let mut entries = self.entries.write().unwrap();
            entries.insert(key, entry);
        }

        {
            let mut current_memory = self.current_memory.write().unwrap();
            *current_memory += size;
        }

        // Update stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.current_entries = self.entries.read().unwrap().len();
            stats.memory_bytes = *self.current_memory.read().unwrap();
        }
    }

    /// Find potential cache hits for EXISTS subqueries
    pub fn find_boolean_matches(&self, subquery_hash: u64) -> Vec<(SubqueryCacheKey, bool)> {
        let mut matches = Vec::new();

        if let Some(keys) = self.boolean_index.read().unwrap().get(&subquery_hash) {
            let entries = self.entries.read().unwrap();

            for key in keys {
                if let Some(entry) = entries.get(key) {
                    if entry.is_valid() {
                        if let Some(boolean_result) = entry.result.as_boolean() {
                            matches.push((key.clone(), boolean_result));
                        }
                    }
                }
            }
        }

        matches
    }

    /// Invalidate entries by graph version
    pub fn invalidate_by_graph_version(&self, version: u64) {
        let mut entries = self.entries.write().unwrap();
        let mut removed_keys = Vec::new();
        let mut removed_size = 0;

        entries.retain(|key, entry| {
            if key.graph_version < version {
                removed_keys.push(key.clone());
                removed_size += entry.size_bytes();
                false
            } else {
                true
            }
        });

        // Update indices
        for key in removed_keys {
            self.update_indices(&key, false);
        }

        {
            let mut current_memory = self.current_memory.write().unwrap();
            *current_memory = current_memory.saturating_sub(removed_size);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.invalidations += 1;
            stats.current_entries = entries.len();
            stats.memory_bytes = *self.current_memory.read().unwrap();
        }
    }

    /// Invalidate entries by schema version
    pub fn invalidate_by_schema_version(&self, version: u64) {
        let mut entries = self.entries.write().unwrap();
        let mut removed_keys = Vec::new();
        let mut removed_size = 0;

        entries.retain(|key, entry| {
            if key.schema_version < version {
                removed_keys.push(key.clone());
                removed_size += entry.size_bytes();
                false
            } else {
                true
            }
        });

        // Update indices
        for key in removed_keys {
            self.update_indices(&key, false);
        }

        {
            let mut current_memory = self.current_memory.write().unwrap();
            *current_memory = current_memory.saturating_sub(removed_size);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.invalidations += 1;
            stats.current_entries = entries.len();
            stats.memory_bytes = *self.current_memory.read().unwrap();
        }
    }

    fn evict_if_needed(&self, incoming_size: usize) {
        let current_memory = *self.current_memory.read().unwrap();
        let current_entries = self.entries.read().unwrap().len();

        if current_memory + incoming_size > self.max_memory_bytes
            || current_entries >= self.max_entries
        {
            // Collect eviction candidates (prioritize by hit rate vs complexity)
            let mut candidates: Vec<(SubqueryCacheKey, f64)> = {
                let entries = self.entries.read().unwrap();
                entries
                    .iter()
                    .map(|(key, entry)| {
                        // Score based on hit rate, recency, and complexity
                        let hit_rate = if entry.metadata.access_count == 0 {
                            0.0
                        } else {
                            entry.hit_count as f64 / entry.metadata.access_count as f64
                        };
                        let recency_score = 1.0 / (1.0 + entry.last_hit.elapsed().as_secs() as f64);
                        let complexity_bonus = entry.complexity_score / 10.0; // Higher complexity = keep longer

                        let keep_score = hit_rate + recency_score + complexity_bonus;
                        (key.clone(), keep_score)
                    })
                    .collect()
            };

            // Sort by keep score (ascending = evict first)
            candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            // Evict lowest scoring entries
            let mut entries = self.entries.write().unwrap();
            for (key, _) in candidates {
                if let Some(evicted_entry) = entries.remove(&key) {
                    let evicted_size = evicted_entry.size_bytes();
                    self.update_indices(&key, false);

                    {
                        let mut current_memory = self.current_memory.write().unwrap();
                        *current_memory = current_memory.saturating_sub(evicted_size);
                    }

                    // Check if we have enough space now
                    let new_current_memory = *self.current_memory.read().unwrap();
                    let new_current_entries = entries.len();

                    if new_current_memory + incoming_size <= self.max_memory_bytes
                        && new_current_entries < self.max_entries
                    {
                        break;
                    }
                }
            }
        }
    }

    fn update_indices(&self, key: &SubqueryCacheKey, add: bool) {
        match key.subquery_type {
            SubqueryType::Exists | SubqueryType::NotExists => {
                let mut boolean_index = self.boolean_index.write().unwrap();
                if add {
                    boolean_index
                        .entry(key.subquery_hash)
                        .or_default()
                        .push(key.clone());
                } else if let Some(keys) = boolean_index.get_mut(&key.subquery_hash) {
                    keys.retain(|k| k != key);
                    if keys.is_empty() {
                        boolean_index.remove(&key.subquery_hash);
                    }
                }
            }
            SubqueryType::Scalar => {
                let mut scalar_index = self.scalar_index.write().unwrap();
                if add {
                    scalar_index
                        .entry(key.subquery_hash)
                        .or_default()
                        .push(key.clone());
                } else if let Some(keys) = scalar_index.get_mut(&key.subquery_hash) {
                    keys.retain(|k| k != key);
                    if keys.is_empty() {
                        scalar_index.remove(&key.subquery_hash);
                    }
                }
            }
            _ => {} // Other types don't need special indices
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> SubqueryCacheStats {
        let mut stats = self.stats.read().unwrap().clone();
        stats.current_entries = self.entries.read().unwrap().len();
        stats.memory_bytes = *self.current_memory.read().unwrap();
        stats
    }

    /// Clear all cached subquery results
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
        self.boolean_index.write().unwrap().clear();
        self.scalar_index.write().unwrap().clear();
        *self.current_memory.write().unwrap() = 0;

        let mut stats = self.stats.write().unwrap();
        stats.current_entries = 0;
        stats.memory_bytes = 0;
    }
}

/// Helper to create subquery cache key
pub fn create_subquery_cache_key(
    subquery_ast: &str, // Normalized subquery string
    outer_variables: Vec<(String, Value)>,
    graph_version: u64,
    schema_version: u64,
    subquery_type: SubqueryType,
) -> SubqueryCacheKey {
    let mut hasher = DefaultHasher::new();
    subquery_ast.hash(&mut hasher);

    SubqueryCacheKey {
        subquery_hash: hasher.finish(),
        outer_variables,
        graph_version,
        schema_version,
        subquery_type,
    }
}

/// Cache hit information for subquery results
#[derive(Debug, Clone)]
pub struct SubqueryCacheHit {
    pub key: SubqueryCacheKey,
    pub result: SubqueryResult,
    pub saved_execution_time: Duration,
    pub hit_timestamp: Instant,
}
