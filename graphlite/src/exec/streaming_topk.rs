// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Streaming Top-K - Memory-efficient ORDER BY + LIMIT (Phase 4: Week 6.5)
//!
//! Maintains only top-K results using a min-heap instead of sorting entire result set.

use crate::exec::result::Row;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Streaming top-K results using min-heap
///
/// **Planned Feature** - Memory-efficient ORDER BY + LIMIT implementation
/// See ROADMAP.md: "Streaming Top-K Operations"
/// Target: v0.2.0 (High Priority)
///
/// This structure maintains only the K highest-scoring results without
/// materializing the entire result set in memory.
///
/// # Phase 4: Memory Optimization
///
/// Without StreamingTopK:
/// ```ignore
/// let mut results = collect_all_results(); // 1M rows in memory
/// results.sort_by(|a, b| score(b).cmp(score(a))); // Sort all 1M
/// results.truncate(10); // Only use 10
/// // Memory wasted: 999,990 rows
/// ```
///
/// With StreamingTopK:
/// ```ignore
/// let mut topk = StreamingTopK::new(10);
/// for row in results_iter {
///     topk.add(row, score); // Maintains heap of size ≤ 10
/// }
/// let results = topk.into_results(); // Already sorted, only 10 rows
/// // Memory used: 10 rows + heap overhead
/// ```
///
/// # Algorithm
///
/// Uses a min-heap where:
/// - Smallest score is at the root
/// - When heap size > K, remove minimum
/// - Result: heap contains K highest scores
///
/// # Performance
///
/// - **Time complexity**: O(N log K) where N = total rows, K = limit
/// - **Space complexity**: O(K) - only K rows in memory
/// - **Improvement over sort**: O(N log N) time, O(N) space
///
/// For N=1M, K=100:
/// - Traditional sort: ~20M comparisons, 1M rows in memory
/// - StreamingTopK: ~20M comparisons, 100 rows in memory (10,000x less)
#[allow(dead_code)]
pub struct StreamingTopK {
    /// Min-heap to maintain top-K results
    /// Root contains the minimum score in the heap
    heap: BinaryHeap<ScoredRow>,

    /// Maximum size (K)
    k: usize,

    /// Total rows processed (for statistics)
    processed_count: usize,

    /// Total rows that would have been kept without limit
    would_keep_count: usize,
}

/// Row with associated score for heap ordering
#[allow(dead_code)]
#[derive(Clone)]
struct ScoredRow {
    row: Row,
    score: f64,
}

