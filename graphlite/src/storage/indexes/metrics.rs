// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Index performance metrics and monitoring system

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// Comprehensive metrics for index operations
#[derive(Debug)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection for performance monitoring
pub struct IndexMetrics {
    // Operation counters
    pub queries: AtomicU64,
    pub inserts: AtomicU64,
    pub updates: AtomicU64,
    pub deletes: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,

    // Timing measurements
    pub query_times: RwLock<Vec<Duration>>,
    pub insert_times: RwLock<Vec<Duration>>,
    pub update_times: RwLock<Vec<Duration>>,
    pub delete_times: RwLock<Vec<Duration>>,

    // Resource usage
    pub memory_bytes: AtomicUsize,
    pub disk_bytes: AtomicUsize,
    pub index_size_entries: AtomicUsize,

    // Error tracking
    pub errors: AtomicU64,
    pub error_types: RwLock<HashMap<String, u64>>,

    // Index-specific metrics
    pub index_name: String,
    pub index_type: String,
    pub created_at: DateTime<Utc>,
}

impl IndexMetrics {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics constructor for monitoring initialization
    pub fn new(index_name: String, index_type: String) -> Self {
        Self {
            queries: AtomicU64::new(0),
            inserts: AtomicU64::new(0),
            updates: AtomicU64::new(0),
            deletes: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            query_times: RwLock::new(Vec::new()),
            insert_times: RwLock::new(Vec::new()),
            update_times: RwLock::new(Vec::new()),
            delete_times: RwLock::new(Vec::new()),
            memory_bytes: AtomicUsize::new(0),
            disk_bytes: AtomicUsize::new(0),
            index_size_entries: AtomicUsize::new(0),
            errors: AtomicU64::new(0),
            error_types: RwLock::new(HashMap::new()),
            index_name,
            index_type,
            created_at: Utc::now(),
        }
    }

    /// Record a query operation
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_query(&self, duration: Duration) {
        self.queries.fetch_add(1, Ordering::Relaxed);
        let mut times = self.query_times.write().unwrap();
        times.push(duration);

        // Keep only last 1000 measurements for memory efficiency
        if times.len() > 1000 {
            let drain_count = times.len() - 1000;
            times.drain(0..drain_count);
        }
    }

    /// Record an insert operation
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_insert(&self, duration: Duration) {
        self.inserts.fetch_add(1, Ordering::Relaxed);
        let mut times = self.insert_times.write().unwrap();
        times.push(duration);

        if times.len() > 1000 {
            let drain_count = times.len() - 1000;
            times.drain(0..drain_count);
        }
    }

    /// Record an update operation
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_update(&self, duration: Duration) {
        self.updates.fetch_add(1, Ordering::Relaxed);
        let mut times = self.update_times.write().unwrap();
        times.push(duration);

        if times.len() > 1000 {
            let drain_count = times.len() - 1000;
            times.drain(0..drain_count);
        }
    }

    /// Record a delete operation
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_delete(&self, duration: Duration) {
        self.deletes.fetch_add(1, Ordering::Relaxed);
        let mut times = self.delete_times.write().unwrap();
        times.push(duration);

        if times.len() > 1000 {
            let drain_count = times.len() - 1000;
            times.drain(0..drain_count);
        }
    }

