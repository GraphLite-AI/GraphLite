// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Benchmarking suite for text index performance
//!
//! Measures:
//! - Index build time and throughput
//! - Search latency (P50, P95, P99)
//! - Memory usage
//! - Query throughput

use crate::storage::indexes::text::errors::TextSearchError;
use crate::storage::indexes::text::inverted_tantivy_clean::InvertedIndex;
use crate::storage::indexes::text::performance::{PerformanceConfig, PerformanceOptimizedIndex};
use std::sync::Arc;
use std::time::Instant;

/// Benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResults {
    /// Test name
    pub test_name: String,
    /// Number of documents
    pub doc_count: usize,
    /// Build time in milliseconds
    pub build_time_ms: f64,
    /// Throughput in docs/second
    pub build_throughput: f64,
    /// Search latency percentiles
    pub search_latency: SearchLatencyMetrics,
    /// Query throughput in QPS
    pub query_throughput_qps: f64,
    /// Peak memory estimate in bytes
    pub peak_memory_bytes: u64,
}

/// Search latency metrics
#[derive(Debug, Clone)]
pub struct SearchLatencyMetrics {
    /// P50 latency in milliseconds
    pub p50_ms: f64,
    /// P95 latency in milliseconds
    pub p95_ms: f64,
    /// P99 latency in milliseconds
    pub p99_ms: f64,
    /// Mean latency in milliseconds
    pub mean_ms: f64,
    /// Max latency in milliseconds
    pub max_ms: f64,
}

/// Index benchmarker
pub struct IndexBenchmark {
    index: Arc<InvertedIndex>,
    optimized: PerformanceOptimizedIndex,
}

impl IndexBenchmark {
    /// Create a new benchmark for an index
    pub fn new(index: Arc<InvertedIndex>) -> Result<Self, TextSearchError> {
        let config = PerformanceConfig::default();
        let optimized = PerformanceOptimizedIndex::new(index.clone(), config)?;

        Ok(Self { index, optimized })
    }

    /// Benchmark index building with given document count
    pub fn benchmark_build(&self, doc_count: usize) -> Result<BenchmarkResults, TextSearchError> {
        let start = Instant::now();

        // Generate and index documents
        for i in 0..doc_count {
            let content = format!("Document {} with some sample text for indexing", i);
            self.optimized.add_document_batched(i as u64, content)?;
        }

        // Flush remaining documents
        self.optimized.flush_batch()?;

        let build_time = start.elapsed().as_secs_f64() * 1000.0; // Convert to ms
        let build_throughput = (doc_count as f64) / (build_time / 1000.0);

        // Verify document count
        let indexed_count = self.index.doc_count()? as usize;

        // Placeholder for memory measurement (would need sys-info crate for real measurement)
        let peak_memory_bytes = (indexed_count as u64) * 500; // Rough estimate: 500 bytes per doc

        Ok(BenchmarkResults {
            test_name: "build_benchmark".to_string(),
            doc_count: indexed_count,
            build_time_ms: build_time,
            build_throughput,
            search_latency: SearchLatencyMetrics {
                p50_ms: 0.0,
                p95_ms: 0.0,
                p99_ms: 0.0,
                mean_ms: 0.0,
                max_ms: 0.0,
            },
            query_throughput_qps: 0.0,
            peak_memory_bytes,
        })
    }

    /// Benchmark search queries
    pub fn benchmark_search(
        &self,
        query: &str,
        iterations: usize,
        limit: Option<usize>,
    ) -> Result<BenchmarkResults, TextSearchError> {
        let mut latencies = Vec::new();

        // Execute queries
        for _ in 0..iterations {
            let start = Instant::now();
            let _ = self.optimized.search_optimized(query, limit)?;
            let latency = start.elapsed().as_secs_f64() * 1000.0; // Convert to ms
            latencies.push(latency);
        }

        // Calculate percentiles
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p50_idx = (latencies.len() as f64 * 0.50) as usize;
        let p95_idx = (latencies.len() as f64 * 0.95) as usize;
        let p99_idx = (latencies.len() as f64 * 0.99) as usize;

        let mean_ms = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let max_ms = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let p50_ms = latencies.get(p50_idx).copied().unwrap_or(0.0);
        let p95_ms = latencies.get(p95_idx).copied().unwrap_or(0.0);
        let p99_ms = latencies.get(p99_idx).copied().unwrap_or(0.0);

        let total_time = latencies.iter().sum::<f64>() / 1000.0; // Convert back to seconds
        let query_throughput_qps = iterations as f64 / total_time;

        let doc_count = self.index.doc_count()? as usize;
        let peak_memory_bytes = (doc_count as u64) * 500;

        Ok(BenchmarkResults {
            test_name: format!("search_benchmark: '{}'", query),
            doc_count,
            build_time_ms: 0.0,
            build_throughput: 0.0,
            search_latency: SearchLatencyMetrics {
                p50_ms,
                p95_ms,
                p99_ms,
                mean_ms,
                max_ms,
            },
            query_throughput_qps,
            peak_memory_bytes,
        })
    }

