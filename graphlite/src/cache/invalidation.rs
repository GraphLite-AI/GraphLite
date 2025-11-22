// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Cache invalidation strategies and event handling

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Cache invalidation strategies
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Multi-strategy cache invalidation system
pub enum InvalidationStrategy {
    /// No automatic invalidation - manual only
    Manual,

    /// Time-based expiration with TTL
    TTL {
        #[allow(dead_code)]
        // ROADMAP v0.5.0 - TTL-based cache expiration with background cleanup task
        default_ttl: Duration,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Maximum TTL for cache entry lifetime limits
        max_ttl: Duration,
    },

    /// Tag-based invalidation (invalidate related entries)
    TagBased {
        /// Tags that trigger invalidation
        #[allow(dead_code)]
        // ROADMAP v0.5.0 - Tag-based selective invalidation for multi-tenant caching
        sensitive_tags: HashSet<String>,
        /// How long to wait before invalidating dependent entries
        #[allow(dead_code)] // ROADMAP v0.5.0 - Propagation delay for eventual consistency
        propagation_delay: Duration,
    },

    /// Version-based invalidation (schema/data versions)
    Versioned {
        /// Track data version changes
        #[allow(dead_code)]
        // ROADMAP v0.5.0 - Version tracking for data invalidation on writes
        track_data_version: bool,
        /// Track schema version changes
        #[allow(dead_code)]
        // ROADMAP v0.5.0 - Version tracking for schema evolution invalidation
        track_schema_version: bool,
    },

    /// Write-through invalidation (invalidate on writes)
    WriteThrough {
        /// Tables that trigger cache invalidation when modified
        #[allow(dead_code)] // ROADMAP v0.5.0 - Table-level write-through invalidation
        watched_tables: HashSet<String>,
        /// Invalidate dependent queries immediately
        #[allow(dead_code)] // ROADMAP v0.5.0 - Immediate vs delayed invalidation control
        immediate: bool,
    },

    /// Dependency-based invalidation
    DependencyBased {
        /// Maximum dependency chain depth to track
        #[allow(dead_code)]
        // ROADMAP v0.5.0 - Cascade depth limit for dependency invalidation
        max_depth: usize,
        /// Whether to invalidate transitively
        #[allow(dead_code)] // ROADMAP v0.5.0 - Transitive dependency invalidation control
        transitive: bool,
    },
}

/// Events that can trigger cache invalidation
#[derive(Debug, Clone)]
pub enum InvalidationEvent {
    /// Data modification events
    DataUpdate {
        table: String,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Row count for invalidation scope optimization
        affected_rows: u64,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Column-level invalidation for fine-grained control
        columns: Vec<String>,
    },

    /// Schema modification events
    SchemaChange {
        table: String,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Schema change type for selective invalidation
        change_type: SchemaChangeType,
    },

    /// Manual invalidation
    Manual {
        tags: Vec<String>,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Reason tracking for audit trail and debugging
        reason: String,
    },

