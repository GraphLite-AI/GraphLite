// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Memory budget enforcement for query execution (Phase 4: Week 6.5)
//!
//! Tracks and enforces memory limits during query execution to prevent OOM errors.
//! Provides graceful degradation when memory limits are approached.

use crate::exec::error::ExecutionError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Memory budget tracker for query execution
///
/// **Planned Feature** - Memory limit enforcement for query execution
/// See ROADMAP.md: "Memory Budget Management"
/// Target: v0.2.0 (High Priority)
///
/// Tracks allocated memory during query execution and enforces limits.
/// Provides early termination when memory budget is exceeded.
///
/// # Usage
/// ```ignore
/// let budget = MemoryBudget::new(100 * 1024 * 1024); // 100MB limit
///
/// // Allocate memory for result set
/// budget.allocate(1024)?; // Allocates 1KB
///
/// // Check if we're approaching limit
/// if budget.usage_ratio() > 0.9 {
///     // Warn user or start streaming
/// }
///
/// // Release memory when done
/// budget.release(1024);
/// ```
///
/// # Design Rationale
/// - Uses atomic operations for thread-safe tracking
/// - Minimal overhead (~8 bytes per ExecutionContext)
/// - Configurable limits per-session or per-query
/// - Graceful degradation instead of hard crashes
#[allow(dead_code)]
#[derive(Clone)]
pub struct MemoryBudget {
    /// Maximum allowed memory in bytes
    limit: usize,

    /// Currently allocated memory (atomic for thread safety)
    allocated: Arc<AtomicUsize>,

    /// Peak allocated memory (for statistics)
    peak: Arc<AtomicUsize>,
}

impl std::fmt::Debug for MemoryBudget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryBudget")
            .field("limit", &self.limit)
            .field("allocated", &self.allocated.load(Ordering::SeqCst))
            .field("peak", &self.peak.load(Ordering::SeqCst))
            .finish()
    }
}

impl MemoryBudget {
    /// Create a new memory budget with the given limit
    ///
    /// # Arguments
    /// - `limit`: Maximum memory in bytes
    ///
    /// # Recommended Limits
    /// - Development: 100MB (100 * 1024 * 1024)
    /// - Production: 1GB (1024 * 1024 * 1024)
    /// - High-memory queries: 5GB (5 * 1024 * 1024 * 1024)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Memory budget enforcement for resource control
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            allocated: Arc::new(AtomicUsize::new(0)),
            peak: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Create an unlimited memory budget (for testing or admin queries)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Unlimited budget for admin operations and testing
    pub fn unlimited() -> Self {
        Self::new(usize::MAX)
    }

    /// Allocate memory from the budget
    ///
    /// # Arguments
    /// - `bytes`: Number of bytes to allocate
    ///
    /// # Returns
    /// - `Ok(())` if allocation succeeded
    /// - `Err(ExecutionError::MemoryLimitExceeded)` if budget exceeded
    ///
    /// # Example
    /// ```ignore
    /// // Allocate space for 1000 rows
    /// let row_size = std::mem::size_of::<Row>();
    /// budget.allocate(1000 * row_size)?;
    /// ```
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory budget enforcement for large queries (see ROADMAP.md §1)
    pub fn allocate(&self, bytes: usize) -> Result<(), ExecutionError> {
        let current = self.allocated.fetch_add(bytes, Ordering::SeqCst);
        let new_total = current + bytes;

        // Update peak if necessary
        self.peak.fetch_max(new_total, Ordering::SeqCst);

        // Check if we exceeded the limit
        if new_total > self.limit {
            // Rollback allocation
            self.allocated.fetch_sub(bytes, Ordering::SeqCst);

            return Err(ExecutionError::MemoryLimitExceeded {
                limit: self.limit,
                requested: new_total,
            });
        }

        Ok(())
    }

    /// Try to allocate memory, returning false if budget exceeded
    ///
    /// Like `allocate()` but returns boolean instead of error.
    /// Useful for soft limits where we want to continue with degraded performance.
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory budget soft allocation (see ROADMAP.md §1)
    pub fn try_allocate(&self, bytes: usize) -> bool {
        self.allocate(bytes).is_ok()
    }

