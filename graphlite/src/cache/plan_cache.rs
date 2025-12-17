// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query plan caching to avoid recompilation

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use super::{CacheEntryMetadata, CacheKey, CacheLevel, CacheValue};
use crate::plan::logical::LogicalPlan;
use crate::plan::physical::PhysicalPlan;
use crate::plan::trace::PlanTrace;

/// Key for plan cache entries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanCacheKey {
    pub query_structure_hash: u64,  // Hash of normalized query structure
    pub schema_hash: u64,           // Hash of relevant schema info
    pub optimization_level: String, // Optimization settings
    pub hints: Vec<String>,         // Query hints that affect planning
}

impl CacheKey for PlanCacheKey {
    fn cache_key(&self) -> String {
        format!(
            "plan:{}:{}:{}",
            self.query_structure_hash, self.schema_hash, self.optimization_level
        )
    }

    fn tags(&self) -> Vec<String> {
        let mut tags = vec![
            format!("schema:{}", self.schema_hash),
            format!("optimization:{}", self.optimization_level),
        ];

        for hint in &self.hints {
            tags.push(format!("hint:{}", hint));
        }

        tags
    }
}

/// Cached plan entry
#[derive(Debug, Clone)]
pub struct PlanCacheEntry {
    pub logical_plan: LogicalPlan,
    pub physical_plan: PhysicalPlan,
    pub trace: Option<PlanTrace>,
    pub compilation_time: Duration,
    pub estimated_cost: f64,
    pub estimated_rows: usize,
    pub metadata: CacheEntryMetadata,
    pub usage_count: u64,
    pub last_used: Instant,
}

impl CacheValue for PlanCacheEntry {
    fn size_bytes(&self) -> usize {
        // Rough estimate - in practice would need serialization
        let base_size = std::mem::size_of::<Self>();
        let trace_size = self
            .trace
            .as_ref()
            .map(|t| t.steps.len() * 100) // Rough estimate
            .unwrap_or(0);

        base_size + trace_size + 1024 // Plan structure overhead
    }

    fn is_valid(&self) -> bool {
        !self.metadata.is_expired()
    }
}

/// Plan cache statistics
#[derive(Debug, Default, Clone)]
pub struct PlanCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub compilations_saved: u64,
    pub total_compilation_time_saved_ms: u64,
    pub evictions: u64,
    pub current_entries: usize,
    pub current_memory_bytes: usize,
}

impl PlanCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn average_compilation_time_saved_ms(&self) -> f64 {
        if self.compilations_saved == 0 {
            0.0
        } else {
            self.total_compilation_time_saved_ms as f64 / self.compilations_saved as f64
        }
    }
}

/// Plan cache implementation
pub struct PlanCache {
    entries: RwLock<HashMap<PlanCacheKey, PlanCacheEntry>>,
    max_entries: usize,
    max_memory_bytes: usize,
    current_memory: RwLock<usize>,
    stats: RwLock<PlanCacheStats>,
    default_ttl: Duration,
}

