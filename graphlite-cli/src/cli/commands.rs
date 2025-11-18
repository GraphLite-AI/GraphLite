// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! CLI command definitions for GraphLite

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Log level options
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LogLevel {
    /// Only errors
    Error,
    /// Warnings and errors
    Warn,
    /// Info, warnings, and errors
    Info,
    /// Debug messages and above (verbose)
    Debug,
    /// All messages including trace (very verbose)
    Trace,
    /// Disable all logging
    Off,
}

impl LogLevel {
    /// Convert to log::LevelFilter
    pub fn to_level_filter(self) -> log::LevelFilter {
        match self {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Off => log::LevelFilter::Off,
        }
    }
}

/// GraphLite CLI - ISO GQL Graph Database
#[derive(Parser)]
#[command(name = "graphlite")]
#[command(about = "GraphLite - A lightweight ISO GQL Graph Database")]
#[command(version)]
pub struct Cli {
    /// Username for authentication
    #[arg(short = 'u', long = "user", global = true)]
    pub user: Option<String>,

    /// Password for authentication (if not provided, will be prompted)
    #[arg(short = 'p', long = "password", global = true)]
    pub password: Option<String>,

    /// Set log level (error, warn, info, debug, trace, off)
    #[arg(short = 'l', long = "log-level", global = true, value_enum)]
    pub log_level: Option<LogLevel>,

    /// Verbose mode (equivalent to --log-level debug)
    #[arg(short = 'v', long = "verbose", global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Show detailed version information
    Version,

    /// Execute a GQL query
    Query {
        /// The GQL query to execute
        query: String,

        /// Database path
        #[arg(long, default_value = "./db")]
        path: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "table")]
        format: OutputFormat,

        /// Show execution plan
        #[arg(short, long)]
        explain: bool,

        /// Show Abstract Syntax Tree (AST)
        #[arg(long)]
        ast: bool,
    },

    /// Interactive GQL console (REPL)
    Gql {
        /// Database path
        #[arg(long, default_value = "./db")]
        path: PathBuf,

        /// Load sample data on startup
        #[arg(short, long)]
        sample: bool,
    },

    /// Install and initialize GraphLite
    Install {
        /// Database path
        #[arg(long, default_value = "./db")]
        path: PathBuf,

        /// Admin username
        #[arg(long, default_value = "admin")]
        admin_user: String,

        /// Admin password (will be prompted if not provided)
        #[arg(long)]
        admin_password: Option<String>,

        /// Force reinstall even if already installed
        #[arg(short, long)]
        force: bool,

        /// Skip confirmation prompts
        #[arg(short, long)]
        yes: bool,
    },

    /// Session management commands
    Session {
        #[command(subcommand)]
        action: SessionAction,

        /// Database path
        #[arg(long, default_value = "./db")]
        path: PathBuf,
    },
}

/// Session management subcommands
#[derive(Subcommand)]
pub enum SessionAction {
    /// Set session variables or characteristics
    Set {
        /// What to set (schema, graph, timezone, etc.)
        target: String,

        /// Value to set
        value: String,

        /// Parameter name (for session variables)
        #[arg(short, long)]
        parameter: Option<String>,
    },

    /// Reset session variables or characteristics
    Reset {
        /// What to reset (all, schema, graph, timezone, or parameter name)
        target: Option<String>,

        /// Reset all parameters
        #[arg(short, long)]
        all: bool,
    },

    /// Show current session state
    Show {
        /// Show specific aspect (parameters, characteristics, all)
        #[arg(default_value = "all")]
        target: String,
    },

    /// Close session
    Close,
}

/// Output format options
#[derive(Clone, Copy, Debug)]
pub enum OutputFormat {
    Table,
    Json,
    Csv,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            _ => Err(format!("Unknown output format: {}", s)),
        }
    }
}