    /// Record a cache hit
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_cache_hit(&self) {
        self.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_cache_miss(&self) {
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn record_error(&self, error_type: &str) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        let mut error_types = self.error_types.write().unwrap();
        *error_types.entry(error_type.to_string()).or_insert(0) += 1;
    }

    /// Update resource usage
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn update_memory_usage(&self, bytes: usize) {
        self.memory_bytes.store(bytes, Ordering::Relaxed);
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn update_disk_usage(&self, bytes: usize) {
        self.disk_bytes.store(bytes, Ordering::Relaxed);
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn update_index_size(&self, entries: usize) {
        self.index_size_entries.store(entries, Ordering::Relaxed);
    }

    /// Get comprehensive statistics
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn get_stats(&self) -> IndexStats {
        let query_times = self.query_times.read().unwrap();
        let insert_times = self.insert_times.read().unwrap();
        let update_times = self.update_times.read().unwrap();
        let delete_times = self.delete_times.read().unwrap();
        let error_types = self.error_types.read().unwrap();

        IndexStats {
            index_name: self.index_name.clone(),
            index_type: self.index_type.clone(),
            created_at: self.created_at,

            // Operation counts
            total_queries: self.queries.load(Ordering::Relaxed),
            total_inserts: self.inserts.load(Ordering::Relaxed),
            total_updates: self.updates.load(Ordering::Relaxed),
            total_deletes: self.deletes.load(Ordering::Relaxed),
            total_errors: self.errors.load(Ordering::Relaxed),

            // Cache statistics
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            cache_hit_rate: self.calculate_cache_hit_rate(),

            // Timing statistics
            avg_query_time: self.calculate_average(&query_times),
            avg_insert_time: self.calculate_average(&insert_times),
            avg_update_time: self.calculate_average(&update_times),
            avg_delete_time: self.calculate_average(&delete_times),

            p50_query_time: self.calculate_percentile(&query_times, 0.50),
            p95_query_time: self.calculate_percentile(&query_times, 0.95),
            p99_query_time: self.calculate_percentile(&query_times, 0.99),

            // Resource usage
            memory_usage_bytes: self.memory_bytes.load(Ordering::Relaxed),
            disk_usage_bytes: self.disk_bytes.load(Ordering::Relaxed),
            index_size_entries: self.index_size_entries.load(Ordering::Relaxed),

            // Error breakdown
            error_breakdown: error_types.clone(),

            // Throughput calculations
            queries_per_second: self.calculate_qps(),
            inserts_per_second: self.calculate_ips(),
        }
    }

    fn calculate_average(&self, times: &[Duration]) -> Duration {
        if times.is_empty() {
            return Duration::from_millis(0);
        }

        let total_nanos: u128 = times.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / times.len() as u128) as u64)
    }

    fn calculate_percentile(&self, times: &[Duration], percentile: f64) -> Duration {
        if times.is_empty() {
            return Duration::from_millis(0);
        }

        let mut sorted_times: Vec<Duration> = times.to_vec();
        sorted_times.sort();

        let index = ((sorted_times.len() as f64 - 1.0) * percentile) as usize;
        sorted_times[index.min(sorted_times.len() - 1)]
    }

    fn calculate_cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed) as f64;
        let misses = self.cache_misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;

        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }

    fn calculate_qps(&self) -> f64 {
        let total_queries = self.queries.load(Ordering::Relaxed) as f64;
        let elapsed_seconds = Utc::now().signed_duration_since(self.created_at).num_seconds() as f64;

        if elapsed_seconds == 0.0 {
            0.0
        } else {
            total_queries / elapsed_seconds.max(1.0)
        }
    }

    fn calculate_ips(&self) -> f64 {
        let total_inserts = self.inserts.load(Ordering::Relaxed) as f64;
        let elapsed_seconds = Utc::now().signed_duration_since(self.created_at).num_seconds() as f64;

        if elapsed_seconds == 0.0 {
            0.0
        } else {
            total_inserts / elapsed_seconds.max(1.0)
        }
    }

    /// Reset all metrics (useful for testing)
    #[allow(dead_code)] // ROADMAP v0.4.0 - Index metrics collection (see ROADMAP.md §6)
    pub fn reset(&self) {
        self.queries.store(0, Ordering::Relaxed);
        self.inserts.store(0, Ordering::Relaxed);
        self.updates.store(0, Ordering::Relaxed);
        self.deletes.store(0, Ordering::Relaxed);
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);

        self.query_times.write().unwrap().clear();
        self.insert_times.write().unwrap().clear();
        self.update_times.write().unwrap().clear();
        self.delete_times.write().unwrap().clear();
        self.error_types.write().unwrap().clear();
    }
}