impl PlanCache {
    pub fn new(max_entries: usize, max_memory_bytes: usize, default_ttl: Duration) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_entries,
            max_memory_bytes,
            current_memory: RwLock::new(0),
            stats: RwLock::new(PlanCacheStats::default()),
            default_ttl,
        }
    }

    /// Get cached plan if available
    pub fn get(&self, key: &PlanCacheKey) -> Option<PlanCacheEntry> {
        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_valid() {
                // Update access info
                entry.metadata.update_access();
                entry.usage_count += 1;
                entry.last_used = Instant::now();

                // Update stats
                {
                    let mut stats = self.stats.write().unwrap();
                    stats.hits += 1;
                    stats.compilations_saved += 1;
                    stats.total_compilation_time_saved_ms +=
                        entry.compilation_time.as_millis() as u64;
                }

                Some(entry.clone())
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
                    stats.current_memory_bytes = *self.current_memory.read().unwrap();
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

    /// Insert plan into cache
    pub fn insert(
        &self,
        key: PlanCacheKey,
        logical_plan: LogicalPlan,
        physical_plan: PhysicalPlan,
        trace: Option<PlanTrace>,
        compilation_time: Duration,
    ) {
        let estimated_cost = physical_plan.estimated_cost;
        let estimated_rows = physical_plan.estimated_rows;

        let entry = PlanCacheEntry {
            logical_plan,
            physical_plan,
            trace,
            compilation_time,
            estimated_cost,
            estimated_rows,
            metadata: CacheEntryMetadata::new(0, CacheLevel::L1).with_ttl(self.default_ttl),
            usage_count: 0,
            last_used: Instant::now(),
        };

        let size = entry.size_bytes();

        // Check if we need to evict entries
        self.evict_if_needed(size);

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
            stats.current_memory_bytes = *self.current_memory.read().unwrap();
        }
    }

    fn evict_if_needed(&self, incoming_size: usize) {
        let current_memory = *self.current_memory.read().unwrap();
        let current_entries = self.entries.read().unwrap().len();

        if current_memory + incoming_size > self.max_memory_bytes
            || current_entries >= self.max_entries
        {
            // Collect candidates for eviction (least recently used with low usage count)
            let mut candidates: Vec<(PlanCacheKey, Instant, u64)> = {
                let entries = self.entries.read().unwrap();
                entries
                    .iter()
                    .map(|(key, entry)| (key.clone(), entry.last_used, entry.usage_count))
                    .collect()
            };

            // Sort by last used time, then by usage count
            candidates.sort_by(|a, b| match a.1.cmp(&b.1) {
                std::cmp::Ordering::Equal => a.2.cmp(&b.2),
                other => other,
            });

            // Evict entries until we have enough space
            let mut entries = self.entries.write().unwrap();
            for (key, _, _) in candidates {
                if let Some(evicted_entry) = entries.remove(&key) {
                    let evicted_size = evicted_entry.size_bytes();

                    {
                        let mut current_memory = self.current_memory.write().unwrap();
                        *current_memory = current_memory.saturating_sub(evicted_size);
                    }

                    {
                        let mut stats = self.stats.write().unwrap();
                        stats.evictions += 1;
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

    /// Invalidate plans by schema hash
    pub fn invalidate_by_schema(&self, schema_hash: u64) {
        let mut entries = self.entries.write().unwrap();
        let mut removed_size = 0;

        entries.retain(|key, entry| {
            if key.schema_hash == schema_hash {
                removed_size += entry.size_bytes();
                false
            } else {
                true
            }
        });

        {
            let mut current_memory = self.current_memory.write().unwrap();
            *current_memory = current_memory.saturating_sub(removed_size);
        }

        {
            let mut stats = self.stats.write().unwrap();
            stats.current_entries = entries.len();
            stats.current_memory_bytes = *self.current_memory.read().unwrap();
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> PlanCacheStats {
        let mut stats = self.stats.read().unwrap().clone();
        stats.current_entries = self.entries.read().unwrap().len();
        stats.current_memory_bytes = *self.current_memory.read().unwrap();
        stats
    }

    /// Clear all cached plans
    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
        *self.current_memory.write().unwrap() = 0;

        let mut stats = self.stats.write().unwrap();
        stats.current_entries = 0;
        stats.current_memory_bytes = 0;
    }

    /// Get cache efficiency metrics
    pub fn efficiency_metrics(&self) -> CacheEfficiencyMetrics {
        let stats = self.stats.read().unwrap();
        let entries = self.entries.read().unwrap();

        let total_usage: u64 = entries.values().map(|e| e.usage_count).sum();
        let avg_usage = if entries.is_empty() {
            0.0
        } else {
            total_usage as f64 / entries.len() as f64
        };

        let memory_efficiency = if self.max_memory_bytes == 0 {
            0.0
        } else {
            stats.current_memory_bytes as f64 / self.max_memory_bytes as f64
        };

        CacheEfficiencyMetrics {
            hit_rate: stats.hit_rate(),
            memory_utilization: memory_efficiency,
            average_entry_usage: avg_usage,
            eviction_rate: if stats.hits + stats.misses == 0 {
                0.0
            } else {
                stats.evictions as f64 / (stats.hits + stats.misses) as f64
            },
            compilation_time_saved_ms: stats.total_compilation_time_saved_ms,
        }
    }
}

/// Cache efficiency metrics for monitoring
#[derive(Debug, Clone)]
pub struct CacheEfficiencyMetrics {
    pub hit_rate: f64,
    pub memory_utilization: f64,
    pub average_entry_usage: f64,
    pub eviction_rate: f64,
    pub compilation_time_saved_ms: u64,
}

/// Helper to create plan cache key from query and context
pub fn create_plan_cache_key(
    query_ast_hash: u64,
    schema_hash: u64,
    optimization_level: &str,
    hints: Vec<String>,
) -> PlanCacheKey {
    PlanCacheKey {
        query_structure_hash: query_ast_hash,
        schema_hash,
        optimization_level: optimization_level.to_string(),
        hints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::logical::LogicalNode;
    use crate::plan::physical::PhysicalNode;

    #[test]
    fn should_return_nothing_for_a_nonexistent_plan_cache_key() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));
        let result = cache.get(&any_plan_cache_key());

        assert_eq!(result.is_none(), true);
    }

    #[test]
    fn should_return_nothing_for_an_expired_subquery_cache_key() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(0));

        let cache_key = PlanCacheKey {
            query_structure_hash: 10,
            schema_hash: 10,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        };
        cache.insert(
            cache_key.clone(),
            LogicalPlan::new(LogicalNode::SingleRow),
            PhysicalPlan::new(PhysicalNode::SingleRow {
                estimated_rows: 100,
                estimated_cost: 10.0,
            }),
            None,
            Duration::from_millis(1),
        );

        let result = cache.get(&cache_key);
        assert_eq!(result.is_none(), true);
    }

    #[test]
    fn should_return_cache_result() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = PlanCacheKey {
            query_structure_hash: 10,
            schema_hash: 10,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        };
        cache.insert(
            cache_key.clone(),
            LogicalPlan::new(LogicalNode::SingleRow),
            PhysicalPlan::new(PhysicalNode::SingleRow {
                estimated_rows: 100,
                estimated_cost: 10.0,
            }),
            None,
            Duration::from_millis(1),
        );

        let result = cache.get(&cache_key);
        assert_eq!(result.is_some(), true);

        let cache_entry = result.unwrap();
        assert_eq!(10.0, cache_entry.estimated_cost);
        assert_eq!(100, cache_entry.estimated_rows);
    }

    #[test]
    fn should_update_the_memory_used_on_adding_entry() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = PlanCacheKey {
            query_structure_hash: 10,
            schema_hash: 10,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        };
        cache.insert(
            cache_key.clone(),
            LogicalPlan::new(LogicalNode::SingleRow),
            PhysicalPlan::new(PhysicalNode::SingleRow {
                estimated_rows: 100,
                estimated_cost: 10.0,
            }),
            None,
            Duration::from_millis(1),
        );

        let memory_guard = cache.current_memory.read().unwrap();
        assert!(*memory_guard > 0);
    }

    #[test]
    fn should_update_cache_stats_on_insert() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = PlanCacheKey {
            query_structure_hash: 10,
            schema_hash: 10,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        };
        cache.insert(
            cache_key.clone(),
            LogicalPlan::new(LogicalNode::SingleRow),
            PhysicalPlan::new(PhysicalNode::SingleRow {
                estimated_rows: 100,
                estimated_cost: 10.0,
            }),
            None,
            Duration::from_millis(1),
        );

        let stats_guard = cache.stats.read().unwrap();
        assert_eq!(1, stats_guard.current_entries);
    }

    #[test]
    fn should_update_cache_stats_on_cache_hit() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let cache_key = PlanCacheKey {
            query_structure_hash: 10,
            schema_hash: 10,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        };
        cache.insert(
            cache_key.clone(),
            LogicalPlan::new(LogicalNode::SingleRow),
            PhysicalPlan::new(PhysicalNode::SingleRow {
                estimated_rows: 100,
                estimated_cost: 10.0,
            }),
            None,
            Duration::from_millis(1),
        );

        let _ = cache.get(&cache_key);

        let stats_guard = cache.stats.read().unwrap();
        assert_eq!(1, stats_guard.hits);
    }

    #[test]
    fn should_update_cache_stats_on_cache_miss() {
        let max_memory_bytes = 1024;
        let cache = PlanCache::new(1, max_memory_bytes, Duration::from_millis(100));

        let _ = cache.get(&any_plan_cache_key());

        let stats_guard = cache.stats.read().unwrap();
        assert_eq!(1, stats_guard.misses);
    }

    fn any_plan_cache_key() -> PlanCacheKey {
        PlanCacheKey {
            query_structure_hash: 0,
            schema_hash: 0,
            optimization_level: "Basic".to_string(),
            hints: vec![],
        }
    }
}
