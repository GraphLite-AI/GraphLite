// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! GraphLite CLI entry point

use clap::Parser;
use colored::Colorize;

mod cli;
use cli::{Cli, Commands};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments first to get log level
    let cli = Cli::parse();

    // Determine log level from CLI args or environment variable
    let log_level = if cli.verbose {
        // -v/--verbose flag takes precedence
        log::LevelFilter::Debug
    } else if let Some(level) = cli.log_level {
        // --log-level flag
        level.to_level_filter()
    } else {
        // Default to Warn (can still be overridden by RUST_LOG env var)
        log::LevelFilter::Warn
    };

    // Initialize logger
    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();

    // Handle commands
    match cli.command {
        Commands::Version => {
            println!("{} {}", "GraphLite".bold().green(), graphlite::VERSION);
            println!("ISO GQL Graph Database");
            Ok(())
        }

        Commands::Install {
            path,
            admin_user,
            admin_password,
            force,
            yes,
        } => {
            cli::handle_install(path, admin_user, admin_password, force, yes)
        }

        Commands::Gql { path, sample } => {
            cli::handle_gql(path, cli.user, cli.password, sample)
        }

        Commands::Query {
            query,
            path,
            format,
            explain,
            ast,
        } => {
            cli::handle_query(path, query, cli.user, cli.password, format, explain, ast)
        }

        Commands::Session { action: _, path: _ } => {
            println!("{}", "Session management not yet implemented".yellow());
            Ok(())
        }
    }
}
