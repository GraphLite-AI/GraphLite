// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Central cache management and coordination

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::exec::QueryResult;
use crate::plan::logical::LogicalPlan;
use crate::plan::physical::PhysicalPlan;
use crate::plan::trace::PlanTrace;

use super::{
    plan_cache::{create_plan_cache_key, CacheEfficiencyMetrics, PlanCacheStats},
    result_cache::{
        create_query_cache_key, CacheHit, CacheParameter, CacheStats as ResultCacheStats,
        QueryCacheKey,
    },
    subquery_cache::{create_subquery_cache_key, SubqueryCacheStats},
    CacheConfig, CacheLevel, InvalidationEvent, InvalidationManager, PlanCache, PlanCacheEntry,
    PlanCacheKey, ResultCache, SubqueryCache, SubqueryCacheHit, SubqueryCacheKey, SubqueryResult,
    SubqueryType,
};

/// Central cache manager coordinating all cache types
pub struct CacheManager {
    config: CacheConfig,

    // Core caches
    result_cache: Arc<ResultCache>,
    plan_cache: Arc<PlanCache>,
    subquery_cache: Arc<SubqueryCache>,

    // Cache statistics and monitoring
    global_stats: Arc<RwLock<GlobalCacheStats>>,

    // Version tracking for invalidation
    graph_version: Arc<RwLock<u64>>,
    schema_version: Arc<RwLock<u64>>,

    // Invalidation management
    invalidation_manager: Arc<InvalidationManager>,

    // Event tracking
    events: Arc<RwLock<Vec<CacheEvent>>>,
    max_events: usize,
}

/// Global cache statistics across all cache types
#[derive(Debug, Default, Clone)]
pub struct GlobalCacheStats {
    pub total_memory_bytes: usize,
    pub total_entries: usize,
    pub total_hits: u64,
    pub total_misses: u64,
    pub total_evictions: u64,
    pub uptime: Duration,
    pub last_reset: Option<Instant>,
}

impl GlobalCacheStats {
    pub fn overall_hit_rate(&self) -> f64 {
        let total = self.total_hits + self.total_misses;
        if total == 0 {
            0.0
        } else {
            self.total_hits as f64 / total as f64
        }
    }

    pub fn memory_efficiency(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.total_memory_bytes as f64 / self.total_entries as f64
        }
    }
}

/// Cache events for monitoring and debugging
#[derive(Debug, Clone)]
pub enum CacheEvent {
    ResultCacheHit {
        key: QueryCacheKey,
        level: CacheLevel,
        saved_time_ms: u64,
        timestamp: Instant,
    },
    ResultCacheMiss {
        key: QueryCacheKey,
        timestamp: Instant,
    },
    PlanCacheHit {
        key: PlanCacheKey,
        saved_time_ms: u64,
        timestamp: Instant,
    },
    PlanCacheMiss {
        key: PlanCacheKey,
        timestamp: Instant,
    },
    Eviction {
        cache_type: String,
        reason: String,
        timestamp: Instant,
    },
    Invalidation {
        strategy: String,
        affected_entries: usize,
        timestamp: Instant,
    },
    ConfigUpdate {
        old_config: String,
        new_config: String,
        timestamp: Instant,
    },
}

