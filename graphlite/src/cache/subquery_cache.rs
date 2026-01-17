// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Subquery result caching for nested query optimization

use super::{CacheEntryMetadata, CacheKey, CacheLevel, CacheValue};
use crate::exec::{QueryResult, Row};
use crate::storage::Value;
use crossbeam_utils::CachePadded;
use moka::sync::Cache;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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
            SubqueryResult::Boolean(_) => size_of::<bool>(),
            SubqueryResult::Scalar(Some(value)) => {
                size_of::<Value>()
                    + match value {
                        Value::String(s) => s.len(),
                        _ => 0,
                    }
            }
            SubqueryResult::Scalar(None) => size_of::<Option<Value>>(),
            SubqueryResult::Set(values) => {
                values.len() * size_of::<Value>()
                    + values
                        .iter()
                        .map(|v| match v {
                            Value::String(s) => s.len(),
                            _ => 0,
                        })
                        .sum::<usize>()
            }
            SubqueryResult::FullResult(result) => {
                let base_size = size_of::<QueryResult>();
                let rows_size = result.rows.len() * size_of::<Row>();
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
    // ROADMAP v0.5.0 - Tracks correlation complexity for cache eviction policies. Currently, set (line 447) but not yet used in eviction scoring. Will be used for cost-based eviction when correlated subquery optimization is implemented.
    pub outer_variable_count: usize,
    pub metadata: CacheEntryMetadata,
    pub last_hit: Instant,
    pub complexity_score: f64, // Higher score = more expensive to compute
}

impl CacheValue for SubqueryCacheEntry {
    fn size_bytes(&self) -> usize {
        size_of::<Self>() + self.result.size_bytes()
    }

    fn is_valid(&self) -> bool {
        !self.metadata.is_expired() && self.result.is_valid()
    }
}

#[repr(usize)]
#[derive(Copy, Clone, Debug)]
pub enum SubqueryCacheMetric {
    Hits = 0,
    Misses = 1,
    TotalExecutionTimeSavedMs = 2,
    BooleanCacheHits = 3,
    ScalarCacheHits = 4,
    SetCacheHits = 5,
    FullResultCacheHits = 6,
    CurrentEntries = 7,
    MemoryBytes = 8,
    Invalidations = 9,
}

const SUBQUERY_CACHE_METRIC_COUNT: usize = 16;

#[derive(Debug)]
pub struct SubqueryCacheStats {
    metrics: [CachePadded<AtomicU64>; SUBQUERY_CACHE_METRIC_COUNT],
}

impl SubqueryCacheStats {
    #[inline]
    pub fn inc(&self, metric: SubqueryCacheMetric) {
        self.metrics[metric as usize].fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn add(&self, metric: SubqueryCacheMetric, value: u64) {
        self.metrics[metric as usize].fetch_add(value, Ordering::Relaxed);
    }

    #[inline]
    pub fn set(&self, metric: SubqueryCacheMetric, value: u64) {
        self.metrics[metric as usize].store(value, Ordering::Relaxed);
    }

    #[inline]
    pub fn load(&self, metric: SubqueryCacheMetric) -> u64 {
        self.metrics[metric as usize].load(Ordering::Relaxed)
    }

    #[inline]
    pub fn hit_rate(&self) -> f64 {
        let hits = self.load(SubqueryCacheMetric::Hits);
        let misses = self.load(SubqueryCacheMetric::Misses);
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }
}

impl Clone for SubqueryCacheStats {
    fn clone(&self) -> Self {
        let cloned = SubqueryCacheStats::default();
        for (index, metric) in self.metrics.iter().enumerate() {
            let value = metric.load(Ordering::Relaxed);
            cloned.metrics[index].store(value, Ordering::Relaxed);
        }
        cloned
    }
}

impl Default for SubqueryCacheStats {
    fn default() -> Self {
        Self {
            metrics: std::array::from_fn(|_| CachePadded::new(AtomicU64::new(0))),
        }
    }
}

/// Subquery result cache implementation
pub struct SubqueryCache {
    entries: Cache<SubqueryCacheKey, SubqueryCacheEntry>,
    max_memory_bytes: usize,
    stats: SubqueryCacheStats,
    ttl: Duration,
}

impl SubqueryCache {
    pub fn new(max_entries: usize, max_memory_bytes: usize, ttl: Duration) -> Self {
        let entries = Cache::builder()
            .time_to_live(ttl)
            .max_capacity(max_entries as u64)
            .build();

        Self {
            entries,
            max_memory_bytes,
            stats: SubqueryCacheStats::default(),
            ttl,
        }
    }

    /// Get cached subquery result
    pub fn get(&self, key: &SubqueryCacheKey) -> Option<SubqueryResult> {
        if let Some(entry) = self.entries.get(key) {
            {
                self.stats.add(SubqueryCacheMetric::Hits, 1);
                self.stats.add(
                    SubqueryCacheMetric::TotalExecutionTimeSavedMs,
                    entry.execution_time.as_millis() as u64,
                );

                match &entry.result {
                    SubqueryResult::Boolean(_) => {
                        self.stats.inc(SubqueryCacheMetric::BooleanCacheHits)
                    }
                    SubqueryResult::Scalar(_) => {
                        self.stats.inc(SubqueryCacheMetric::ScalarCacheHits)
                    }
                    SubqueryResult::Set(_) => self.stats.inc(SubqueryCacheMetric::ScalarCacheHits),
                    SubqueryResult::FullResult(_) => {
                        self.stats.inc(SubqueryCacheMetric::FullResultCacheHits)
                    }
                }
            }
            Some(entry.result)
        } else {
            self.stats.inc(SubqueryCacheMetric::Misses);
            None
        }
    }

