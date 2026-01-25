// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query planning and optimization for GQL queries
//!
//! This module provides query planning capabilities that convert AST queries
//! into optimized execution plans. It includes logical plan generation,
//! physical plan optimization, and cost estimation.

pub mod cost;
pub mod optimizer;
pub mod pattern_optimization;
pub mod trace;

// Phase 2 refactoring: New module structure
pub mod builders; // Plan builders (AST→Logical, Logical→Physical)
pub mod operators; // Logical and physical operators (organized)
pub mod optimizers; // Optimization rules (logical and physical) - TODO: extract from optimizer.rs

// Re-export for backward compatibility
pub use builders::insert_builder as insert_planner;
pub use operators::logical; // logical.rs moved to operators/
pub use operators::physical; // physical.rs moved to operators/ // insert_planner.rs moved to builders/
