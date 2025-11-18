// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Row Iterator - Lazy evaluation for query execution (Phase 4: Week 6.5)
//!
//! Provides iterator-based row processing to reduce memory footprint.

use crate::exec::result::Row;
use crate::exec::error::ExecutionError;

/// Iterator trait for lazy row evaluation
///
/// This trait enables streaming query execution where rows are processed
/// one-at-a-time instead of materializing entire result sets in memory.
///
/// # Phase 4: Memory Optimization
///
/// Without lazy evaluation:
/// ```ignore
/// let rows: Vec<Row> = execute_query(); // Loads all rows in memory
/// for row in rows.into_iter().take(10) { // Only uses 10, wasted memory
///     process(row);
/// }
/// ```
///
/// With lazy evaluation:
/// ```ignore
/// let rows: Box<dyn RowIterator> = execute_query_lazy(); // No upfront allocation
/// for row in rows.take(10) { // Stops after 10 rows
///     process(row?);
/// }
/// ```
pub trait RowIterator: Iterator<Item = Result<Row, ExecutionError>> {
    /// Get estimated row count if known
    ///
    /// This is used for:
    /// - Query optimization decisions
    /// - Pre-allocating appropriate capacity
    /// - Progress reporting
    ///
    /// Returns `None` if the count cannot be determined without scanning.
    #[allow(dead_code)] // ROADMAP v0.5.0 - Row count estimation for query optimization
    fn size_hint_rows(&self) -> Option<usize> {
        None
    }

    /// Check if results are pre-sorted by a specific field
    ///
    /// This helps avoid redundant sorting operations.
    ///
    /// # Examples
    /// - TextSearchIterator returns `true` for `is_sorted_by("TEXT_SCORE()")`
    /// - SortIterator returns `true` for the sorted field
    #[allow(dead_code)] // ROADMAP v0.5.0 - Sorted result detection for optimization
    fn is_sorted_by(&self, _field: &str) -> bool {
        false
    }

    /// Convert to a boxed trait object
    #[allow(dead_code)] // ROADMAP v0.5.0 - Dynamic iterator type erasure
    fn boxed(self) -> Box<dyn RowIterator>
    where
        Self: Sized + 'static,
    {
        Box::new(self)
    }
}

/// Iterator wrapper for Vec<Row>
///
/// Converts a materialized Vec<Row> into a RowIterator.
/// Useful for backward compatibility with existing code that returns Vec<Row>.
pub struct VecRowIterator {
    rows: std::vec::IntoIter<Row>,
    #[allow(dead_code)]
    count: usize,
}

impl VecRowIterator {
    /// Create a new VecRowIterator from a Vec<Row>
    #[allow(dead_code)] // ROADMAP v0.5.0 - Vec to iterator conversion for backward compatibility
    pub fn new(rows: Vec<Row>) -> Self {
        let count = rows.len();
        Self {
            rows: rows.into_iter(),
            count,
        }
    }
}

impl Iterator for VecRowIterator {
    type Item = Result<Row, ExecutionError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rows.next().map(Ok)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.rows.size_hint()
    }
}

impl RowIterator for VecRowIterator {
    fn size_hint_rows(&self) -> Option<usize> {
        Some(self.count)
    }

    fn is_sorted_by(&self, _field: &str) -> bool {
        // Vec doesn't track sort state
        false
    }
}

/// Empty iterator (returns no rows)
///
/// Used when:
/// - No input data available
/// - All rows filtered out
/// - Error condition that should return empty result
#[allow(dead_code)]
pub struct EmptyRowIterator;

impl Iterator for EmptyRowIterator {
    type Item = Result<Row, ExecutionError>;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}

impl RowIterator for EmptyRowIterator {
    fn size_hint_rows(&self) -> Option<usize> {
        Some(0)
    }

    fn is_sorted_by(&self, _field: &str) -> bool {
        true // Empty set is sorted by any criterion
    }
}

/// Error iterator (yields a single error)
///
/// Used to propagate errors in iterator chains without panicking.
pub struct ErrorRowIterator {
    error: Option<ExecutionError>,
}

impl ErrorRowIterator {
    #[allow(dead_code)] // ROADMAP v0.5.0 - Error propagation in iterator chains
    pub fn new(error: ExecutionError) -> Self {
        Self {
            error: Some(error),
        }
    }
}

impl Iterator for ErrorRowIterator {
    type Item = Result<Row, ExecutionError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.error.take().map(Err)
    }
}

impl RowIterator for ErrorRowIterator {
    fn size_hint_rows(&self) -> Option<usize> {
        Some(0)
    }
}

/// Utility function to collect iterator results
///
/// Convenience wrapper for `.collect::<Result<Vec<_>, _>>()`
#[allow(dead_code)] // ROADMAP v0.5.0 - Iterator collection utility for materialization
pub fn collect_rows<I>(iter: I) -> Result<Vec<Row>, ExecutionError>
where
    I: Iterator<Item = Result<Row, ExecutionError>>,
{
    iter.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Value;
    use std::collections::HashMap;

    fn create_test_row(id: usize) -> Row {
        let mut values = HashMap::new();
        values.insert("id".to_string(), Value::Number(id as f64));
        Row {
            values,
            positional_values: vec![],
            source_entities: HashMap::new(),
            text_score: None,
            highlight_snippet: None,
        }
    }

    #[test]
    fn test_vec_row_iterator() {
        let rows = vec![create_test_row(1), create_test_row(2), create_test_row(3)];
        let iter = VecRowIterator::new(rows);

        assert_eq!(iter.size_hint_rows(), Some(3));

        let collected: Vec<Row> = iter.map(|r| r.unwrap()).collect();
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn test_empty_row_iterator() {
        let mut iter = EmptyRowIterator;

        assert_eq!(iter.size_hint_rows(), Some(0));
        assert!(iter.next().is_none());
        assert!(RowIterator::is_sorted_by(&iter, "any_field"));
    }

    #[test]
    fn test_error_row_iterator() {
        let error = ExecutionError::UnsupportedOperator("test error".to_string());
        let mut iter = ErrorRowIterator::new(error);

        assert_eq!(iter.size_hint_rows(), Some(0));

        let result = iter.next();
        assert!(result.is_some());
        assert!(result.unwrap().is_err());

        // Second call returns None (error consumed)
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_collect_rows() {
        let rows = vec![create_test_row(1), create_test_row(2)];
        let iter = VecRowIterator::new(rows);

        let collected = collect_rows(iter).unwrap();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_collect_rows_with_error() {
        let error = ExecutionError::UnsupportedOperator("test".to_string());
        let iter = ErrorRowIterator::new(error);

        let result = collect_rows(iter);
        assert!(result.is_err());
    }
}
