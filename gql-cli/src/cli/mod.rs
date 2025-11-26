// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! CLI module for GraphLite
//!
//! Provides command-line interface for database initialization,
//! interactive GQL console (REPL), and one-off query execution.

pub mod commands;
pub mod gqlcli;
pub mod output;

pub use commands::{Cli, Commands};
pub use gqlcli::{handle_gql, handle_install, handle_query};
