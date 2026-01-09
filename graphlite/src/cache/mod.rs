// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Comprehensive caching system
//!
//! This module provides multi-level caching for:
//! - Query results
//! - Compiled query plans
//! - Subquery results
//! - Metadata lookups
//! - Statistics and cardinality estimates

pub mod cache_config;
pub mod cache_manager;
pub mod invalidation;
pub mod plan_cache;
pub mod result_cache;
pub mod subquery_cache;

pub use cache_config::{CacheConfig, EvictionPolicy};
pub use cache_manager::CacheManager;
pub use invalidation::{InvalidationEvent, InvalidationManager};
pub use plan_cache::{PlanCache, PlanCacheEntry, PlanCacheKey};
pub use result_cache::ResultCache;
pub use subquery_cache::{
    SubqueryCache, SubqueryCacheHit, SubqueryResult, SubqueryType,
};

use std::time::{Duration, Instant};

/// Cache levels for different types of data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheLevel {
    /// L1: Hot data, frequently accessed (in-memory, small, fast)
    L1,
    /// L2: Warm data, occasionally accessed (in-memory, larger, moderate speed)
    L2,
    /// L3: Cold data, infrequently accessed (disk-backed, large, slower)
    L3,
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheEntryMetadata {
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: u32,
    pub size_bytes: usize,
    pub ttl: Option<Duration>,
    pub level: CacheLevel,
    pub tags: Vec<String>, // For cache invalidation
}

impl CacheEntryMetadata {
    pub fn new(size_bytes: usize, level: CacheLevel) -> Self {
        let now = Instant::now();
        Self {
            created_at: now,
            last_accessed: now,
            access_count: 0,
            size_bytes,
            ttl: None,
            level,
            tags: Vec::new(),
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl {
            self.created_at.elapsed() > ttl
        } else {
            false
        }
    }

    pub fn update_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Generic cache key trait
pub trait CacheKey:
    std::fmt::Debug + Clone + PartialEq + Eq + std::hash::Hash + Send + Sync
{
    #[allow(dead_code)] // ROADMAP v0.5.0 - Cache key generation for query caching (see ROADMAP.md §9)
    fn cache_key(&self) -> String;
    fn tags(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Generic cached value trait
pub trait CacheValue: std::fmt::Debug + Clone + Send + Sync {
    fn size_bytes(&self) -> usize;
    fn is_valid(&self) -> bool {
        true
    }
}
