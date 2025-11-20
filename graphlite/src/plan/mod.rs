// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query planning and optimization for GQL queries
//!
//! This module provides query planning capabilities that convert AST queries
//! into optimized execution plans. It includes logical plan generation,
//! physical plan optimization, and cost estimation.

pub mod cost;
pub mod insert_planner;
pub mod logical;
pub mod optimizer;
pub mod pattern_optimization;
pub mod physical;
pub mod trace;