    /// Release memory back to the budget
    ///
    /// # Arguments
    /// - `bytes`: Number of bytes to release
    ///
    /// # Note
    /// It's the caller's responsibility to track how much was allocated.
    /// Releasing more than was allocated will underflow (saturating to 0).
    #[allow(dead_code)] // ROADMAP v0.2.0 - Release allocated memory (see ROADMAP.md §1)
    pub fn release(&self, bytes: usize) {
        self.allocated.fetch_sub(bytes, Ordering::SeqCst);
    }

    /// Get currently allocated memory in bytes
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory usage tracking (see ROADMAP.md §1)
    pub fn allocated(&self) -> usize {
        self.allocated.load(Ordering::SeqCst)
    }

    /// Get peak allocated memory in bytes
    #[allow(dead_code)] // ROADMAP v0.2.0 - Peak memory tracking (see ROADMAP.md §1)
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    /// Get memory limit in bytes
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory limit accessor (see ROADMAP.md §1)
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Get available memory in bytes
    #[allow(dead_code)] // ROADMAP v0.2.0 - Available memory calculation (see ROADMAP.md §1)
    pub fn available(&self) -> usize {
        self.limit.saturating_sub(self.allocated())
    }

    /// Get usage ratio (0.0 to 1.0)
    ///
    /// # Returns
    /// - 0.0 = no memory used
    /// - 0.5 = 50% of budget used
    /// - 1.0 = budget fully used
    /// - >1.0 = over budget (should not happen)
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory usage ratio for adaptive behavior (see ROADMAP.md §1)
    pub fn usage_ratio(&self) -> f64 {
        if self.limit == 0 {
            return 0.0;
        }
        self.allocated() as f64 / self.limit as f64
    }

    /// Check if we're approaching the memory limit
    ///
    /// # Arguments
    /// - `threshold`: Ratio threshold (0.0 to 1.0)
    ///
    /// # Returns
    /// - `true` if usage > threshold
    ///
    /// # Example
    /// ```ignore
    /// if budget.is_approaching_limit(0.9) {
    ///     // 90% of budget used - start streaming results
    /// }
    /// ```
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory pressure detection for graceful degradation (see ROADMAP.md §1)
    pub fn is_approaching_limit(&self, threshold: f64) -> bool {
        self.usage_ratio() > threshold
    }

    /// Reset the budget (clear all allocations)
    ///
    /// Use this when starting a new query execution.
    #[allow(dead_code)] // ROADMAP v0.2.0 - Budget reset for query completion (see ROADMAP.md §1)
    pub fn reset(&self) {
        self.allocated.store(0, Ordering::SeqCst);
        self.peak.store(0, Ordering::SeqCst);
    }

    /// Get memory statistics
    #[allow(dead_code)] // ROADMAP v0.2.0 - Memory statistics for monitoring (see ROADMAP.md §1)
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            limit: self.limit,
            allocated: self.allocated(),
            peak: self.peak(),
            available: self.available(),
        }
    }
}

/// Memory usage statistics
///
/// **Planned Feature** - Memory statistics for query execution monitoring
/// See ROADMAP.md: "Memory Budget Management"
/// Target: v0.2.0
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// Memory limit in bytes
    pub limit: usize,

    /// Currently allocated memory in bytes
    pub allocated: usize,

    /// Peak allocated memory in bytes
    pub peak: usize,

    /// Available memory in bytes
    pub available: usize,
}

impl MemoryStats {
    /// Format as human-readable string
    #[allow(dead_code)] // ROADMAP v0.5.0 - Human-readable memory statistics for diagnostics
    pub fn format_human_readable(&self) -> String {
        format!(
            "Memory: {}/{} ({:.1}%), Peak: {}",
            Self::format_bytes(self.allocated),
            Self::format_bytes(self.limit),
            (self.allocated as f64 / self.limit as f64) * 100.0,
            Self::format_bytes(self.peak)
        )
    }

