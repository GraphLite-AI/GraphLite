// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Production hardening: Resource limits and monitoring
//!
//! Provides:
//! - Query execution timeouts
//! - Memory usage limits
//! - Result set size limits
//! - Index size constraints
//! - Resource usage monitoring
//! - Limit enforcement and violation reporting

use std::time::Duration;

/// Resource limits for query execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum query execution time
    pub query_timeout: Duration,
    /// Maximum memory per query in bytes
    pub max_memory_bytes: u64,
    /// Maximum result set size
    pub max_result_size: usize,
    /// Maximum index size in bytes
    pub max_index_size_bytes: u64,
    /// Enable enforcement
    pub enforce_limits: bool,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            query_timeout: Duration::from_secs(30),           // 30 second timeout
            max_memory_bytes: 1_000_000_000,                  // 1 GB
            max_result_size: 100_000,                         // 100K results
            max_index_size_bytes: 10_000_000_000,             // 10 GB
            enforce_limits: true,
        }
    }
}

impl ResourceLimits {
    /// Create with custom timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            query_timeout: timeout,
            ..Default::default()
        }
    }

    /// Create with custom memory limit
    pub fn with_memory(memory_bytes: u64) -> Self {
        Self {
            max_memory_bytes: memory_bytes,
            ..Default::default()
        }
    }

    /// Create permissive limits (for testing)
    pub fn permissive() -> Self {
        Self {
            query_timeout: Duration::from_secs(300),
            max_memory_bytes: 10_000_000_000,
            max_result_size: 1_000_000,
            max_index_size_bytes: 100_000_000_000,
            enforce_limits: false,
        }
    }
}

/// Resource limit violation
#[derive(Debug, Clone)]
pub struct LimitViolation {
    /// Type of limit violated
    pub limit_type: LimitType,
    /// Current value
    pub current_value: u64,
    /// Limit threshold
    pub limit_value: u64,
    /// Message
    pub message: String,
}

/// Types of resource limits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitType {
    QueryTimeout,
    MemoryUsage,
    ResultSize,
    IndexSize,
}

impl LimitType {
    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            Self::QueryTimeout => "Query Timeout",
            Self::MemoryUsage => "Memory Usage",
            Self::ResultSize => "Result Size",
            Self::IndexSize => "Index Size",
        }
    }
}

/// Resource usage monitor
pub struct ResourceMonitor {
    limits: ResourceLimits,
    violations: Vec<LimitViolation>,
}

impl ResourceMonitor {
    /// Create a new resource monitor
    pub fn new(limits: ResourceLimits) -> Self {
        Self {
            limits,
            violations: Vec::new(),
        }
    }

    /// Check if result size exceeds limit
    pub fn check_result_size(&mut self, size: usize) -> Result<(), LimitViolation> {
        if !self.limits.enforce_limits {
            return Ok(());
        }

        if size > self.limits.max_result_size {
            let violation = LimitViolation {
                limit_type: LimitType::ResultSize,
                current_value: size as u64,
                limit_value: self.limits.max_result_size as u64,
                message: format!(
                    "Result size {} exceeds limit {}",
                    size, self.limits.max_result_size
                ),
            };
            self.violations.push(violation.clone());
            Err(violation)
        } else {
            Ok(())
        }
    }

    /// Check if memory usage exceeds limit
    pub fn check_memory(&mut self, memory_bytes: u64) -> Result<(), LimitViolation> {
        if !self.limits.enforce_limits {
            return Ok(());
        }

        if memory_bytes > self.limits.max_memory_bytes {
            let violation = LimitViolation {
                limit_type: LimitType::MemoryUsage,
                current_value: memory_bytes,
                limit_value: self.limits.max_memory_bytes,
                message: format!(
                    "Memory usage {} exceeds limit {}",
                    memory_bytes, self.limits.max_memory_bytes
                ),
            };
            self.violations.push(violation.clone());
            Err(violation)
        } else {
            Ok(())
        }
    }

    /// Check if index size exceeds limit
    pub fn check_index_size(&mut self, size_bytes: u64) -> Result<(), LimitViolation> {
        if !self.limits.enforce_limits {
            return Ok(());
        }

        if size_bytes > self.limits.max_index_size_bytes {
            let violation = LimitViolation {
                limit_type: LimitType::IndexSize,
                current_value: size_bytes,
                limit_value: self.limits.max_index_size_bytes,
                message: format!(
                    "Index size {} exceeds limit {}",
                    size_bytes, self.limits.max_index_size_bytes
                ),
            };
            self.violations.push(violation.clone());
            Err(violation)
        } else {
            Ok(())
        }
    }