impl CacheManager {
    /// Create new cache manager with configuration
    pub fn new(config: CacheConfig) -> Result<Self, String> {
        config.validate()?;

        let result_cache = Arc::new(ResultCache::new(
            config.l1_config.max_entries,
            config.l1_config.max_memory_bytes,
            config.l2_config.max_entries,
            config.l2_config.max_memory_bytes,
            config.eviction_policy.clone(),
        ));

        let plan_cache = Arc::new(PlanCache::new(
            config.l3_config.max_entries,
            config.l3_config.max_memory_bytes,
            config
                .l3_config
                .default_ttl
                .unwrap_or(Duration::from_secs(3600)),
        ));

        let subquery_cache = Arc::new(SubqueryCache::new(
            config.l1_config.max_entries / 2, // Subqueries are smaller, allow more entries
            config.l1_config.max_memory_bytes / 4, // But use less memory
            config
                .l1_config
                .default_ttl
                .unwrap_or(Duration::from_secs(300)),
        ));

        let invalidation_strategy = match &config.invalidation_strategy {
            super::cache_config::InvalidationStrategy::Manual => {
                super::invalidation::InvalidationStrategy::Manual
            }
            super::cache_config::InvalidationStrategy::Ttl => {
                super::invalidation::InvalidationStrategy::Ttl {
                    default_ttl: Duration::from_secs(3600),
                    max_ttl: Duration::from_secs(7200),
                }
            }
            super::cache_config::InvalidationStrategy::TagBased => {
                super::invalidation::InvalidationStrategy::TagBased {
                    sensitive_tags: [
                        "nodes".to_string(),
                        "edges".to_string(),
                        "schema".to_string(),
                    ]
                    .into_iter()
                    .collect(),
                    propagation_delay: Duration::from_millis(100),
                }
            }
            super::cache_config::InvalidationStrategy::Versioned => {
                super::invalidation::InvalidationStrategy::Versioned {
                    track_data_version: true,
                    track_schema_version: true,
                }
            }
            super::cache_config::InvalidationStrategy::Hybrid {
                primary,
                fallback: _,
            } => {
                // Use primary strategy for now - could be enhanced to use both
                match primary.as_ref() {
                    super::cache_config::InvalidationStrategy::TagBased => {
                        super::invalidation::InvalidationStrategy::TagBased {
                            sensitive_tags: [
                                "nodes".to_string(),
                                "edges".to_string(),
                                "schema".to_string(),
                            ]
                            .into_iter()
                            .collect(),
                            propagation_delay: Duration::from_millis(100),
                        }
                    }
                    _ => super::invalidation::InvalidationStrategy::Manual,
                }
            }
        };

        Ok(Self {
            config,
            result_cache,
            plan_cache,
            subquery_cache,
            global_stats: Arc::new(RwLock::new(GlobalCacheStats::default())),
            graph_version: Arc::new(RwLock::new(1)),
            schema_version: Arc::new(RwLock::new(1)),
            invalidation_manager: Arc::new(InvalidationManager::new(invalidation_strategy, 1000)),
            events: Arc::new(RwLock::new(Vec::new())),
            max_events: 10000,
        })
    }

    /// Get cached query result
    pub fn get_query_result(
        &self,
        query: &str,
        parameters: Vec<CacheParameter>,
        user_context: Option<String>,
    ) -> Option<(QueryResult, CacheHit)> {
        if !self.config.enabled {
            return None;
        }

        let graph_version = *self.graph_version.read().unwrap();
        let key = create_query_cache_key(query, parameters, graph_version, user_context);

        if let Some(cache_hit) = self.result_cache.get(&key) {
            self.record_event(CacheEvent::ResultCacheHit {
                key: key.clone(),
                level: cache_hit.hit_level,
                saved_time_ms: cache_hit.saved_execution_time.as_millis() as u64,
                timestamp: Instant::now(),
            });

            // Get the actual result from the cache
            // Note: This is a simplified version - in practice we'd need to extract the result
            // from the cache hit or modify the cache to return the result directly
            None // Placeholder - would return actual cached result
        } else {
            self.record_event(CacheEvent::ResultCacheMiss {
                key,
                timestamp: Instant::now(),
            });
            None
        }
    }

    /// Cache query result
    pub fn cache_query_result(
        &self,
        query: &str,
        parameters: Vec<CacheParameter>,
        user_context: Option<String>,
        result: QueryResult,
        execution_time: Duration,
        plan_hash: u64,
    ) {
        if !self.config.enabled {
            return;
        }

        let graph_version = *self.graph_version.read().unwrap();
        let key = create_query_cache_key(query, parameters, graph_version, user_context);

        self.result_cache
            .insert(key, result, execution_time, plan_hash);
        self.update_global_stats();
    }

    /// Get cached query plan
    pub fn get_query_plan(
        &self,
        query_hash: u64,
        optimization_level: &str,
        hints: Vec<String>,
    ) -> Option<PlanCacheEntry> {
        if !self.config.enabled {
            return None;
        }

        let schema_version = *self.schema_version.read().unwrap();
        let key = create_plan_cache_key(
            query_hash,
            schema_version,
            optimization_level,
            hints.clone(),
        );

        if let Some(plan_entry) = self.plan_cache.get(&key) {
            self.record_event(CacheEvent::PlanCacheHit {
                key,
                saved_time_ms: plan_entry.compilation_time.as_millis() as u64,
                timestamp: Instant::now(),
            });
            Some(plan_entry)
        } else {
            self.record_event(CacheEvent::PlanCacheMiss {
                key,
                timestamp: Instant::now(),
            });
            None
        }
    }

