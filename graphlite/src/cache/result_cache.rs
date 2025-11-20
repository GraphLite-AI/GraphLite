// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query result caching implementation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::{CacheEntryMetadata, CacheKey, CacheLevel, CacheValue, EvictionPolicy};
use crate::exec::{QueryResult, Row};

/// Key for query result cache entries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryCacheKey {
    pub query_hash: u64,
    pub parameters: Vec<CacheParameter>,
    pub graph_version: u64,           // For invalidation when graph changes
    pub user_context: Option<String>, // For row-level security
}

/// Cached parameter value
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CacheParameter {
    String(String),
    Integer(i64),
    Float(u64), // Store as bits for exact comparison
    Boolean(bool),
    Null,
}

impl CacheKey for QueryCacheKey {
    fn cache_key(&self) -> String {
        format!(
            "query:{}:{}:{}",
            self.query_hash,
            self.graph_version,
            self.user_context.as_deref().unwrap_or("default")
        )
    }

    fn tags(&self) -> Vec<String> {
        let mut tags = vec![
            format!("graph_version:{}", self.graph_version),
            format!("query_hash:{}", self.query_hash),
        ];

        if let Some(user) = &self.user_context {
            tags.push(format!("user:{}", user));
        }

        tags
    }
}

/// Cached query result entry
#[derive(Debug, Clone)]
pub struct QueryResultEntry {
    pub result: QueryResult,
    pub execution_time: Duration,
    #[allow(dead_code)] // ROADMAP v0.5.0 - Plan hash for cache invalidation (see ROADMAP.md §9)
    pub plan_hash: u64,
    pub metadata: CacheEntryMetadata,
    #[allow(dead_code)]
    // ROADMAP v0.5.0 - Compression ratio for cache statistics (see ROADMAP.md §9)
    pub compression_ratio: Option<f32>, // If compressed
}

impl CacheValue for QueryResultEntry {
    fn size_bytes(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let rows_size = self.result.rows.len() * std::mem::size_of::<Row>();
        let variables_size = self
            .result
            .variables
            .iter()
            .map(|var| var.len())
            .sum::<usize>();

        base_size + rows_size + variables_size
    }

    fn is_valid(&self) -> bool {
        !self.metadata.is_expired() && !self.result.rows.is_empty() // Don't cache empty results
    }
}

/// Cache hit information for analytics
#[derive(Debug, Clone)]
pub struct CacheHit {
    pub key: QueryCacheKey,
    pub hit_level: CacheLevel,
    pub access_time: Instant,
    pub saved_execution_time: Duration,
}

/// LRU eviction tracker
#[derive(Debug)]
struct LRUTracker<K> {
    order: VecDeque<K>,
    positions: HashMap<K, usize>,
}

impl<K: Clone + Eq + Hash> LRUTracker<K> {
    fn new() -> Self {
        Self {
            order: VecDeque::new(),
            positions: HashMap::new(),
        }
    }

    fn access(&mut self, key: &K) {
        if let Some(&pos) = self.positions.get(key) {
            // Move to front
            self.order.remove(pos);
            self.order.push_front(key.clone());
            self.update_positions();
        } else {
            // New entry
            self.order.push_front(key.clone());
            self.positions.insert(key.clone(), 0);
            self.update_positions();
        }
    }

    fn remove_lru(&mut self) -> Option<K> {
        if let Some(key) = self.order.pop_back() {
            self.positions.remove(&key);
            self.update_positions();
            Some(key)
        } else {
            None
        }
    }

    fn update_positions(&mut self) {
        self.positions.clear();
        for (pos, key) in self.order.iter().enumerate() {
            self.positions.insert(key.clone(), pos);
        }
    }
}

/// Multi-level result cache
pub struct ResultCache {
    // L1 Cache: Hot frequently accessed results
    l1_cache: Arc<RwLock<HashMap<QueryCacheKey, QueryResultEntry>>>,
    l1_lru: Arc<RwLock<LRUTracker<QueryCacheKey>>>,
    l1_max_entries: usize,
    l1_max_memory: usize,
    l1_current_memory: Arc<RwLock<usize>>,

    // L2 Cache: Warm occasionally accessed results
    l2_cache: Arc<RwLock<HashMap<QueryCacheKey, QueryResultEntry>>>,
    l2_lru: Arc<RwLock<LRUTracker<QueryCacheKey>>>,
    l2_max_entries: usize,
    l2_max_memory: usize,
    l2_current_memory: Arc<RwLock<usize>>,

    // Cache statistics
    stats: Arc<RwLock<CacheStats>>,
    _eviction_policy: EvictionPolicy,
}

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l2_hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub insertions: u64,
    pub total_requests: u64,
    pub memory_savings_bytes: u64,
    pub time_savings_ms: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            (self.l1_hits + self.l2_hits) as f64 / self.total_requests as f64
        }
    }

    pub fn l1_hit_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.l1_hits as f64 / self.total_requests as f64
        }
    }
}