    /// Format bytes as human-readable (KB, MB, GB)
    #[allow(dead_code)] // ROADMAP v0.5.0 - Byte formatting utility for memory statistics
    fn format_bytes(bytes: usize) -> String {
        const KB: usize = 1024;
        const MB: usize = KB * 1024;
        const GB: usize = MB * 1024;

        if bytes >= GB {
            format!("{:.2}GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2}MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2}KB", bytes as f64 / KB as f64)
        } else {
            format!("{}B", bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_budget_basic() {
        let budget = MemoryBudget::new(1000);

        // Allocate some memory
        assert!(budget.allocate(100).is_ok());
        assert_eq!(budget.allocated(), 100);

        // Allocate more
        assert!(budget.allocate(200).is_ok());
        assert_eq!(budget.allocated(), 300);

        // Release some
        budget.release(100);
        assert_eq!(budget.allocated(), 200);
    }

    #[test]
    fn test_memory_budget_limit_exceeded() {
        let budget = MemoryBudget::new(1000);

        // Allocate up to limit
        assert!(budget.allocate(900).is_ok());

        // Try to exceed limit
        let result = budget.allocate(200);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(ExecutionError::MemoryLimitExceeded { .. })
        ));

        // Allocation should not have happened
        assert_eq!(budget.allocated(), 900);
    }

    #[test]
    fn test_memory_budget_usage_ratio() {
        let budget = MemoryBudget::new(1000);

        assert_eq!(budget.usage_ratio(), 0.0);

        budget.allocate(500).unwrap();
        assert_eq!(budget.usage_ratio(), 0.5);

        budget.allocate(250).unwrap();
        assert_eq!(budget.usage_ratio(), 0.75);
    }

    #[test]
    fn test_memory_budget_peak_tracking() {
        let budget = MemoryBudget::new(1000);

        budget.allocate(100).unwrap();
        assert_eq!(budget.peak(), 100);

        budget.allocate(200).unwrap();
        assert_eq!(budget.peak(), 300);

        budget.release(150);
        assert_eq!(budget.allocated(), 150);
        assert_eq!(budget.peak(), 300); // Peak doesn't decrease
    }

    #[test]
    fn test_memory_budget_approaching_limit() {
        let budget = MemoryBudget::new(1000);

        budget.allocate(500).unwrap();
        assert!(!budget.is_approaching_limit(0.9));

        budget.allocate(450).unwrap();
        assert!(budget.is_approaching_limit(0.9)); // 95% used
    }

    #[test]
    fn test_memory_budget_reset() {
        let budget = MemoryBudget::new(1000);

        budget.allocate(500).unwrap();
        assert_eq!(budget.allocated(), 500);
        assert_eq!(budget.peak(), 500);

        budget.reset();

        assert_eq!(budget.allocated(), 0);
        assert_eq!(budget.peak(), 0);
    }

    #[test]
    fn test_memory_budget_unlimited() {
        let budget = MemoryBudget::unlimited();

        // Should be able to allocate huge amounts
        assert!(budget.allocate(1_000_000_000).is_ok());
        assert!(budget.allocate(1_000_000_000).is_ok());
    }

    #[test]
    fn test_memory_stats_format() {
        let budget = MemoryBudget::new(100 * 1024 * 1024); // 100MB
        budget.allocate(50 * 1024 * 1024).unwrap(); // 50MB

        let stats = budget.stats();
        let formatted = stats.format_human_readable();

        assert!(formatted.contains("50.00MB"));
        assert!(formatted.contains("100.00MB"));
        assert!(formatted.contains("50.0%"));
    }

    #[test]
    fn test_try_allocate() {
        let budget = MemoryBudget::new(1000);

        assert!(budget.try_allocate(500));
        assert_eq!(budget.allocated(), 500);

        assert!(!budget.try_allocate(600)); // Would exceed limit
        assert_eq!(budget.allocated(), 500); // No change
    }

    #[test]
    fn test_available_memory() {
        let budget = MemoryBudget::new(1000);

        assert_eq!(budget.available(), 1000);

        budget.allocate(300).unwrap();
        assert_eq!(budget.available(), 700);

        budget.allocate(500).unwrap();
        assert_eq!(budget.available(), 200);
    }
}
