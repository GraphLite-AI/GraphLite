// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! CLI command handlers for GraphLite

use colored::Colorize;
use rustyline::{error::ReadlineError, CompletionType, Config, EditMode, Editor};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::commands::OutputFormat;
use super::output::ResultFormatter;
use graphlite::QueryCoordinator;

// Note: init_database_components has been removed.
// All database initialization is now handled internally by QueryCoordinator::from_path()

/// Handle the install command
///
/// SQLite-style initialization: Creates database files and fully initializes
/// the database using a coordinator instance that lives only during this command.
/// All state is persisted to disk via Sled before the process exits.
pub fn handle_install(
    path: PathBuf,
    admin_user: String,
    admin_password: Option<String>,
    force: bool,
    yes: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if database already exists
    if path.exists() && !force && !yes {
        println!(
            "{}",
            format!("Database already exists at {:?}", path).yellow()
        );
        println!("Use --force to reinstall, or choose a different path.");
        return Err("Database already exists".into());
    }

    // Prompt for password if not provided
    let password = match admin_password {
        Some(pwd) => pwd,
        None => {
            print!("Enter admin password: ");
            std::io::Write::flush(&mut std::io::stdout())?;
            rpassword::read_password()?
        }
    };

    println!("{}", "Initializing GraphLite...".bold().green());

    // Create database directory
    std::fs::create_dir_all(&path)?;

    // SQLite-style pattern: Create coordinator for this command's lifetime
    // The coordinator will initialize all components and persist state to disk
    println!("  → Creating database files...");

    // Initialize coordinator - this handles all internal component setup
    let coordinator = QueryCoordinator::from_path(&path)
        .map_err(|e| format!("Failed to initialize database: {}", e))?;

    println!("  → Initializing security catalog...");

    // The security catalog provider already created a default 'admin' user during initialization
    // Now we need to set the password for this user
    println!("  → Setting admin user password...");

    coordinator
        .set_user_password(&admin_user, &password)
        .map_err(|e| format!("Failed to set admin password: {}", e))?;

    println!("    Password set for user '{}'", admin_user);
    println!("    Security catalog saved to disk");

    // Create a system session for additional setup operations
    let session_id = coordinator.create_simple_session("system")?;

    // Create default schema
    println!("  → Creating default schema...");
    match coordinator.process_query("CREATE SCHEMA IF NOT EXISTS /default", &session_id) {
        Ok(_) => println!("    Default schema created"),
        Err(e) => println!("    Schema creation: {}", e),
    }

    // Close the system session
    let _ = coordinator.close_session(&session_id);

    // Print success message
    println!(
        "{}",
        format!("\nGraphLite initialized at {:?}", path).green()
    );
    println!("{}", "\nDatabase is ready to use!".bold().green());
    println!("{}", "\nStart the GQL console with:".yellow());
    println!(
        "{}",
        format!("  cargo run -- gql --path {:?} -u {}", path, admin_user).cyan()
    );
    println!("{}", "\nOr execute queries directly:".yellow());
    println!(
        "{}",
        format!(
            "  cargo run -- query --path {:?} -u {} \"MATCH (n) RETURN n\"",
            path, admin_user
        )
        .cyan()
    );

    // Coordinator drops here, closing all connections
    // All data has been persisted to disk via Sled
    Ok(())
}