    /// Cache compiled query plan
    pub fn cache_query_plan(
        &self,
        query_hash: u64,
        optimization_level: &str,
        hints: Vec<String>,
        logical_plan: LogicalPlan,
        physical_plan: PhysicalPlan,
        trace: Option<PlanTrace>,
        compilation_time: Duration,
    ) {
        if !self.config.enabled {
            return;
        }

        let schema_version = *self.schema_version.read().unwrap();
        let key = create_plan_cache_key(query_hash, schema_version, optimization_level, hints);

        self.plan_cache
            .insert(key, logical_plan, physical_plan, trace, compilation_time);
        self.update_global_stats();
    }

    /// Get cached subquery result
    pub fn get_subquery_result(
        &self,
        subquery_ast: &str,
        outer_variables: Vec<(String, crate::storage::Value)>,
        subquery_type: SubqueryType,
    ) -> Option<SubqueryCacheHit> {
        if !self.config.enabled {
            return None;
        }

        let graph_version = *self.graph_version.read().unwrap();
        let schema_version = *self.schema_version.read().unwrap();
        let key = create_subquery_cache_key(
            subquery_ast,
            outer_variables,
            graph_version,
            schema_version,
            subquery_type,
        );

        if let Some(result) = self.subquery_cache.get(&key) {
            // Record cache hit event
            self.record_event(CacheEvent::ResultCacheHit {
                key: QueryCacheKey {
                    query_hash: key.subquery_hash,
                    parameters: vec![], // Subqueries don't have parameters in the same way
                    graph_version: key.graph_version,
                    user_context: None,
                },
                level: CacheLevel::L1,
                saved_time_ms: 0, // Would need to track execution time
                timestamp: Instant::now(),
            });

            Some(SubqueryCacheHit {
                key,
                result,
                saved_execution_time: Duration::from_millis(0), // Would get from cache entry
                hit_timestamp: Instant::now(),
            })
        } else {
            None
        }
    }

    /// Cache subquery result
    pub fn cache_subquery_result(
        &self,
        subquery_ast: &str,
        outer_variables: Vec<(String, crate::storage::Value)>,
        subquery_type: SubqueryType,
        result: SubqueryResult,
        execution_time: Duration,
        complexity_score: f64,
    ) {
        if !self.config.enabled {
            return;
        }

        let graph_version = *self.graph_version.read().unwrap();
        let schema_version = *self.schema_version.read().unwrap();
        let key = create_subquery_cache_key(
            subquery_ast,
            outer_variables,
            graph_version,
            schema_version,
            subquery_type,
        );

        self.subquery_cache
            .insert(key, result, execution_time, complexity_score);
        self.update_global_stats();
    }

    /// Invalidate caches when graph data changes
    pub fn invalidate_on_data_change(&self, table: Option<String>, affected_rows: u64) {
        let mut graph_version = self.graph_version.write().unwrap();
        *graph_version += 1;

        // Create invalidation event
        let event = InvalidationEvent::DataUpdate {
            table: table.unwrap_or_else(|| "unknown".to_string()),
            affected_rows,
            columns: vec![], // Could be enhanced to track specific columns
        };

        // Handle invalidation through the manager
        let result = self.invalidation_manager.handle_event(event.clone());

        // Invalidate result cache and subquery cache entries with old graph version
        self.result_cache
            .invalidate_by_graph_version(*graph_version);
        self.subquery_cache
            .invalidate_by_graph_version(*graph_version);

        self.record_event(CacheEvent::Invalidation {
            strategy: result.strategy_used,
            affected_entries: result.entries_invalidated,
            timestamp: Instant::now(),
        });

        self.update_global_stats();
    }

