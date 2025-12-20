// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Logical plan optimization rules
//!
//! Future optimization rules:
//! - predicate_pushdown.rs - Push filters down the plan tree
//! - projection_pushdown.rs - Push projections down to reduce data
//! - join_reordering.rs - Reorder joins based on selectivity
//! - constant_folding.rs - Evaluate constant expressions at planning time
//! - subquery_unnesting.rs - Convert subqueries to joins where possible

// TODO: Extract optimization rules from optimizer.rs
