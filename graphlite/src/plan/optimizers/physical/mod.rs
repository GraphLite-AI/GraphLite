// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Physical plan optimization rules
//!
//! Future optimization rules:
//! - index_selection.rs - Choose optimal indexes for scans
//! - join_algorithm.rs - Select join algorithm (hash, nested loop, merge)
//! - operator_selection.rs - Choose physical operators (seq scan vs index scan)
//! - parallel_execution.rs - Identify parallelizable operations

// TODO: Extract optimization rules from optimizer.rs