/// Comprehensive index statistics for monitoring and reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub index_name: String,
    pub index_type: String,
    pub created_at: DateTime<Utc>,

    // Operation counts
    pub total_queries: u64,
    pub total_inserts: u64,
    pub total_updates: u64,
    pub total_deletes: u64,
    pub total_errors: u64,

    // Cache statistics
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,

    // Timing statistics
    pub avg_query_time: Duration,
    pub avg_insert_time: Duration,
    pub avg_update_time: Duration,
    pub avg_delete_time: Duration,

    pub p50_query_time: Duration,
    pub p95_query_time: Duration,
    pub p99_query_time: Duration,

    // Resource usage
    pub memory_usage_bytes: usize,
    pub disk_usage_bytes: usize,
    pub index_size_entries: usize,

    // Error breakdown
    pub error_breakdown: HashMap<String, u64>,

    // Throughput
    pub queries_per_second: f64,
    pub inserts_per_second: f64,
}

impl IndexStats {
    /// Generate a human-readable summary
    #[allow(dead_code)] // ROADMAP v0.4.0 - Stats summary generation for reporting
    pub fn summary(&self) -> String {
        format!(
            "Index '{}' ({}) - {} queries, {} inserts, {:.2}ms avg query time, {:.1}% cache hit rate",
            self.index_name,
            self.index_type,
            self.total_queries,
            self.total_inserts,
            self.avg_query_time.as_secs_f64() * 1000.0,
            self.cache_hit_rate * 100.0
        )
    }

    /// Check if the index has performance issues
    #[allow(dead_code)] // ROADMAP v0.4.0 - Performance issue detection (see ROADMAP.md §6)
    pub fn has_performance_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();

        // Check for slow queries (>100ms average)
        if self.avg_query_time.as_millis() > 100 {
            issues.push(format!("Slow average query time: {}ms", self.avg_query_time.as_millis()));
        }

        // Check for low cache hit rate (<70%)
        if self.cache_hit_rate < 0.7 && (self.cache_hits + self.cache_misses) > 100 {
            issues.push(format!("Low cache hit rate: {:.1}%", self.cache_hit_rate * 100.0));
        }

        // Check for high error rate (>5%)
        let total_operations = self.total_queries + self.total_inserts + self.total_updates + self.total_deletes;
        if total_operations > 0 && (self.total_errors as f64 / total_operations as f64) > 0.05 {
            issues.push(format!("High error rate: {:.1}%", (self.total_errors as f64 / total_operations as f64) * 100.0));
        }

        issues
    }
}

/// Global metrics manager for all indexes
#[derive(Debug)]
#[allow(dead_code)] // ROADMAP v0.4.0 - Global metrics manager for system-wide monitoring
pub struct IndexMetricsManager {
    metrics: RwLock<HashMap<String, Arc<IndexMetrics>>>,
}

impl IndexMetricsManager {
    #[allow(dead_code)] // ROADMAP v0.4.0 - Metrics manager constructor
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new index for monitoring
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn register_index(&self, index_name: String, index_type: String) -> Arc<IndexMetrics> {
        let metrics = Arc::new(IndexMetrics::new(index_name.clone(), index_type));
        self.metrics.write().unwrap().insert(index_name, metrics.clone());
        metrics
    }

    /// Get metrics for a specific index
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn get_metrics(&self, index_name: &str) -> Option<Arc<IndexMetrics>> {
        self.metrics.read().unwrap().get(index_name).cloned()
    }

    /// Get all index metrics
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn get_all_metrics(&self) -> Vec<Arc<IndexMetrics>> {
        self.metrics.read().unwrap().values().cloned().collect()
    }

    /// Get summary statistics for all indexes
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn get_summary_stats(&self) -> Vec<IndexStats> {
        self.get_all_metrics()
            .into_iter()
            .map(|metrics| metrics.get_stats())
            .collect()
    }

    /// Remove metrics for an index (when index is dropped)
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn unregister_index(&self, index_name: &str) {
        self.metrics.write().unwrap().remove(index_name);
    }