    /// Memory pressure
    MemoryPressure {
        #[allow(dead_code)] // ROADMAP v0.5.0 - Memory usage tracking for LRU/LFU eviction
        current_usage: usize,
        #[allow(dead_code)] // ROADMAP v0.5.0 - Memory limit for automatic eviction trigger
        max_usage: usize,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Schema change event types for DDL invalidation
pub enum SchemaChangeType {
    TableCreated,
    TableDropped,
    ColumnAdded,
    ColumnDropped,
    ColumnModified,
    ConstraintAdded,
    ConstraintDropped,
}

/// Result of invalidation operation
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Invalidation result tracking for monitoring
pub struct InvalidationResult {
    pub entries_invalidated: usize,
    pub memory_freed: usize,
    pub duration: Duration,
    pub strategy_used: String,
    pub cascade_depth: usize,
}

/// Dependency tracking for cache entries
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Cache entry dependency tracking for automatic invalidation
pub struct CacheDependency {
    pub entry_key: String,
    pub depends_on: HashSet<String>, // Tables, schemas, etc.
    pub dependency_type: DependencyType,
    pub last_validated: Instant,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Dependency type classification for invalidation routing
pub enum DependencyType {
    Table(String),
    Schema(String),
    Index(String),
    Query(String),
    Expression(u64), // Hash of expression
}

/// Invalidation manager coordinating invalidation across caches
#[allow(dead_code)] // ROADMAP v0.5.0 - Central invalidation coordinator for cache coherence
pub struct InvalidationManager {
    strategy: InvalidationStrategy,

    /// Track dependencies between cache entries and data sources
    dependencies: Arc<RwLock<HashMap<String, CacheDependency>>>,

    /// Reverse index: data source -> dependent cache entries
    reverse_deps: Arc<RwLock<HashMap<String, HashSet<String>>>>,

    /// Invalidation event history
    event_history: Arc<RwLock<Vec<(InvalidationEvent, InvalidationResult, Instant)>>>,
    max_history_size: usize,

    /// Statistics
    stats: Arc<RwLock<InvalidationStats>>,
}

#[derive(Debug, Default, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Invalidation statistics for monitoring and tuning
pub struct InvalidationStats {
    pub total_events: u64,
    pub total_invalidations: u64,
    pub total_memory_freed: usize,
    pub cascade_invalidations: u64,
    pub average_cascade_depth: f64,
    pub false_positives: u64, // Invalidations that weren't necessary
}

impl InvalidationManager {
    pub fn new(strategy: InvalidationStrategy, max_history_size: usize) -> Self {
        Self {
            strategy,
            dependencies: Arc::new(RwLock::new(HashMap::new())),
            reverse_deps: Arc::new(RwLock::new(HashMap::new())),
            event_history: Arc::new(RwLock::new(Vec::new())),
            max_history_size,
            stats: Arc::new(RwLock::new(InvalidationStats::default())),
        }
    }

    /// Register a cache entry dependency
    #[allow(dead_code)] // ROADMAP v0.5.0 - Dependency registration when caching query results
    pub fn register_dependency(&self, entry_key: String, dependency: CacheDependency) {
        let dep_key = self.dependency_key(&dependency.dependency_type);

        {
            let mut dependencies = self.dependencies.write().unwrap();
            dependencies.insert(entry_key.clone(), dependency);
        }

        {
            let mut reverse_deps = self.reverse_deps.write().unwrap();
            reverse_deps.entry(dep_key).or_default().insert(entry_key);
        }
    }

    /// Handle invalidation event
    pub fn handle_event(&self, event: InvalidationEvent) -> InvalidationResult {
        let start_time = Instant::now();
        let mut _entries_invalidated = 0;
        let memory_freed = 0;
        let mut cascade_depth = 0;

        let affected_entries = match &event {
            InvalidationEvent::DataUpdate { table, .. } => {
                self.find_dependent_entries(&DependencyType::Table(table.clone()))
            }

            InvalidationEvent::SchemaChange { table, .. } => {
                let mut affected =
                    self.find_dependent_entries(&DependencyType::Table(table.clone()));
                affected
                    .extend(self.find_dependent_entries(&DependencyType::Schema(table.clone())));
                affected
            }

            InvalidationEvent::Manual { tags, .. } => self.find_entries_by_tags(tags),

            InvalidationEvent::MemoryPressure { .. } => {
                // For memory pressure, return empty set (handled separately by eviction logic)
                HashSet::new()
            }
        };

        _entries_invalidated = affected_entries.len();

        // Perform cascade invalidation if needed
        if self.should_cascade(&event) {
            let cascaded = self.cascade_invalidation(&affected_entries);
            _entries_invalidated += cascaded.len();
            cascade_depth = self.calculate_cascade_depth(&affected_entries, &cascaded);
        }

        let duration = start_time.elapsed();
        let result = InvalidationResult {
            entries_invalidated: _entries_invalidated,
            memory_freed,
            duration,
            strategy_used: self.strategy_name(),
            cascade_depth,
        };

        // Record event and result
        self.record_event_result(event, result.clone());

        // Update statistics
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_events += 1;
            stats.total_invalidations += _entries_invalidated as u64;
            stats.total_memory_freed += memory_freed;
            if cascade_depth > 0 {
                stats.cascade_invalidations += 1;
                stats.average_cascade_depth = (stats.average_cascade_depth
                    * (stats.cascade_invalidations - 1) as f64
                    + cascade_depth as f64)
                    / stats.cascade_invalidations as f64;
            }
        }

        result
    }

    /// Cleanup expired dependencies
    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache invalidation management (see ROADMAP.md §9)
    pub fn cleanup_expired_dependencies(&self, max_age: Duration) {
        let cutoff_time = Instant::now() - max_age;
        let mut expired_keys = Vec::new();

        {
            let dependencies = self.dependencies.read().unwrap();
            for (key, dep) in dependencies.iter() {
                if dep.last_validated < cutoff_time {
                    expired_keys.push(key.clone());
                }
            }
        }

        for key in expired_keys {
            self.remove_dependency(&key);
        }
    }

    /// Get invalidation statistics
    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache invalidation management (see ROADMAP.md §9)
    pub fn stats(&self) -> InvalidationStats {
        self.stats.read().unwrap().clone()
    }

    /// Get recent invalidation events
    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache invalidation management (see ROADMAP.md §9)
    pub fn recent_events(
        &self,
        limit: usize,
    ) -> Vec<(InvalidationEvent, InvalidationResult, Instant)> {
        let history = self.event_history.read().unwrap();
        let limit = limit.min(history.len());
        history.iter().rev().take(limit).cloned().collect()
    }

    fn find_dependent_entries(&self, dependency_type: &DependencyType) -> HashSet<String> {
        let dep_key = self.dependency_key(dependency_type);
        self.reverse_deps
            .read()
            .unwrap()
            .get(&dep_key)
            .cloned()
            .unwrap_or_default()
    }

    fn find_entries_by_tags(&self, _tags: &[String]) -> HashSet<String> {
        // In practice, would need tag tracking in cache entries
        // This is a simplified implementation
        HashSet::new()
    }

    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache invalidation management (see ROADMAP.md §9)
    fn find_version_sensitive_entries(&self) -> HashSet<String> {
        // Find entries that depend on version information
        let dependencies = self.dependencies.read().unwrap();
        dependencies.keys().cloned().collect()
    }

    fn should_cascade(&self, _event: &InvalidationEvent) -> bool {
        match &self.strategy {
            InvalidationStrategy::DependencyBased { transitive, .. } => *transitive,
            InvalidationStrategy::TagBased { .. } => true,
            _ => false,
        }
    }

    fn cascade_invalidation(&self, initial_entries: &HashSet<String>) -> HashSet<String> {
        let mut cascaded = HashSet::new();
        let mut to_process: Vec<String> = initial_entries.iter().cloned().collect();
        let mut processed = HashSet::new();

        let max_depth = match &self.strategy {
            InvalidationStrategy::DependencyBased { max_depth, .. } => *max_depth,
            _ => 3, // Default cascade depth
        };

        let mut current_depth = 0;

        while !to_process.is_empty() && current_depth < max_depth {
            let current_batch = to_process;
            to_process = Vec::new();
            current_depth += 1;

            for entry in current_batch {
                if processed.contains(&entry) {
                    continue;
                }
                processed.insert(entry.clone());

                // Find entries that depend on this entry
                let dependent_entries = self.find_entries_dependent_on(&entry);
                for dep in dependent_entries {
                    if !cascaded.contains(&dep) && !initial_entries.contains(&dep) {
                        cascaded.insert(dep.clone());
                        to_process.push(dep);
                    }
                }
            }
        }

        cascaded
    }

    fn find_entries_dependent_on(&self, _entry_key: &str) -> HashSet<String> {
        // This would require reverse dependency tracking
        // Simplified implementation
        HashSet::new()
    }

    fn calculate_cascade_depth(
        &self,
        initial: &HashSet<String>,
        cascaded: &HashSet<String>,
    ) -> usize {
        if cascaded.is_empty() {
            0
        } else {
            // Simplified - would need proper depth tracking
            (cascaded.len() as f64 / initial.len() as f64).ceil() as usize
        }
    }

    fn dependency_key(&self, dependency_type: &DependencyType) -> String {
        match dependency_type {
            DependencyType::Table(name) => format!("table:{}", name),
            DependencyType::Schema(name) => format!("schema:{}", name),
            DependencyType::Index(name) => format!("index:{}", name),
            DependencyType::Query(query) => format!("query:{}", query),
            DependencyType::Expression(hash) => format!("expr:{}", hash),
        }
    }

    fn strategy_name(&self) -> String {
        match &self.strategy {
            InvalidationStrategy::Manual => "Manual".to_string(),
            InvalidationStrategy::TTL { .. } => "TTL".to_string(),
            InvalidationStrategy::TagBased { .. } => "TagBased".to_string(),
            InvalidationStrategy::Versioned { .. } => "Versioned".to_string(),
            InvalidationStrategy::WriteThrough { .. } => "WriteThrough".to_string(),
            InvalidationStrategy::DependencyBased { .. } => "DependencyBased".to_string(),
        }
    }

    fn record_event_result(&self, event: InvalidationEvent, result: InvalidationResult) {
        let mut history = self.event_history.write().unwrap();

        if history.len() >= self.max_history_size {
            history.remove(0);
        }

        history.push((event, result, Instant::now()));
    }

    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache invalidation management (see ROADMAP.md §9)
    fn remove_dependency(&self, entry_key: &str) {
        if let Some(dependency) = self.dependencies.write().unwrap().remove(entry_key) {
            let dep_key = self.dependency_key(&dependency.dependency_type);

            let mut reverse_deps = self.reverse_deps.write().unwrap();
            if let Some(entries) = reverse_deps.get_mut(&dep_key) {
                entries.remove(entry_key);
                if entries.is_empty() {
                    reverse_deps.remove(&dep_key);
                }
            }
        }
    }
}

/// Helper functions for creating common invalidation strategies
impl InvalidationStrategy {
    /// Create TTL-based strategy with default settings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Default TTL strategy factory (30min default, 2hr max)
    pub fn default_ttl() -> Self {
        Self::TTL {
            default_ttl: Duration::from_secs(1800), // 30 minutes
            max_ttl: Duration::from_secs(7200),     // 2 hours
        }
    }

    /// Create tag-based strategy for graph databases
    #[allow(dead_code)] // ROADMAP v0.5.0 - Graph-specific tag-based invalidation strategy
    pub fn graph_tag_based() -> Self {
        let mut sensitive_tags = HashSet::new();
        sensitive_tags.insert("nodes".to_string());
        sensitive_tags.insert("edges".to_string());
        sensitive_tags.insert("schema".to_string());

        Self::TagBased {
            sensitive_tags,
            propagation_delay: Duration::from_millis(100),
        }
    }

    /// Create version-based strategy tracking both data and schema
    #[allow(dead_code)] // ROADMAP v0.5.0 - Full version tracking for data and schema changes
    pub fn full_versioned() -> Self {
        Self::Versioned {
            track_data_version: true,
            track_schema_version: true,
        }
    }

    /// Create dependency-based strategy with reasonable limits
    #[allow(dead_code)] // ROADMAP v0.5.0 - Dependency-based cascade invalidation (depth-3, transitive)
    pub fn dependency_tracking() -> Self {
        Self::DependencyBased {
            max_depth: 3,
            transitive: true,
        }
    }
}