    /// Check if query timeout exceeded
    pub fn check_timeout(&mut self, elapsed: Duration) -> Result<(), LimitViolation> {
        if !self.limits.enforce_limits {
            return Ok(());
        }

        if elapsed > self.limits.query_timeout {
            let violation = LimitViolation {
                limit_type: LimitType::QueryTimeout,
                current_value: elapsed.as_millis() as u64,
                limit_value: self.limits.query_timeout.as_millis() as u64,
                message: format!(
                    "Query timeout: {:?} exceeds limit {:?}",
                    elapsed, self.limits.query_timeout
                ),
            };
            self.violations.push(violation.clone());
            Err(violation)
        } else {
            Ok(())
        }
    }

    /// Get all violations
    pub fn violations(&self) -> &[LimitViolation] {
        &self.violations
    }

    /// Clear violation history
    pub fn clear_violations(&mut self) {
        self.violations.clear();
    }

    /// Get violation summary
    pub fn violation_summary(&self) -> String {
        if self.violations.is_empty() {
            return "No limit violations".to_string();
        }

        let mut summary = format!("{} limit violations:\n", self.violations.len());
        for violation in &self.violations {
            summary.push_str(&format!(
                "  - {}: {} > {} ({:.1}%)\n",
                violation.limit_type.name(),
                violation.current_value,
                violation.limit_value,
                (violation.current_value as f64 / violation.limit_value as f64) * 100.0
            ));
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.query_timeout, Duration::from_secs(30));
        assert_eq!(limits.max_memory_bytes, 1_000_000_000);
        assert_eq!(limits.max_result_size, 100_000);
        assert!(limits.enforce_limits);
    }

    #[test]
    fn test_resource_limits_with_timeout() {
        let timeout = Duration::from_secs(60);
        let limits = ResourceLimits::with_timeout(timeout);
        assert_eq!(limits.query_timeout, timeout);
    }

    #[test]
    fn test_resource_limits_permissive() {
        let limits = ResourceLimits::permissive();
        assert!(!limits.enforce_limits);
        assert!(limits.query_timeout > Duration::from_secs(100));
    }

    #[test]
    fn test_limit_type_names() {
        assert_eq!(LimitType::QueryTimeout.name(), "Query Timeout");
        assert_eq!(LimitType::MemoryUsage.name(), "Memory Usage");
        assert_eq!(LimitType::ResultSize.name(), "Result Size");
        assert_eq!(LimitType::IndexSize.name(), "Index Size");
    }

    #[test]
    fn test_resource_monitor_creation() {
        let limits = ResourceLimits::default();
        let monitor = ResourceMonitor::new(limits);
        assert_eq!(monitor.violations().len(), 0);
    }

    #[test]
    fn test_result_size_check() {
        let limits = ResourceLimits::default();
        let mut monitor = ResourceMonitor::new(limits);

        // Within limit
        assert!(monitor.check_result_size(50_000).is_ok());

        // Exceeds limit
        assert!(monitor.check_result_size(150_000).is_err());
        assert_eq!(monitor.violations().len(), 1);
    }

    #[test]
    fn test_memory_check() {
        let limits = ResourceLimits::default();
        let mut monitor = ResourceMonitor::new(limits);

        // Within limit
        assert!(monitor.check_memory(500_000_000).is_ok());

        // Exceeds limit
        assert!(monitor.check_memory(2_000_000_000).is_err());
        assert_eq!(monitor.violations().len(), 1);
    }

    #[test]
    fn test_timeout_check() {
        let limits = ResourceLimits::default();
        let mut monitor = ResourceMonitor::new(limits);

        // Within limit
        assert!(monitor.check_timeout(Duration::from_secs(10)).is_ok());

        // Exceeds limit
        assert!(monitor.check_timeout(Duration::from_secs(60)).is_err());
        assert_eq!(monitor.violations().len(), 1);
    }

    #[test]
    fn test_violation_summary() {
        let limits = ResourceLimits::default();
        let mut monitor = ResourceMonitor::new(limits);

        monitor.check_result_size(150_000).ok();
        monitor.check_memory(2_000_000_000).ok();

        let summary = monitor.violation_summary();
        assert!(summary.contains("2 limit violations"));
        assert!(summary.contains("Result Size"));
        assert!(summary.contains("Memory Usage"));
    }

    #[test]
    fn test_monitor_disabled_limits() {
        let mut limits = ResourceLimits::default();
        limits.enforce_limits = false;

        let mut monitor = ResourceMonitor::new(limits);

        // All checks should pass even when exceeded
        assert!(monitor.check_result_size(200_000).is_ok());
        assert!(monitor.check_memory(5_000_000_000).is_ok());
        assert_eq!(monitor.violations().len(), 0);
    }
}
