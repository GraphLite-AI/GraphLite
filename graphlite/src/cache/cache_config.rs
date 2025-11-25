// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Cache configuration and policies

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Global cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable/disable caching entirely
    pub enabled: bool,

    /// Maximum memory usage across all caches (bytes)
    pub max_memory_bytes: usize,

    /// L1 cache configuration (hot data)
    pub l1_config: LevelConfig,

    /// L2 cache configuration (warm data)
    pub l2_config: LevelConfig,

    /// L3 cache configuration (cold data)
    pub l3_config: LevelConfig,

    /// Global eviction policy
    pub eviction_policy: EvictionPolicy,

    /// Cache statistics collection interval
    pub stats_interval: Duration,

    /// Enable cache compression
    pub compression_enabled: bool,

    /// Invalidation strategy
    pub invalidation_strategy: InvalidationStrategy,
}

/// Configuration for a specific cache level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelConfig {
    /// Maximum number of entries
    pub max_entries: usize,

    /// Maximum memory for this level (bytes)
    pub max_memory_bytes: usize,

    /// Default TTL for entries
    pub default_ttl: Option<Duration>,

    /// Cache policy for this level
    pub policy: CachePolicy,
}

/// Cache policies determining behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachePolicy {
    /// Write-through: write to cache and storage simultaneously
    WriteThrough,
    /// Write-back: write to cache first, storage later
    WriteBack,
    /// Write-around: write to storage, bypass cache
    WriteAround,
    /// Read-through: read from storage if cache miss, populate cache
    ReadThrough,
}

/// Eviction policies for when cache is full
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    /// Least Recently Used
    Lru,
    /// Least Frequently Used
    Lfu,
    /// First In First Out
    Fifo,
    /// Random replacement
    Random,
    /// Time-based (oldest entries first)
    Ttl,
    /// Size-based (largest entries first)  
    Size,
    /// Adaptive Replacement Cache (balance recency/frequency)
    Arc,
}

/// Cache invalidation strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InvalidationStrategy {
    /// Manual invalidation only
    Manual,
    /// Time-based expiration
    Ttl,
    /// Tag-based invalidation (invalidate by graph/table changes)
    TagBased,
    /// Version-based invalidation
    Versioned,
    /// Hybrid approach combining multiple strategies
    Hybrid {
        primary: Box<InvalidationStrategy>,
        fallback: Box<InvalidationStrategy>,
    },
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_memory_bytes: 1024 * 1024 * 512, // 512MB
            l1_config: LevelConfig {
                max_entries: 1000,
                max_memory_bytes: 1024 * 1024 * 64, // 64MB
                default_ttl: Some(Duration::from_secs(300)), // 5 minutes
                policy: CachePolicy::ReadThrough,
            },
            l2_config: LevelConfig {
                max_entries: 5000,
                max_memory_bytes: 1024 * 1024 * 256, // 256MB
                default_ttl: Some(Duration::from_secs(1800)), // 30 minutes
                policy: CachePolicy::ReadThrough,
            },
            l3_config: LevelConfig {
                max_entries: 20000,
                max_memory_bytes: 1024 * 1024 * 192, // 192MB (remainder)
                default_ttl: Some(Duration::from_secs(3600)), // 1 hour
                policy: CachePolicy::WriteBack,
            },
            eviction_policy: EvictionPolicy::Arc,
            stats_interval: Duration::from_secs(60),
            compression_enabled: true,
            invalidation_strategy: InvalidationStrategy::Hybrid {
                primary: Box::new(InvalidationStrategy::TagBased),
                fallback: Box::new(InvalidationStrategy::Ttl),
            },
        }
    }
}

impl CacheConfig {
    /// Create configuration optimized for read-heavy workloads
    pub fn read_optimized() -> Self {
        Self {
            l1_config: LevelConfig {
                max_entries: 2000,
                default_ttl: Some(Duration::from_secs(600)), // 10 minutes
                ..Self::default().l1_config
            },
            l2_config: LevelConfig {
                max_entries: 10000,
                default_ttl: Some(Duration::from_secs(3600)), // 1 hour
                ..Self::default().l2_config
            },
            ..Self::default()
        }
    }

    /// Create configuration optimized for write-heavy workloads
    pub fn write_optimized() -> Self {
        let mut config = Self::default();
        config.l1_config.policy = CachePolicy::WriteBack;
        config.l2_config.policy = CachePolicy::WriteBack;
        config.eviction_policy = EvictionPolicy::Lru; // Simpler for write workloads
        config.invalidation_strategy = InvalidationStrategy::TagBased;
        config
    }

    /// Create configuration for memory-constrained environments
    pub fn memory_constrained() -> Self {
        let mut config = Self::default();
        config.max_memory_bytes = 1024 * 1024 * 128; // 128MB total
        config.l1_config.max_memory_bytes = 1024 * 1024 * 32; // 32MB
        config.l1_config.max_entries = 500;
        config.l2_config.max_memory_bytes = 1024 * 1024 * 64; // 64MB
        config.l2_config.max_entries = 2000;
        config.l3_config.max_memory_bytes = 1024 * 1024 * 32; // 32MB
        config.l3_config.max_entries = 5000;
        config.compression_enabled = true;
        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let total_memory = self.l1_config.max_memory_bytes
            + self.l2_config.max_memory_bytes
            + self.l3_config.max_memory_bytes;

        if total_memory > self.max_memory_bytes {
            return Err(format!(
                "Sum of level memory limits ({} bytes) exceeds max memory ({} bytes)",
                total_memory, self.max_memory_bytes
            ));
        }

        if self.l1_config.max_entries == 0
            || self.l2_config.max_entries == 0
            || self.l3_config.max_entries == 0
        {
            return Err("Cache levels must have max_entries > 0".to_string());
        }

        Ok(())
    }
}