impl Ord for ScoredRow {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap
        // (BinaryHeap is max-heap by default, we want min-heap)
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for ScoredRow {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for ScoredRow {}

impl PartialEq for ScoredRow {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl StreamingTopK {
    /// Create a new StreamingTopK with capacity K
    ///
    /// # Arguments
    /// - `k`: Maximum number of results to keep
    ///
    /// # Example
    /// ```ignore
    /// let mut topk = StreamingTopK::new(10); // Keep top 10
    /// for row in rows {
    ///     topk.add(row, score);
    /// }
    /// let top_10 = topk.into_results();
    /// ```
    #[allow(dead_code)] // ROADMAP v0.5.0 - Streaming top-K for memory-efficient result limiting
    pub fn new(k: usize) -> Self {
        Self {
            heap: BinaryHeap::with_capacity(k + 1), // +1 for temporary overflow
            k,
            processed_count: 0,
            would_keep_count: 0,
        }
    }

    /// Add a row to the top-K
    ///
    /// If the heap is full and this score is lower than the minimum,
    /// the row is discarded. Otherwise, it's added and the minimum is removed.
    ///
    /// # Arguments
    /// - `row`: The row to potentially add
    /// - `score`: The relevance score for ranking
    ///
    /// # Time Complexity
    /// - Best case: O(1) if score below minimum
    /// - Worst case: O(log K) for heap operations
    #[allow(dead_code)] // ROADMAP v0.5.0 - Add rows to top-K heap for streaming filtering
    pub fn add(&mut self, row: Row, score: f64) {
        self.processed_count += 1;

        // If heap not full, always add
        if self.heap.len() < self.k {
            self.heap.push(ScoredRow { row, score });
            self.would_keep_count += 1;
            return;
        }

        // Heap is full - check if this score beats minimum
        if let Some(min_item) = self.heap.peek() {
            if score > min_item.score {
                // This row is better than current minimum
                self.heap.push(ScoredRow { row, score });
                self.heap.pop(); // Remove minimum
                self.would_keep_count += 1;
            }
            // else: score <= min, discard this row
        }
    }

    /// Get the current minimum score in the heap
    ///
    /// This is the threshold - any row with score ≤ this can be discarded.
    #[allow(dead_code)] // ROADMAP v0.5.0 - Minimum score threshold for early filtering
    pub fn min_score(&self) -> Option<f64> {
        self.heap.peek().map(|item| item.score)
    }

    /// Get current heap size
    #[allow(dead_code)] // ROADMAP v0.5.0 - Current top-K result count
    pub fn len(&self) -> usize {
        self.heap.len()
    }

    /// Check if heap is empty
    #[allow(dead_code)] // ROADMAP v0.5.0 - Empty check for top-K processing
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Get statistics about processing
    #[allow(dead_code)] // ROADMAP v0.5.0 - Performance statistics for top-K optimization
    pub fn stats(&self) -> TopKStats {
        TopKStats {
            processed_count: self.processed_count,
            kept_count: self.heap.len(),
            would_keep_count: self.would_keep_count,
            k: self.k,
        }
    }

    /// Convert to sorted results (descending by score)
    ///
    /// Consumes the StreamingTopK and returns the top-K rows
    /// sorted in descending order by score.
    ///
    /// # Time Complexity
    /// O(K log K) for sorting K results
    #[allow(dead_code)] // ROADMAP v0.5.0 - Streaming TopK result extraction
    pub fn into_results(self) -> Vec<Row> {
        let mut results: Vec<ScoredRow> = self.heap.into_iter().collect();

        // Sort descending by score
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

        // Extract rows
        results.into_iter().map(|sr| sr.row).collect()
    }

    /// Get results without consuming (clones rows)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Streaming TopK results accessor
    pub fn results(&self) -> Vec<Row> {
        let mut results: Vec<ScoredRow> = self.heap.iter().cloned().collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));

        results.into_iter().map(|sr| sr.row).collect()
    }
}

/// Statistics about StreamingTopK processing
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.5.0 - Performance metrics for streaming top-K optimization
pub struct TopKStats {
    /// Total rows processed
    pub processed_count: usize,

    /// Rows currently in heap
    pub kept_count: usize,

    /// Rows that would be kept without limit (for analysis)
    pub would_keep_count: usize,

    /// K parameter
    pub k: usize,
}

impl TopKStats {
    /// Calculate memory savings ratio
    ///
    /// Returns how much memory was saved compared to keeping all rows.
    /// 1.0 = no savings, 0.1 = 90% savings
    #[allow(dead_code)] // ROADMAP v0.5.0 - Memory efficiency metrics for performance analysis
    pub fn memory_savings_ratio(&self) -> f64 {
        if self.processed_count == 0 {
            return 1.0;
        }
        self.kept_count as f64 / self.processed_count as f64
    }

