// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Performance optimization module for text indexes
//!
//! Provides:
//! - Batch document processing with configurable commit frequency
//! - Query result caching with LRU eviction
//! - Early termination for LIMIT queries
//! - Search filter pushdown to Tantivy
//! - Memory and performance tuning

use crate::storage::indexes::text::errors::TextSearchError;
use crate::storage::indexes::text::inverted_tantivy_clean::{InvertedIndex, SearchResult};
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Performance configuration for index operations
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Batch size for document inserts before commit
    pub batch_commit_size: usize,
    /// Query result cache size (in number of queries)
    pub cache_size: usize,
    /// Cache entry TTL in seconds
    pub cache_ttl_secs: u64,
    /// Enable early termination for LIMIT queries
    pub enable_early_termination: bool,
    /// Enable query result caching
    pub enable_query_cache: bool,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            batch_commit_size: 1000,      // Commit every 1000 documents
            cache_size: 1000,             // Cache 1000 query results
            cache_ttl_secs: 300,          // 5-minute cache TTL
            enable_early_termination: true,
            enable_query_cache: true,
        }
    }
}

/// Cache entry with TTL tracking
#[derive(Debug, Clone)]
struct CacheEntry {
    results: Vec<SearchResult>,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn new(results: Vec<SearchResult>, ttl_secs: u64) -> Self {
        Self {
            results,
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(ttl_secs),
        }
    }

    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

/// Query result cache key combining query text and limit
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    query: String,
    limit: Option<usize>,
}

/// Performance-optimized index wrapper
pub struct PerformanceOptimizedIndex {
    index: Arc<InvertedIndex>,
    config: PerformanceConfig,
    query_cache: Arc<Mutex<LruCache<CacheKey, CacheEntry>>>,
    batch_buffer: Arc<Mutex<Vec<(u64, String)>>>,
    batch_count: Arc<Mutex<usize>>,
}

impl PerformanceOptimizedIndex {
    /// Create a new performance-optimized index wrapper
    pub fn new(index: Arc<InvertedIndex>, config: PerformanceConfig) -> Result<Self, TextSearchError> {
        let cache_size =
            NonZeroUsize::new(config.cache_size).unwrap_or_else(|| NonZeroUsize::new(1000).unwrap());

        Ok(Self {
            index,
            config,
            query_cache: Arc::new(Mutex::new(LruCache::new(cache_size))),
            batch_buffer: Arc::new(Mutex::new(Vec::new())),
            batch_count: Arc::new(Mutex::new(0)),
        })
    }