    /// Run comprehensive benchmark suite
    pub fn run_full_suite(&self, doc_count: usize) -> Result<FullSuitResults, TextSearchError> {
        // Build benchmark
        let build_results = self.benchmark_build(doc_count)?;

        // Search benchmarks
        let search_results_simple = self.benchmark_search("document", 100, None)?;
        let search_results_limited = self.benchmark_search("document", 100, Some(10))?;

        Ok(FullSuitResults {
            build: build_results,
            search_unlimited: search_results_simple,
            search_limited: search_results_limited,
        })
    }

    /// Format benchmark results for display
    pub fn format_results(results: &BenchmarkResults) -> String {
        let mut output = String::new();
        output.push_str(&format!("Benchmark: {}\n", results.test_name));
        output.push_str(&format!("  Documents: {}\n", results.doc_count));

        if results.build_time_ms > 0.0 {
            output.push_str(&format!("  Build time: {:.2} ms\n", results.build_time_ms));
            output.push_str(&format!(
                "  Build throughput: {:.0} docs/sec\n",
                results.build_throughput
            ));
        }

        if results.search_latency.p50_ms > 0.0 {
            output.push_str(&format!(
                "  Search latency - P50: {:.2} ms, P95: {:.2} ms, P99: {:.2} ms\n",
                results.search_latency.p50_ms,
                results.search_latency.p95_ms,
                results.search_latency.p99_ms
            ));
            output.push_str(&format!(
                "  Search latency - Mean: {:.2} ms, Max: {:.2} ms\n",
                results.search_latency.mean_ms, results.search_latency.max_ms
            ));
            output.push_str(&format!(
                "  Query throughput: {:.0} QPS\n",
                results.query_throughput_qps
            ));
        }

        output.push_str(&format!(
            "  Peak memory: {:.2} MB\n",
            results.peak_memory_bytes as f64 / 1_000_000.0
        ));

        output
    }
}

/// Full benchmark suite results
#[derive(Debug, Clone)]
pub struct FullSuitResults {
    pub build: BenchmarkResults,
    pub search_unlimited: BenchmarkResults,
    pub search_limited: BenchmarkResults,
}

impl FullSuitResults {
    /// Check if results meet performance targets
    pub fn meets_targets(&self) -> PerformanceTargets {
        let build_target = self.build.build_time_ms < 60_000.0; // <1 min for 100K docs
        let search_p50_target = self.search_unlimited.search_latency.p50_ms < 50.0;
        let search_p99_target = self.search_unlimited.search_latency.p99_ms < 200.0;
        let throughput_target = self.search_unlimited.query_throughput_qps > 500.0;

        // Memory target: peak memory should be less than or equal to 1.3x (30% overhead) of baseline
        // Baseline estimate: 100 bytes per doc minimum
        let baseline_memory = (self.build.doc_count as u64) * 100;
        let memory_target = self.build.peak_memory_bytes <= (baseline_memory as f64 * 1.3) as u64;

        PerformanceTargets {
            build_time_ok: build_target,
            search_p50_ok: search_p50_target,
            search_p99_ok: search_p99_target,
            throughput_ok: throughput_target,
            memory_ok: memory_target,
        }
    }

    /// Format full suite results
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Full Benchmark Suite Results ===\n\n");
        output.push_str(&IndexBenchmark::format_results(&self.build));
        output.push_str("\n");
        output.push_str(&IndexBenchmark::format_results(&self.search_unlimited));
        output.push_str("\n");
        output.push_str(&IndexBenchmark::format_results(&self.search_limited));