    /// Calculate discard ratio
    ///
    /// Percentage of rows that were discarded.
    #[allow(dead_code)] // ROADMAP v0.5.0 - Discard rate metrics for optimization tuning
    pub fn discard_ratio(&self) -> f64 {
        if self.processed_count == 0 {
            return 0.0;
        }
        (self.processed_count - self.kept_count) as f64 / self.processed_count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Value;
    use std::collections::HashMap;

    fn create_test_row(id: usize, score: f64) -> Row {
        let mut values = HashMap::new();
        values.insert("id".to_string(), Value::Number(id as f64));
        values.insert("score".to_string(), Value::Number(score));

        Row {
            values,
            positional_values: vec![],
            source_entities: HashMap::new(),
            text_score: Some(score),
            highlight_snippet: None,
        }
    }

    #[test]
    fn test_streaming_topk_basic() {
        let mut topk = StreamingTopK::new(3);

        // Add rows with different scores
        topk.add(create_test_row(1, 10.0), 10.0);
        topk.add(create_test_row(2, 20.0), 20.0);
        topk.add(create_test_row(3, 5.0), 5.0);
        topk.add(create_test_row(4, 15.0), 15.0);

        let results = topk.into_results();

        assert_eq!(results.len(), 3);

        // Should keep top 3: 20.0, 15.0, 10.0
        let scores: Vec<f64> = results
            .iter()
            .map(|r| r.get_text_score().unwrap())
            .collect();

        assert_eq!(scores[0], 20.0);
        assert_eq!(scores[1], 15.0);
        assert_eq!(scores[2], 10.0);
    }

    #[test]
    fn test_streaming_topk_with_many_rows() {
        let mut topk = StreamingTopK::new(10);

        // Add 100 rows with random scores
        for i in 0..100 {
            let score = (i * 7 % 100) as f64; // Pseudo-random
            topk.add(create_test_row(i, score), score);
        }

        let results = topk.into_results();

        assert_eq!(results.len(), 10);

        // Verify results are sorted descending
        for i in 0..results.len() - 1 {
            let score_i = results[i].get_text_score().unwrap();
            let score_j = results[i + 1].get_text_score().unwrap();
            assert!(score_i >= score_j);
        }
    }

    #[test]
    fn test_streaming_topk_min_score() {
        let mut topk = StreamingTopK::new(3);

        topk.add(create_test_row(1, 10.0), 10.0);
        topk.add(create_test_row(2, 20.0), 20.0);
        topk.add(create_test_row(3, 5.0), 5.0);

        // Min should be 5.0 (smallest in heap)
        assert_eq!(topk.min_score(), Some(5.0));

        // Add higher score, min should update
        topk.add(create_test_row(4, 15.0), 15.0);

        // Now min should be 10.0 (5.0 was removed)
        assert_eq!(topk.min_score(), Some(10.0));
    }

    #[test]
    fn test_streaming_topk_stats() {
        let mut topk = StreamingTopK::new(5);

        for i in 0..20 {
            topk.add(create_test_row(i, i as f64), i as f64);
        }

        let stats = topk.stats();

        assert_eq!(stats.processed_count, 20);
        assert_eq!(stats.kept_count, 5);
        assert_eq!(stats.k, 5);

        // Memory savings: kept 5 out of 20 = 25%
        assert!((stats.memory_savings_ratio() - 0.25).abs() < 0.01);

        // Discard ratio: discarded 15 out of 20 = 75%
        assert!((stats.discard_ratio() - 0.75).abs() < 0.01);
    }

    #[test]
    fn test_streaming_topk_empty() {
        let topk = StreamingTopK::new(10);

        assert!(topk.is_empty());
        assert_eq!(topk.len(), 0);
        assert_eq!(topk.min_score(), None);

        let results = topk.into_results();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_streaming_topk_fewer_than_k() {
        let mut topk = StreamingTopK::new(10);

        // Add only 5 rows when K=10
        for i in 0..5 {
            topk.add(create_test_row(i, i as f64), i as f64);
        }

        let results = topk.into_results();

        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_streaming_topk_duplicate_scores() {
        let mut topk = StreamingTopK::new(3);

        topk.add(create_test_row(1, 10.0), 10.0);
        topk.add(create_test_row(2, 10.0), 10.0);
        topk.add(create_test_row(3, 10.0), 10.0);
        topk.add(create_test_row(4, 5.0), 5.0);

        let results = topk.into_results();

        assert_eq!(results.len(), 3);

        // Should keep 3 rows with score 10.0
        let scores: Vec<f64> = results
            .iter()
            .map(|r| r.get_text_score().unwrap())
            .collect();

        assert_eq!(scores[0], 10.0);
        assert_eq!(scores[1], 10.0);
        assert_eq!(scores[2], 10.0);
    }
}
