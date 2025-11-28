// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Session management for multi-graph database operations
//!
//! This module provides session management functionality for:
//! - User session contexts with graph preferences
//! - Session-scoped graph context switching (SET SESSION GRAPH)
//! - User authentication and authorization
//! - Session isolation and concurrent access
//!
//! Features supported:
//! - Session creation and destruction
//! - Current graph context per session
//! - Home graph assignment per user
//! - Session parameter management
//! - Multi-tenancy and user isolation
//!
//! # Session Providers
//!
//! Two session provider implementations are available:
//!
//! - `InstanceSessionProvider`: Each QueryCoordinator has isolated sessions (embedded mode)
//! - `GlobalSessionProvider`: All QueryCoordinators share a global session pool (server mode)

pub mod global_provider;
pub mod instance_provider;
pub mod manager;
pub mod mode;
pub mod models;
pub mod provider;
pub mod transaction_state;

pub use global_provider::GlobalSessionProvider;
pub use instance_provider::InstanceSessionProvider;
pub use manager::SessionManager;
pub use mode::SessionMode;
pub use models::{SessionPermissionCache, UserSession};
pub use provider::SessionProvider;
pub use transaction_state::SessionTransactionState;