    /// Invalidate caches when schema changes  
    pub fn invalidate_on_schema_change(&self, table: String, change_type: String) {
        let mut schema_version = self.schema_version.write().unwrap();
        *schema_version += 1;

        // Create schema change event
        let schema_change_type = match change_type.as_str() {
            "table_created" => super::invalidation::SchemaChangeType::TableCreated,
            "table_dropped" => super::invalidation::SchemaChangeType::TableDropped,
            "column_added" => super::invalidation::SchemaChangeType::ColumnAdded,
            "column_dropped" => super::invalidation::SchemaChangeType::ColumnDropped,
            "column_modified" => super::invalidation::SchemaChangeType::ColumnModified,
            "constraint_added" => super::invalidation::SchemaChangeType::ConstraintAdded,
            "constraint_dropped" => super::invalidation::SchemaChangeType::ConstraintDropped,
            _ => super::invalidation::SchemaChangeType::ColumnModified,
        };

        let event = InvalidationEvent::SchemaChange {
            table,
            change_type: schema_change_type,
        };

        // Handle invalidation through the manager
        let result = self.invalidation_manager.handle_event(event.clone());

        // Invalidate plan cache entries with old schema version
        self.plan_cache.invalidate_by_schema(*schema_version);

        // Also invalidate result cache and subquery cache since plans may have changed
        let graph_version = *self.graph_version.read().unwrap();
        self.result_cache.invalidate_by_graph_version(graph_version);
        self.subquery_cache
            .invalidate_by_schema_version(*schema_version);

        self.record_event(CacheEvent::Invalidation {
            strategy: result.strategy_used,
            affected_entries: result.entries_invalidated,
            timestamp: Instant::now(),
        });

        self.update_global_stats();
    }

    /// Get comprehensive cache statistics
    pub fn get_stats(&self) -> CacheManagerStats {
        let result_stats = self.result_cache.stats();
        let plan_stats = self.plan_cache.stats();
        let subquery_stats = self.subquery_cache.stats();
        let global_stats = self.global_stats.read().unwrap().clone();
        let efficiency_metrics = self.plan_cache.efficiency_metrics();

        CacheManagerStats {
            global: global_stats,
            result_cache: result_stats,
            plan_cache: plan_stats,
            subquery_cache: Some(subquery_stats),
            efficiency: efficiency_metrics,
            config: self.config.clone(),
            graph_version: *self.graph_version.read().unwrap(),
            schema_version: *self.schema_version.read().unwrap(),
        }
    }

    /// Update cache configuration
    pub fn update_config(&self, new_config: CacheConfig) -> Result<(), String> {
        new_config.validate()?;

        let old_config_str = format!("{:?}", self.config);
        let new_config_str = format!("{:?}", new_config);

        self.record_event(CacheEvent::ConfigUpdate {
            old_config: old_config_str,
            new_config: new_config_str,
            timestamp: Instant::now(),
        });

        // Note: In a full implementation, we'd update the actual config
        // This would require making config fields mutable or rebuilding caches

        Ok(())
    }

    /// Manually invalidate cache entries by tags
    pub fn invalidate_by_tags(&self, tags: Vec<String>, reason: String) {
        let event = InvalidationEvent::Manual { tags, reason };
        let result = self.invalidation_manager.handle_event(event.clone());

        self.record_event(CacheEvent::Invalidation {
            strategy: result.strategy_used,
            affected_entries: result.entries_invalidated,
            timestamp: Instant::now(),
        });

        self.update_global_stats();
    }

    /// Handle memory pressure by triggering evictions
    pub fn handle_memory_pressure(&self, current_usage: usize, max_usage: usize) {
        let event = InvalidationEvent::MemoryPressure {
            current_usage,
            max_usage,
        };
        let _result = self.invalidation_manager.handle_event(event.clone());

        // Force evictions in result and plan caches
        // This would need to be implemented as aggressive eviction methods

        self.record_event(CacheEvent::Eviction {
            cache_type: "all".to_string(),
            reason: "memory_pressure".to_string(),
            timestamp: Instant::now(),
        });

        self.update_global_stats();
    }

    /// Clear all caches
    pub fn clear_all(&self) {
        self.result_cache.clear();
        self.plan_cache.clear();
        self.subquery_cache.clear();

        {
            let mut global_stats = self.global_stats.write().unwrap();
            *global_stats = GlobalCacheStats::default();
            global_stats.last_reset = Some(Instant::now());
        }

        self.events.write().unwrap().clear();
    }