    /// Generate a comprehensive monitoring report
    #[allow(dead_code)] // ROADMAP v0.4.0 - System-wide index metrics (see ROADMAP.md §6)
    pub fn generate_report(&self) -> MonitoringReport {
        let all_stats = self.get_summary_stats();

        MonitoringReport {
            timestamp: Utc::now(),
            total_indexes: all_stats.len(),
            index_stats: all_stats.clone(),
            system_summary: self.calculate_system_summary(&all_stats),
            performance_issues: self.identify_performance_issues(&all_stats),
        }
    }

    fn calculate_system_summary(&self, all_stats: &[IndexStats]) -> SystemSummary {
        let total_queries: u64 = all_stats.iter().map(|s| s.total_queries).sum();
        let total_inserts: u64 = all_stats.iter().map(|s| s.total_inserts).sum();
        let total_memory: usize = all_stats.iter().map(|s| s.memory_usage_bytes).sum();
        let total_disk: usize = all_stats.iter().map(|s| s.disk_usage_bytes).sum();
        let total_entries: usize = all_stats.iter().map(|s| s.index_size_entries).sum();

        let avg_cache_hit_rate = if all_stats.is_empty() {
            0.0
        } else {
            all_stats.iter().map(|s| s.cache_hit_rate).sum::<f64>() / all_stats.len() as f64
        };

        SystemSummary {
            total_queries,
            total_inserts,
            total_memory_bytes: total_memory,
            total_disk_bytes: total_disk,
            total_index_entries: total_entries,
            average_cache_hit_rate: avg_cache_hit_rate,
        }
    }

    fn identify_performance_issues(&self, all_stats: &[IndexStats]) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        for stats in all_stats {
            let index_issues = stats.has_performance_issues();
            for issue in index_issues {
                issues.push(PerformanceIssue {
                    index_name: stats.index_name.clone(),
                    issue_type: "performance".to_string(),
                    description: issue.clone(),
                    severity: self.classify_severity(&issue),
                });
            }
        }

        issues
    }

    fn classify_severity(&self, issue: &str) -> String {
        if issue.contains("error rate") {
            "high".to_string()
        } else if issue.contains("slow") {
            "medium".to_string()
        } else {
            "low".to_string()
        }
    }
}

impl Default for IndexMetricsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// System-wide summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSummary {
    pub total_queries: u64,
    pub total_inserts: u64,
    pub total_memory_bytes: usize,
    pub total_disk_bytes: usize,
    pub total_index_entries: usize,
    pub average_cache_hit_rate: f64,
}

/// Performance issue identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    pub index_name: String,
    pub issue_type: String,
    pub description: String,
    pub severity: String,
}

/// Comprehensive monitoring report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringReport {
    pub timestamp: DateTime<Utc>,
    pub total_indexes: usize,
    pub index_stats: Vec<IndexStats>,
    pub system_summary: SystemSummary,
    pub performance_issues: Vec<PerformanceIssue>,
}

impl MonitoringReport {
    /// Export report as JSON
    #[allow(dead_code)] // ROADMAP v0.4.0 - JSON export for monitoring reports
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate a human-readable summary
    #[allow(dead_code)] // ROADMAP v0.4.0 - Monitoring report summary (see ROADMAP.md §6)
    pub fn summary(&self) -> String {
        format!(
            "Index Monitoring Report ({})\n\
             Total Indexes: {}\n\
             Total Queries: {}\n\
             Total Memory: {:.2} MB\n\
             Average Cache Hit Rate: {:.1}%\n\
             Performance Issues: {}",
            self.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.total_indexes,
            self.system_summary.total_queries,
            self.system_summary.total_memory_bytes as f64 / 1024.0 / 1024.0,
            self.system_summary.average_cache_hit_rate * 100.0,
            self.performance_issues.len()
        )
    }
}

/// Utility for timing operations
#[allow(dead_code)] // ROADMAP v0.4.0 - Operation timing utility for metrics collection
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    #[allow(dead_code)] // ROADMAP v0.4.0 - Timer elapsed accessor (see ROADMAP.md §6)
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}