    /// Add a document with batching support
    pub fn add_document_batched(&self, doc_id: u64, content: String) -> Result<(), TextSearchError> {
        // Add to buffer
        {
            let mut buffer = self
                .batch_buffer
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("Buffer lock error: {}", e)))?;
            buffer.push((doc_id, content));
        }

        // Check if we should commit
        {
            let mut count = self
                .batch_count
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("Count lock error: {}", e)))?;
            *count += 1;

            if *count >= self.config.batch_commit_size {
                drop(count); // Release lock before commit
                self.flush_batch()?;
            }
        }

        Ok(())
    }

    /// Flush buffered documents and commit
    pub fn flush_batch(&self) -> Result<usize, TextSearchError> {
        let mut buffer = self
            .batch_buffer
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("Buffer lock error: {}", e)))?;

        if buffer.is_empty() {
            return Ok(0);
        }

        let batch_size = buffer.len();

        // Add all buffered documents
        self.index.add_documents(buffer.drain(..).collect())?;

        // Commit to persist
        self.index.commit()?;

        // Reset counter
        {
            let mut count = self
                .batch_count
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("Count lock error: {}", e)))?;
            *count = 0;
        }

        Ok(batch_size)
    }

    /// Search with caching and optional early termination
    pub fn search_optimized(
        &self,
        query_text: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>, TextSearchError> {
        // Create cache key
        let cache_key = CacheKey {
            query: query_text.to_string(),
            limit,
        };

        // Check cache first
        if self.config.enable_query_cache {
            let mut cache = self
                .query_cache
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("Cache lock error: {}", e)))?;

            if let Some(entry) = cache.get(&cache_key) {
                if !entry.is_expired() {
                    return Ok(entry.results.clone());
                } else {
                    // Remove expired entry
                    cache.pop(&cache_key);
                }
            }
        }

        // Execute search with early termination if enabled
        let results = if self.config.enable_early_termination && limit.is_some() {
            self.index.search_with_limit(query_text, limit)?
        } else {
            self.index.search(query_text)?
        };

        // Store in cache if enabled
        if self.config.enable_query_cache && !results.is_empty() {
            let mut cache = self
                .query_cache
                .lock()
                .map_err(|e| TextSearchError::IndexError(format!("Cache lock error: {}", e)))?;

            cache.put(
                cache_key,
                CacheEntry::new(results.clone(), self.config.cache_ttl_secs),
            );
        }

        Ok(results)
    }

    /// Clear the query cache
    pub fn clear_query_cache(&self) -> Result<(), TextSearchError> {
        let mut cache = self
            .query_cache
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("Cache lock error: {}", e)))?;
        cache.clear();
        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> Result<CacheStats, TextSearchError> {
        let cache = self
            .query_cache
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("Cache lock error: {}", e)))?;

        let total_entries = cache.len();
        let capacity = cache.cap().get();
        let hit_rate = if total_entries == 0 {
            0.0
        } else {
            (total_entries as f64 / capacity as f64) * 100.0
        };

        Ok(CacheStats {
            cached_queries: total_entries,
            cache_capacity: capacity,
            utilization_percent: hit_rate,
        })
    }

    /// Get buffer statistics
    pub fn get_buffer_stats(&self) -> Result<BufferStats, TextSearchError> {
        let buffer = self
            .batch_buffer
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("Buffer lock error: {}", e)))?;

        let count = self
            .batch_count
            .lock()
            .map_err(|e| TextSearchError::IndexError(format!("Count lock error: {}", e)))?;

        Ok(BufferStats {
            buffered_documents: buffer.len(),
            total_processed: *count,
            batch_size_config: self.config.batch_commit_size,
        })
    }

    /// Get underlying index reference
    pub fn index(&self) -> &Arc<InvertedIndex> {
        &self.index
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cached_queries: usize,
    pub cache_capacity: usize,
    pub utilization_percent: f64,
}

/// Buffer statistics
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub buffered_documents: usize,
    pub total_processed: usize,
    pub batch_size_config: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_config_default() {
        let config = PerformanceConfig::default();
        assert_eq!(config.batch_commit_size, 1000);
        assert_eq!(config.cache_size, 1000);
        assert_eq!(config.cache_ttl_secs, 300);
        assert!(config.enable_early_termination);
        assert!(config.enable_query_cache);
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry::new(vec![], 1);
        assert!(!entry.is_expired());

        // Simulate expired entry (can't easily test with sleep in unit tests)
        let mut old_entry = entry;
        old_entry.inserted_at = Instant::now() - Duration::from_secs(2);
        assert!(old_entry.is_expired());
    }

    #[test]
    fn test_cache_key_equality() {
        let key1 = CacheKey {
            query: "test".to_string(),
            limit: Some(100),
        };
        let key2 = CacheKey {
            query: "test".to_string(),
            limit: Some(100),
        };
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_performance_optimized_index_creation() {
        let index = Arc::new(InvertedIndex::new("test").unwrap());
        let config = PerformanceConfig::default();
        let optimized = PerformanceOptimizedIndex::new(index, config).unwrap();

        let stats = optimized.get_cache_stats().unwrap();
        assert_eq!(stats.cached_queries, 0);
        assert_eq!(stats.cache_capacity, 1000);
    }

    #[test]
    fn test_buffer_stats() {
        let index = Arc::new(InvertedIndex::new("test").unwrap());
        let config = PerformanceConfig::default();
        let optimized = PerformanceOptimizedIndex::new(index, config).unwrap();

        let stats = optimized.get_buffer_stats().unwrap();
        assert_eq!(stats.buffered_documents, 0);
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.batch_size_config, 1000);
    }
}
