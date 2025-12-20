// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Query plan optimizers
//!
//! This module will contain optimization rules for logical and physical plans:
//! - logical/: Logical optimization rules (predicate pushdown, join reordering, etc.)
//! - physical/: Physical optimization rules (index selection, operator selection, etc.)
//!
//! Currently these are still in optimizer.rs and will be extracted in future refactoring.

pub mod logical;
pub mod physical;
