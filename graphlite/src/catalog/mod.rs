// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Pluggable catalog system
//!
//! This module implements a modular, pluggable catalog architecture that allows
//! adding new catalog types by implementing a trait and registering them.
//! All catalogs follow the same interface patterns and integrate seamlessly
//! with the existing storage system.

// Core catalog system exports
pub mod traits;
pub mod operations;
pub mod error;
pub mod registry;
pub mod manager;
pub mod providers;
// pub mod metadata; // Removed - not part of ISO GQL
pub mod storage;
pub mod system_procedures;