/// Handle the gql (REPL) command
pub fn handle_gql(
    path: PathBuf,
    user: Option<String>,
    password: Option<String>,
    _sample: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if database exists
    if !path.exists() {
        return Err(format!(
            "Database not found at {:?}. Run 'cargo run -- install' first.",
            path
        )
        .into());
    }

    // Prompt for credentials if not provided
    let username = user.unwrap_or_else(|| {
        print!("Username: ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        input.trim().to_string()
    });

    let password = password.unwrap_or_else(|| {
        print!("Password: ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        rpassword::read_password().unwrap()
    });

    // Load database
    let coordinator = load_database(&path)?;

    // Authenticate
    let session_id = authenticate(&coordinator, &username, &password)?;

    println!("{}", "GraphLite".bold().green());
    println!("Type 'help' for commands, 'exit' or 'quit' to exit");
    println!("Multi-line queries supported - use ';' to terminate");
    println!("NL2GQL: prefix with 'nl:' for natural language (e.g., 'nl: find all people')");
    println!("{}", format!("Authenticated as: {}", username).cyan());
    println!("Session ID: {}", session_id);

    // Create REPL editor
    let config = Config::builder()
        .edit_mode(EditMode::Emacs)
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .auto_add_history(false)
        .build();

    let mut rl = Editor::<(), _>::with_config(config)?;

    let history_path = ".graphlite/.gql_history.txt";
    if let Some(parent) = Path::new(&history_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    let _ = rl.load_history(&history_path);

    let mut query_buffer = String::new();

    loop {
        let prompt = if query_buffer.is_empty() {
            format!("{}::gql> ", username.cyan())
        } else {
            format!("{}::...> ", username.cyan())
        };

        let line = match rl.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                if !query_buffer.is_empty() {
                    query_buffer.clear();
                    println!("{}", "\nQuery buffer cleared".yellow());
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "Goodbye!".green());
                break;
            }
            Err(err) => {
                eprintln!("{}", format!("Error: {:?}", err).red());
                break;
            }
        };

        let trimmed = line.trim();

        // Handle special commands
        if query_buffer.is_empty() {
            match trimmed.to_lowercase().as_str() {
                "exit" | "quit" => {
                    println!("{}", "Goodbye!".green());
                    break;
                }
                "help" => {
                    print_help();
                    continue;
                }
                "clear" => {
                    print!("\x1B[2J\x1B[1;1H");
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    continue;
                }
                "" => continue,
                _ => {}
            }

            // Check for nl: prefix (natural language query)
            if trimmed.to_lowercase().starts_with("nl:") {
                let nl_input = trimmed[3..].trim();
                if nl_input.is_empty() {
                    println!("{}", "Usage: nl: <natural language query>".yellow());
                    println!("{}", "Example: nl: find all people older than 25".cyan());
                    continue;
                }

                // Extract schema and convert NL to GQL
                let schema = match extract_schema(&coordinator, &session_id) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{}", format!("Failed to extract schema: {}", e).red());
                        println!(
                            "{}",
                            "Make sure you have set a graph with SESSION SET GRAPH".yellow()
                        );
                        continue;
                    }
                };

                println!("{}", "Converting natural language to GQL...".cyan());
                match convert_nl_to_gql(nl_input, &schema) {
                    Ok(gql) => {
                        println!("{}", format!("Generated GQL: {}", gql).green());
                        rl.add_history_entry(trimmed)?;
                        match coordinator.process_query(&gql, &session_id) {
                            Ok(result) => {
                                let output = ResultFormatter::format(&result, OutputFormat::Table);
                                println!("{}", output);
                            }
                            Err(e) => {
                                eprintln!("{}", format!("Error: {}", e).red());
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}", format!("NL2GQL error: {}", e).red());
                    }
                }
                continue;
            }
        }

        // Append line to buffer
        query_buffer.push_str(&line);
        query_buffer.push('\n');

        // Check if query is complete (ends with semicolon)
        if trimmed.ends_with(';') {
            let query = query_buffer.trim().to_string();
            rl.add_history_entry(&query)?;

            // Execute query
            match coordinator.process_query(&query, &session_id) {
                Ok(result) => {
                    let output = ResultFormatter::format(&result, OutputFormat::Table);
                    println!("{}", output);
                }
                Err(e) => {
                    // Don't show error for duplicate entries with IF NOT EXISTS
                    // These are gracefully handled and expected
                    if !e.contains("Duplicate entry") && !e.contains("already exists") {
                        eprintln!("{}", format!("Error: {}", e).red());
                    }
                }
            }

            query_buffer.clear();
        }
    }

    // Save history
    let _ = rl.save_history(&history_path);

    Ok(())
}

/// Handle the query command (one-off query execution)
pub fn handle_query(
    path: PathBuf,
    query: String,
    user: Option<String>,
    password: Option<String>,
    format: OutputFormat,
    explain: bool,
    ast: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if database exists
    if !path.exists() {
        return Err(format!(
            "Database not found at {:?}. Run 'cargo run -- install' first.",
            path
        )
        .into());
    }

    // Load database
    let coordinator = load_database(&path)?;

    // Authenticate if credentials provided, otherwise use anonymous session
    let session_id = if let (Some(u), Some(p)) = (user, password) {
        authenticate(&coordinator, &u, &p)?
    } else {
        // Create anonymous session (limited permissions)
        coordinator.create_simple_session("anonymous")?
    };

    // Show AST if requested
    if ast {
        println!(
            "{}",
            "AST display feature not available in CLI-only mode".yellow()
        );
        println!("{}", "AST is an internal implementation detail".yellow());
        return Ok(());
    }

    // Show execution plan if requested
    if explain {
        println!("{}", "Query execution plan not yet implemented".yellow());
        return Ok(());
    }

    // Execute query
    match coordinator.process_query(&query, &session_id) {
        Ok(result) => {
            let output = ResultFormatter::format(&result, format);
            println!("{}", output);
            Ok(())
        }
        Err(e) => {
            eprintln!("{}", format!("Error: {}", e).red());
            Err(e.into())
        }
    }
}

/// Load an existing database
fn load_database(path: &PathBuf) -> Result<Arc<QueryCoordinator>, Box<dyn std::error::Error>> {
    // Use simplified API - all component initialization is handled internally
    let coordinator = QueryCoordinator::from_path(path)
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    Ok(coordinator)
}