        let targets = self.meets_targets();
        output.push_str("\n=== Performance Targets ===\n");
        output.push_str(&format!(
            "Build time (<1 min for 100K): {}\n",
            if targets.build_time_ok { "✓" } else { "✗" }
        ));
        output.push_str(&format!(
            "Search P50 (<50ms): {}\n",
            if targets.search_p50_ok { "✓" } else { "✗" }
        ));
        output.push_str(&format!(
            "Search P99 (<200ms): {}\n",
            if targets.search_p99_ok { "✓" } else { "✗" }
        ));
        output.push_str(&format!(
            "Throughput (>500 QPS): {}\n",
            if targets.throughput_ok { "✓" } else { "✗" }
        ));
        output.push_str(&format!(
            "Memory (<30% overhead): {}\n",
            if targets.memory_ok { "✓" } else { "✗" }
        ));

        output
    }
}

/// Performance target validation
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    pub build_time_ok: bool,
    pub search_p50_ok: bool,
    pub search_p99_ok: bool,
    pub throughput_ok: bool,
    pub memory_ok: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_creation() {
        let index = Arc::new(InvertedIndex::new("test").unwrap());
        let benchmark = IndexBenchmark::new(index).unwrap();
        assert!(benchmark.index.name().contains("test"));
    }

    #[test]
    fn test_search_latency_metrics_creation() {
        let metrics = SearchLatencyMetrics {
            p50_ms: 10.0,
            p95_ms: 30.0,
            p99_ms: 50.0,
            mean_ms: 20.0,
            max_ms: 100.0,
        };

        assert!(metrics.p50_ms < metrics.p99_ms);
        assert!(metrics.mean_ms > 0.0);
    }

    #[test]
    fn test_benchmark_results_formatting() {
        let results = BenchmarkResults {
            test_name: "test_benchmark".to_string(),
            doc_count: 1000,
            build_time_ms: 500.0,
            build_throughput: 2000.0,
            search_latency: SearchLatencyMetrics {
                p50_ms: 10.0,
                p95_ms: 30.0,
                p99_ms: 50.0,
                mean_ms: 20.0,
                max_ms: 100.0,
            },
            query_throughput_qps: 1000.0,
            peak_memory_bytes: 1_000_000,
        };

        let formatted = IndexBenchmark::format_results(&results);
        assert!(formatted.contains("test_benchmark"));
        assert!(formatted.contains("1000"));
        assert!(formatted.contains("500.00 ms"));
    }

    #[test]
    fn test_performance_targets_validation() {
        let results = FullSuitResults {
            build: BenchmarkResults {
                test_name: "build".to_string(),
                doc_count: 100_000,
                build_time_ms: 30_000.0,
                build_throughput: 3_333.0,
                search_latency: SearchLatencyMetrics {
                    p50_ms: 0.0,
                    p95_ms: 0.0,
                    p99_ms: 0.0,
                    mean_ms: 0.0,
                    max_ms: 0.0,
                },
                query_throughput_qps: 0.0,
                peak_memory_bytes: 13_000_000, // ~130 bytes per doc (within 30% overhead of 100 byte baseline)
            },
            search_unlimited: BenchmarkResults {
                test_name: "search".to_string(),
                doc_count: 100_000,
                build_time_ms: 0.0,
                build_throughput: 0.0,
                search_latency: SearchLatencyMetrics {
                    p50_ms: 25.0,
                    p95_ms: 75.0,
                    p99_ms: 100.0,
                    mean_ms: 50.0,
                    max_ms: 150.0,
                },
                query_throughput_qps: 800.0,
                peak_memory_bytes: 13_000_000,
            },
            search_limited: BenchmarkResults {
                test_name: "search_limited".to_string(),
                doc_count: 100_000,
                build_time_ms: 0.0,
                build_throughput: 0.0,
                search_latency: SearchLatencyMetrics {
                    p50_ms: 15.0,
                    p95_ms: 40.0,
                    p99_ms: 60.0,
                    mean_ms: 25.0,
                    max_ms: 80.0,
                },
                query_throughput_qps: 1200.0,
                peak_memory_bytes: 13_000_000,
            },
        };

        let targets = results.meets_targets();
        assert!(targets.build_time_ok);
        assert!(targets.search_p50_ok);
        assert!(targets.memory_ok);
    }
}