impl ResultCache {
    pub fn new(
        l1_max_entries: usize,
        l1_max_memory: usize,
        l2_max_entries: usize,
        l2_max_memory: usize,
        _eviction_policy: EvictionPolicy,
    ) -> Self {
        Self {
            l1_cache: Arc::new(RwLock::new(HashMap::new())),
            l1_lru: Arc::new(RwLock::new(LRUTracker::new())),
            l1_max_entries,
            l1_max_memory,
            l1_current_memory: Arc::new(RwLock::new(0)),

            l2_cache: Arc::new(RwLock::new(HashMap::new())),
            l2_lru: Arc::new(RwLock::new(LRUTracker::new())),
            l2_max_entries,
            l2_max_memory,
            l2_current_memory: Arc::new(RwLock::new(0)),

            stats: Arc::new(RwLock::new(CacheStats::default())),
            _eviction_policy,
        }
    }

    /// Get cached result if available
    pub fn get(&self, key: &QueryCacheKey) -> Option<CacheHit> {
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_requests += 1;
        }

        // Try L1 first
        {
            let mut l1_cache = self.l1_cache.write().unwrap();
            if let Some(entry) = l1_cache.get_mut(key) {
                if entry.is_valid() {
                    entry.metadata.update_access();
                    self.l1_lru.write().unwrap().access(key);

                    let mut stats = self.stats.write().unwrap();
                    stats.l1_hits += 1;
                    stats.time_savings_ms += entry.execution_time.as_millis() as u64;

                    return Some(CacheHit {
                        key: key.clone(),
                        hit_level: CacheLevel::L1,
                        access_time: Instant::now(),
                        saved_execution_time: entry.execution_time,
                    });
                } else {
                    // Remove expired entry
                    l1_cache.remove(key);
                }
            }
        }

        // Try L2
        {
            let should_promote;
            let _execution_time;
            let cache_hit;

            {
                let mut l2_cache = self.l2_cache.write().unwrap();
                if let Some(entry) = l2_cache.get_mut(key) {
                    if entry.is_valid() {
                        entry.metadata.update_access();
                        self.l2_lru.write().unwrap().access(key);

                        // Check if we should promote to L1
                        should_promote = entry.metadata.access_count >= 3;
                        _execution_time = entry.execution_time;

                        cache_hit = Some(CacheHit {
                            key: key.clone(),
                            hit_level: CacheLevel::L2,
                            access_time: Instant::now(),
                            saved_execution_time: entry.execution_time,
                        });

                        let mut stats = self.stats.write().unwrap();
                        stats.l2_hits += 1;
                        stats.time_savings_ms += entry.execution_time.as_millis() as u64;

                        // Promote if needed
                        if should_promote {
                            let promoted_entry = entry.clone();
                            l2_cache.remove(key);
                            // Drop the locks before calling insert_l1
                            drop(l2_cache);
                            drop(stats);
                            self.insert_l1(key.clone(), promoted_entry);
                        }

                        return cache_hit;
                    } else {
                        // Remove expired entry
                        l2_cache.remove(key);
                    }
                } else {
                    return None;
                }
            }
        }

        // Cache miss
        {
            let mut stats = self.stats.write().unwrap();
            stats.misses += 1;
        }

