// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query plan optimizers
//!
//! This module contains optimization rules for logical and physical plans:
//! - logical_optimizer: Logical optimization rules (predicate pushdown, join reordering, etc.)
//! - physical_optimizer: Physical optimization rules (index selection, operator selection, etc.)

pub mod logical;
pub mod logical_optimizer;
pub mod physical;
pub mod physical_optimizer;

// Re-export for convenience
pub use logical_optimizer::LogicalOptimizer;
pub use physical_optimizer::PhysicalOptimizer;