/// Authenticate a user and create a session
fn authenticate(
    coordinator: &Arc<QueryCoordinator>,
    username: &str,
    password: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    coordinator
        .authenticate_and_create_session(username, password)
        .map_err(|e| e.into())
}

/// Extract schema from the current graph by querying node labels, properties, and edge types
fn extract_schema(coordinator: &Arc<QueryCoordinator>, session_id: &str) -> Result<String, String> {
    use graphlite::Value;
    use std::collections::HashSet;

    let mut schema_parts = Vec::new();

    // 1. Get all node labels
    let labels_result = coordinator
        .process_query("MATCH (n) RETURN DISTINCT labels(n) AS labels;", session_id)
        .map_err(|e| e.to_string())?;

    let mut node_labels = HashSet::new();
    for row in &labels_result.rows {
        if let Some(Value::List(items)) = row.values.get("labels") {
            for item in items {
                if let Value::String(label) = item {
                    node_labels.insert(label.clone());
                }
            }
        }
    }

    // 2. Get properties for each label (query one sample node)
    let mut entities = Vec::new();
    for label in &node_labels {
        let query = format!("MATCH (n:{}) RETURN n LIMIT 1;", label);
        if let Ok(result) = coordinator.process_query(&query, session_id) {
            let mut props: Vec<String> = Vec::new();
            for row in &result.rows {
                if let Some(Value::Node(node)) = row.values.get("n") {
                    props.extend(node.properties.keys().cloned());
                }
            }
            props.sort();
            props.dedup();
            if props.is_empty() {
                entities.push(format!("- {}", label));
            } else {
                entities.push(format!("- {}: {}", label, props.join(", ")));
            }
        } else {
            entities.push(format!("- {}", label));
        }
    }
    entities.sort(); // Consistent ordering

    if !entities.is_empty() {
        schema_parts.push(format!("Entities and properties:\n{}", entities.join("\n")));
    }

    // 3. Get all edge types
    let edges_result = coordinator
        .process_query(
            "MATCH ()-[r]->() RETURN DISTINCT type(r) AS edge_type;",
            session_id,
        )
        .map_err(|e| e.to_string())?;

    let mut edge_types = HashSet::new();
    for row in &edges_result.rows {
        if let Some(Value::String(edge_type)) = row.values.get("edge_type") {
            edge_types.insert(edge_type.clone());
        }
    }

    if !edge_types.is_empty() {
        let mut edges_sorted: Vec<_> = edge_types.iter().collect();
        edges_sorted.sort();
        let edges_formatted: Vec<String> =
            edges_sorted.iter().map(|e| format!("- [:{}]", e)).collect();
        schema_parts.push(format!("Relationships:\n{}", edges_formatted.join("\n")));
    }

    if schema_parts.is_empty() {
        return Err("No schema found - the graph appears to be empty".to_string());
    }

    Ok(schema_parts.join("\n\n"))
}

/// Convert natural language to GQL using the Python NL2GQL pipeline
fn convert_nl_to_gql(nl_query: &str, schema: &str) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new("python3")
        .args([
            "-m",
            "nl2gql.pipeline.cli",
            "--nl",
            nl_query,
            "--schema",
            schema,
            "--raw",
        ])
        .output()
        .map_err(|e| format!("Failed to run NL2GQL pipeline: {}", e))?;

    if output.status.success() {
        // With --raw flag, stdout contains only the generated query
        let gql = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if gql.is_empty() {
            Err("NL2GQL returned empty query".to_string())
        } else {
            Ok(gql)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.trim().to_string())
    }
}

/// Print help message
fn print_help() {
    println!("{}", "Available commands:".bold().green());
    println!("  {}  - Show this help message", "help".cyan());
    println!("  {}  - Exit the GQL console", "exit/quit".cyan());
    println!("  {}  - Clear the screen", "clear".cyan());
    println!("\n{}", "Natural language queries:".bold().green());
    println!(
        "  {}  - Convert natural language to GQL and execute",
        "nl: <query>".cyan()
    );
    println!("\n{}", "Query syntax:".bold().green());
    println!("  Multi-line queries are supported");
    println!("  Terminate queries with semicolon (;)");
    println!("\n{}", "GQL examples:".bold().green());
    println!("  {}", "MATCH (n:Person) RETURN n;".yellow());
    println!("  {}", "CREATE SCHEMA /myschema;".yellow());
    println!("  {}", "INSERT (p:Person {{name: 'Alice'}});".yellow());
    println!("\n{}", "NL examples:".bold().green());
    println!("  {}", "nl: find all people".yellow());
    println!("  {}", "nl: show users older than 30".yellow());
}