        None
    }

    /// Insert result into cache
    pub fn insert(
        &self,
        key: QueryCacheKey,
        result: QueryResult,
        execution_time: Duration,
        plan_hash: u64,
    ) {
        let entry = QueryResultEntry {
            result,
            execution_time,
            plan_hash,
            metadata: CacheEntryMetadata::new(0, CacheLevel::L1), // Size calculated in CacheValue impl
            compression_ratio: None,
        };

        let size = entry.size_bytes();

        // Try L1 first
        if size <= self.l1_max_memory {
            self.insert_l1(key, entry);
        } else {
            // Fall back to L2
            self.insert_l2(key, entry);
        }

        let mut stats = self.stats.write().unwrap();
        stats.insertions += 1;
    }

    fn insert_l1(&self, key: QueryCacheKey, mut entry: QueryResultEntry) {
        entry.metadata.level = CacheLevel::L1;
        let size = entry.size_bytes();

        // Evict if necessary
        self.evict_l1_if_needed(size);

        {
            let mut l1_cache = self.l1_cache.write().unwrap();
            l1_cache.insert(key.clone(), entry);
        }

        {
            let mut l1_lru = self.l1_lru.write().unwrap();
            l1_lru.access(&key);
        }

        {
            let mut current_memory = self.l1_current_memory.write().unwrap();
            *current_memory += size;
        }
    }

    fn insert_l2(&self, key: QueryCacheKey, mut entry: QueryResultEntry) {
        entry.metadata.level = CacheLevel::L2;
        let size = entry.size_bytes();

        // Evict if necessary
        self.evict_l2_if_needed(size);

        {
            let mut l2_cache = self.l2_cache.write().unwrap();
            l2_cache.insert(key.clone(), entry);
        }

        {
            let mut l2_lru = self.l2_lru.write().unwrap();
            l2_lru.access(&key);
        }

        {
            let mut current_memory = self.l2_current_memory.write().unwrap();
            *current_memory += size;
        }
    }

    fn evict_l1_if_needed(&self, incoming_size: usize) {
        let current_memory = *self.l1_current_memory.read().unwrap();
        let current_entries = self.l1_cache.read().unwrap().len();

        if current_memory + incoming_size > self.l1_max_memory
            || current_entries >= self.l1_max_entries
        {
            let mut lru = self.l1_lru.write().unwrap();
            while let Some(key_to_evict) = lru.remove_lru() {
                if let Some(evicted_entry) = self.l1_cache.write().unwrap().remove(&key_to_evict) {
                    let evicted_size = evicted_entry.size_bytes();

                    {
                        let mut current_memory = self.l1_current_memory.write().unwrap();
                        *current_memory = current_memory.saturating_sub(evicted_size);
                    }

                    // Demote to L2 if still valid
                    if evicted_entry.is_valid() {
                        self.insert_l2(key_to_evict, evicted_entry);
                    }

                    {
                        let mut stats = self.stats.write().unwrap();
                        stats.evictions += 1;
                    }

                    // Check if we have enough space now
                    let new_current_memory = *self.l1_current_memory.read().unwrap();
                    let new_current_entries = self.l1_cache.read().unwrap().len();

                    if new_current_memory + incoming_size <= self.l1_max_memory
                        && new_current_entries < self.l1_max_entries
                    {
                        break;
                    }
                }
            }
        }
    }

    fn evict_l2_if_needed(&self, incoming_size: usize) {
        let current_memory = *self.l2_current_memory.read().unwrap();
        let current_entries = self.l2_cache.read().unwrap().len();

        if current_memory + incoming_size > self.l2_max_memory
            || current_entries >= self.l2_max_entries
        {
            let mut lru = self.l2_lru.write().unwrap();
            while let Some(key_to_evict) = lru.remove_lru() {
                if let Some(evicted_entry) = self.l2_cache.write().unwrap().remove(&key_to_evict) {
                    let evicted_size = evicted_entry.size_bytes();

                    {
                        let mut current_memory = self.l2_current_memory.write().unwrap();
                        *current_memory = current_memory.saturating_sub(evicted_size);
                    }

                    {
                        let mut stats = self.stats.write().unwrap();
                        stats.evictions += 1;
                    }

                    // Check if we have enough space now
                    let new_current_memory = *self.l2_current_memory.read().unwrap();
                    let new_current_entries = self.l2_cache.read().unwrap().len();

                    if new_current_memory + incoming_size <= self.l2_max_memory
                        && new_current_entries < self.l2_max_entries
                    {
                        break;
                    }
                }
            }
        }
    }

    /// Invalidate entries by graph version
    pub fn invalidate_by_graph_version(&self, version: u64) {
        let mut removed_keys = Vec::new();

        // Remove from L1
        {
            let mut l1_cache = self.l1_cache.write().unwrap();
            l1_cache.retain(|key, entry| {
                if key.graph_version < version {
                    removed_keys.push(key.clone());
                    let size = entry.size_bytes();
                    *self.l1_current_memory.write().unwrap() =
                        self.l1_current_memory.read().unwrap().saturating_sub(size);
                    false
                } else {
                    true
                }
            });
        }

        // Remove from L2
        {
            let mut l2_cache = self.l2_cache.write().unwrap();
            l2_cache.retain(|key, entry| {
                if key.graph_version < version {
                    if !removed_keys.contains(key) {
                        removed_keys.push(key.clone());
                    }
                    let size = entry.size_bytes();
                    *self.l2_current_memory.write().unwrap() =
                        self.l2_current_memory.read().unwrap().saturating_sub(size);
                    false
                } else {
                    true
                }
            });
        }

        // Update LRU trackers
        {
            let mut l1_lru = self.l1_lru.write().unwrap();
            let mut l2_lru = self.l2_lru.write().unwrap();

            for key in removed_keys {
                // Note: In a real implementation, we'd need to properly update LRU positions
                // This is a simplified version
                l1_lru.positions.remove(&key);
                l2_lru.positions.remove(&key);
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.read().unwrap().clone()
    }

    /// Clear all cached results
    pub fn clear(&self) {
        self.l1_cache.write().unwrap().clear();
        self.l2_cache.write().unwrap().clear();
        *self.l1_current_memory.write().unwrap() = 0;
        *self.l2_current_memory.write().unwrap() = 0;
        *self.l1_lru.write().unwrap() = LRUTracker::new();
        *self.l2_lru.write().unwrap() = LRUTracker::new();
    }
}

/// Helper to create query cache key from query string and parameters
pub fn create_query_cache_key(
    query: &str,
    parameters: Vec<CacheParameter>,
    graph_version: u64,
    user_context: Option<String>,
) -> QueryCacheKey {
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    parameters.hash(&mut hasher);

    QueryCacheKey {
        query_hash: hasher.finish(),
        parameters,
        graph_version,
        user_context,
    }
}
