//! Database connection and session management
//!
//! This module provides the main entry points for working with GraphLite databases.
//! It follows a similar pattern to rusqlite (SQLite's Rust bindings) but adapted
//! for graph databases.

use crate::error::{Error, Result};
use crate::transaction::Transaction;
use graphlite::{QueryCoordinator, QueryResult};
use std::sync::Arc;

/// Main entry point for GraphLite database operations
///
/// Represents an open connection to a GraphLite database. Despite being an
/// embedded database, we use "Connection" terminology following the SQLite
/// convention as it represents the connection to the database files.
///
/// # Examples
///
/// ```no_run
/// use graphlite_sdk::GraphLite;
///
/// # fn main() -> Result<(), graphlite_sdk::Error> {
/// // Open a database
/// let db = GraphLite::open("./mydb")?;
///
/// // Create a session for a user
/// let session = db.session("admin")?;
///
/// // Execute a query
/// let result = session.query("MATCH (n:Person) RETURN n")?;
/// # Ok(())
/// # }
/// ```
pub struct GraphLite {
    coordinator: Arc<QueryCoordinator>,
}

impl GraphLite {
    /// Open a GraphLite database at the given path
    ///
    /// Creates or opens a database at the specified path and initializes
    /// all necessary components. This is the main entry point for working
    /// with GraphLite databases.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database directory
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use graphlite_sdk::GraphLite;
    ///
    /// let db = GraphLite::open("./mydb")?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let coordinator = QueryCoordinator::from_path(path)
            .map_err(|e| Error::Connection(format!("Failed to open database: {}", e)))?;
        Ok(GraphLite {
            coordinator,
        })
    }

    /// Create a new session for the given user
    ///
    /// Sessions provide user context for permissions and security. Each session
    /// maintains its own transaction state and is isolated from other sessions.
    ///
    /// Unlike SQLite which doesn't have sessions, GraphLite uses sessions for:
    /// - User authentication and permissions
    /// - Transaction isolation
    /// - Audit logging
    ///
    /// # Arguments
    ///
    /// * `username` - Username for the session
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// let session = db.session("admin")?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn session(&self, username: &str) -> Result<Session> {
        let session_id = self.coordinator
            .create_simple_session(username)
            .map_err(|e| Error::Session(format!("Failed to create session: {}", e)))?;

        Ok(Session {
            id: session_id,
            coordinator: self.coordinator.clone(),
            username: username.to_string(),
        })
    }

    /// Get access to the underlying QueryCoordinator
    ///
    /// Provides direct access to the low-level API when needed for
    /// advanced operations not yet covered by the SDK.
    ///
    /// # Safety
    ///
    /// This is an escape hatch for advanced users. Most applications should
    /// use the high-level SDK API instead.
    pub fn coordinator(&self) -> &QueryCoordinator {
        &self.coordinator
    }
}

/// Represents an active database session
///
/// Sessions provide user context and are required for executing queries.
/// Unlike SQLite, GraphLite uses sessions for user authentication, permissions,
/// and transaction isolation.
///
/// # Examples
///
/// ```no_run
/// # use graphlite_sdk::GraphLite;
/// # let db = GraphLite::open("./mydb")?;
/// let session = db.session("admin")?;
/// let result = session.query("MATCH (n) RETURN n")?;
/// # Ok::<(), graphlite_sdk::Error>(())
/// ```
pub struct Session {
    id: String,
    coordinator: Arc<QueryCoordinator>,
    username: String,
}

impl Session {
    /// Get the session ID
    ///
    /// The session ID is used internally for query execution and
    /// transaction management.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the username associated with this session
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Execute a GQL query in this session
    ///
    /// This is the main method for executing queries. For simple read queries,
    /// this is all you need. For multi-statement operations that need atomicity,
    /// use transactions instead.
    ///
    /// # Arguments
    ///
    /// * `query` - GQL query string
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// let session = db.session("admin")?;
    ///
    /// // Simple query
    /// let result = session.query("MATCH (n:Person) RETURN n")?;
    ///
    /// // Query with parameters (using GQL parameter syntax)
    /// let result = session.query("MATCH (n:Person {name: 'Alice'}) RETURN n")?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn query(&self, query: &str) -> Result<QueryResult> {
        self.coordinator
            .process_query(query, &self.id)
            .map_err(|e| Error::Query(format!("Query failed: {}", e)))
    }

    /// Execute a statement without returning results
    ///
    /// This is useful for DDL statements (CREATE SCHEMA, CREATE GRAPH, etc.)
    /// and DML statements where you don't need the results.
    ///
    /// # Arguments
    ///
    /// * `statement` - GQL statement to execute
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// let session = db.session("admin")?;
    ///
    /// // Create a node
    /// session.execute("CREATE (p:Person {name: 'Alice', age: 30})")?;
    ///
    /// // Create a schema
    /// session.execute("CREATE SCHEMA my_schema")?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn execute(&self, statement: &str) -> Result<()> {
        self.coordinator
            .process_query(statement, &self.id)
            .map_err(|e| Error::Query(format!("Execute failed: {}", e)))?;
        Ok(())
    }

    /// Begin a new transaction
    ///
    /// Transactions provide ACID guarantees and can be committed or rolled back.
    /// Following the rusqlite pattern, transactions will automatically roll back
    /// when dropped unless explicitly committed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// let session = db.session("admin")?;
    ///
    /// // Transaction with explicit commit
    /// let mut tx = session.transaction()?;
    /// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    /// tx.execute("CREATE (p:Person {name: 'Bob'})")?;
    /// tx.commit()?;
    ///
    /// // Transaction that auto-rolls back (dropped without commit)
    /// {
    ///     let mut tx = session.transaction()?;
    ///     tx.execute("CREATE (p:Person {name: 'Charlie'})")?;
    ///     // tx is dropped here, changes are rolled back
    /// }
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn transaction(&self) -> Result<Transaction<'_>> {
        Transaction::begin(self)
    }

    /// Get the internal coordinator (for internal SDK use)
    pub(crate) fn coordinator(&self) -> &QueryCoordinator {
        &self.coordinator
    }
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_connection_types_compile() {
        // Compilation test - ensures types are properly defined
    }
}