    /// Get recent cache events for debugging
    pub fn get_recent_events(&self, limit: Option<usize>) -> Vec<CacheEvent> {
        let events = self.events.read().unwrap();
        let limit = limit.unwrap_or(100).min(events.len());
        events.iter().rev().take(limit).cloned().collect()
    }

    /// Get cache health score (0.0 to 1.0)
    pub fn get_health_score(&self) -> CacheHealthScore {
        let stats = self.get_stats();

        // Hit rate score (0.0 to 1.0)
        let hit_rate_score = stats.global.overall_hit_rate();

        // Memory efficiency score
        let memory_usage_ratio =
            stats.global.total_memory_bytes as f64 / self.config.max_memory_bytes as f64;
        let memory_score = if memory_usage_ratio > 0.9 {
            0.5 // High memory usage is concerning
        } else if memory_usage_ratio > 0.7 {
            0.8 // Moderate usage is good
        } else {
            1.0 // Low usage is excellent
        };

        // Eviction rate score (lower is better)
        let total_requests = stats.global.total_hits + stats.global.total_misses;
        let eviction_rate = if total_requests == 0 {
            0.0
        } else {
            stats.global.total_evictions as f64 / total_requests as f64
        };
        let eviction_score = (1.0 - eviction_rate.min(1.0)).max(0.0);

        // Overall score (weighted average)
        let overall_score = (hit_rate_score * 0.5) + (memory_score * 0.3) + (eviction_score * 0.2);

        CacheHealthScore {
            overall: overall_score,
            hit_rate: hit_rate_score,
            memory_efficiency: memory_score,
            eviction_health: eviction_score,
            recommendations: self.generate_recommendations(&stats),
        }
    }

    fn update_global_stats(&self) {
        let result_stats = self.result_cache.stats();
        let plan_stats = self.plan_cache.stats();
        let subquery_stats = self.subquery_cache.stats();

        let mut global_stats = self.global_stats.write().unwrap();
        global_stats.total_hits =
            result_stats.l1_hits + result_stats.l2_hits + plan_stats.hits + subquery_stats.hits;
        global_stats.total_misses = result_stats.misses + plan_stats.misses + subquery_stats.misses;
        global_stats.total_evictions = result_stats.evictions + plan_stats.evictions;
        global_stats.total_entries = plan_stats.current_entries + subquery_stats.current_entries;
        global_stats.total_memory_bytes =
            plan_stats.current_memory_bytes + subquery_stats.memory_bytes;
    }

    fn record_event(&self, event: CacheEvent) {
        let mut events = self.events.write().unwrap();

        if events.len() >= self.max_events {
            events.remove(0); // Remove oldest event
        }

        events.push(event);
    }

    fn generate_recommendations(&self, stats: &CacheManagerStats) -> Vec<String> {
        let mut recommendations = Vec::new();

        if stats.global.overall_hit_rate() < 0.3 {
            recommendations.push("Hit rate is low (<30%). Consider increasing cache sizes or reviewing query patterns.".to_string());
        }

        if stats.efficiency.eviction_rate > 0.1 {
            recommendations.push("High eviction rate (>10%). Consider increasing memory limits or adjusting TTL settings.".to_string());
        }

        if stats.efficiency.memory_utilization > 0.9 {
            recommendations.push("Memory utilization is high (>90%). Consider increasing max memory or enabling compression.".to_string());
        }

        if stats.result_cache.l1_hit_rate() < 0.1 {
            recommendations.push(
                "L1 cache hit rate is very low. Consider adjusting L1 size or TTL settings."
                    .to_string(),
            );
        }

        recommendations
    }
}

/// Comprehensive cache statistics
#[derive(Debug, Clone)]
pub struct CacheManagerStats {
    pub global: GlobalCacheStats,
    pub result_cache: ResultCacheStats,
    pub plan_cache: PlanCacheStats,
    pub subquery_cache: Option<SubqueryCacheStats>,
    pub efficiency: CacheEfficiencyMetrics,
    pub config: CacheConfig,
    pub graph_version: u64,
    pub schema_version: u64,
}

/// Cache health assessment
#[derive(Debug, Clone)]
pub struct CacheHealthScore {
    pub overall: f64,
    pub hit_rate: f64,
    pub memory_efficiency: f64,
    pub eviction_health: f64,
    pub recommendations: Vec<String>,
}