    pub fn insert(
        &self,
        key: SubqueryCacheKey,
        result: SubqueryResult,
        execution_time: Duration,
        complexity_score: f64,
    ) {
        let entry = SubqueryCacheEntry {
            result,
            execution_time,
            outer_variable_count: key.outer_variables.len(),
            metadata: CacheEntryMetadata::new(0, CacheLevel::L1)
                .with_ttl(self.ttl)
                .with_tags(key.tags()),
            last_hit: Instant::now(),
            complexity_score,
        };

        let entry_size = entry.size_bytes();

        self.entries.insert(key, entry);
        self.stats.inc(SubqueryCacheMetric::CurrentEntries);
        self.stats
            .add(SubqueryCacheMetric::MemoryBytes, entry_size as u64);
    }

    /// Invalidate entries by graph version
    pub fn invalidate_by_graph_version(&self, version: u64) {
        let _predicate_id = self
            .entries
            .invalidate_entries_if(move |key, _| key.graph_version < version)
            .unwrap();

        self.stats.inc(SubqueryCacheMetric::Invalidations);
        self.stats.set(
            SubqueryCacheMetric::CurrentEntries,
            self.entries.entry_count(),
        );
    }

    /// Invalidate entries by schema version
    pub fn invalidate_by_schema_version(&self, version: u64) {
        let _predicate_id = self
            .entries
            .invalidate_entries_if(move |key, _| key.schema_version < version)
            .unwrap();

        self.stats.inc(SubqueryCacheMetric::Invalidations);
        self.stats.set(
            SubqueryCacheMetric::CurrentEntries,
            self.entries.entry_count(),
        );
    }

    /// Get cache statistics
    pub fn stats(&self) -> SubqueryCacheStats {
        self.stats.clone()
    }

    /// Clear all cached subquery results
    pub fn clear(&self) {
        self.entries.invalidate_all();

        self.stats.set(SubqueryCacheMetric::CurrentEntries, 0);
        self.stats.set(SubqueryCacheMetric::MemoryBytes, 0);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_nothing_for_a_nonexistent_subquery_cache_key() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));
        let result = cache.get(&any_subquery_cache_key());

        assert_eq!(result.is_none(), true);
    }

    #[test]
    fn should_return_nothing_for_an_expired_subquery_cache_key() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(0));
        let cache_key = SubqueryCacheKey {
            subquery_hash: 10,
            outer_variables: vec![],
            graph_version: 1,
            schema_version: 1,
            subquery_type: SubqueryType::Scalar,
        };
        let result = SubqueryResult::Scalar(Some(Value::Boolean(true)));
        cache.insert(cache_key.clone(), result, Duration::from_secs(2), 0.50);

        let result = cache.get(&any_subquery_cache_key());
        assert_eq!(result.is_none(), true);
    }

    #[test]
    fn should_return_cache_result() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = SubqueryCacheKey {
            subquery_hash: 10,
            outer_variables: vec![],
            graph_version: 1,
            schema_version: 1,
            subquery_type: SubqueryType::Scalar,
        };
        let result = SubqueryResult::Scalar(Some(Value::Boolean(true)));
        cache.insert(cache_key.clone(), result, Duration::from_secs(0), 0.50);

        let result = cache.get(&cache_key);
        assert_eq!(result.is_some(), true);

        let value = result.unwrap().as_boolean();
        assert_eq!(Some(true), value);
    }

    #[test]
    fn should_update_the_memory_used_on_adding_entry() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = SubqueryCacheKey {
            subquery_hash: 10,
            outer_variables: vec![],
            graph_version: 1,
            schema_version: 1,
            subquery_type: SubqueryType::Scalar,
        };
        let result = SubqueryResult::Scalar(Some(Value::Boolean(true)));
        cache.insert(cache_key.clone(), result, Duration::from_secs(0), 0.50);

        let stats = cache.stats();
        assert!(stats.load(SubqueryCacheMetric::MemoryBytes) > 0);
    }

    #[test]
    fn should_update_cache_entry_stats_on_insert() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = SubqueryCacheKey {
            subquery_hash: 10,
            outer_variables: vec![],
            graph_version: 1,
            schema_version: 1,
            subquery_type: SubqueryType::Scalar,
        };
        let result = SubqueryResult::Scalar(Some(Value::Boolean(true)));
        cache.insert(cache_key.clone(), result, Duration::from_secs(0), 0.50);

        let stats = cache.stats();
        assert_eq!(1, stats.load(SubqueryCacheMetric::CurrentEntries));
    }

    #[test]
    fn should_update_cache_stats_on_cache_hit() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = SubqueryCacheKey {
            subquery_hash: 10,
            outer_variables: vec![],
            graph_version: 1,
            schema_version: 1,
            subquery_type: SubqueryType::Scalar,
        };
        let result = SubqueryResult::Scalar(Some(Value::Boolean(true)));
        cache.insert(cache_key.clone(), result, Duration::from_secs(0), 0.50);

        let _ = cache.get(&cache_key);

        let stats = cache.stats();
        assert_eq!(1, stats.load(SubqueryCacheMetric::Hits));
    }

    #[test]
    fn should_update_cache_stats_on_cache_miss() {
        let max_memory_bytes = 1024;
        let cache = SubqueryCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let _ = cache.get(&any_subquery_cache_key());

        let stats = cache.stats();
        assert_eq!(1, stats.load(SubqueryCacheMetric::Misses));
    }

    fn any_subquery_cache_key() -> SubqueryCacheKey {
        SubqueryCacheKey {
            subquery_hash: 0,
            outer_variables: vec![],
            graph_version: 0,
            schema_version: 0,
            subquery_type: SubqueryType::Exists,
        }
    }
}
