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

pub mod models;
pub mod transaction_state;
pub mod manager;

pub use models::{UserSession, SessionPermissionCache};
pub use transaction_state::SessionTransactionState;
pub use manager::{get_session_manager, SessionManager};
