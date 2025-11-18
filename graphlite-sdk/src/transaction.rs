//! Transaction support with ACID guarantees
//!
//! This module provides transaction support following the rusqlite pattern:
//! - Transactions automatically roll back when dropped (unless committed)
//! - RAII ensures no forgotten rollbacks
//! - Explicit commit() required to persist changes

use crate::connection::Session;
use crate::error::{Error, Result};
use graphlite::QueryResult;

/// Represents an active database transaction
///
/// Transactions provide ACID guarantees for multi-statement operations.
/// Following the rusqlite pattern:
/// - Transactions automatically **roll back** when dropped
/// - Must explicitly call `commit()` to persist changes
/// - This prevents accidentally forgetting to commit or rollback
///
/// # Examples
///
/// ```no_run
/// # use graphlite_sdk::GraphLite;
/// # let db = GraphLite::open("./mydb")?;
/// # let session = db.session("admin")?;
/// // Transaction with explicit commit
/// let mut tx = session.transaction()?;
/// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
/// tx.execute("CREATE (p:Person {name: 'Bob'})")?;
/// tx.commit()?;  // Changes are persisted
///
/// // Transaction that rolls back (dropped without commit)
/// {
///     let mut tx = session.transaction()?;
///     tx.execute("CREATE (p:Person {name: 'Charlie'})")?;
///     // tx is dropped here, changes are automatically rolled back
/// }
/// # Ok::<(), graphlite_sdk::Error>(())
/// ```
pub struct Transaction<'conn> {
    session: &'conn Session,
    committed: bool,
    drop_behavior: DropBehavior,
}

/// Behavior when a transaction is dropped
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropBehavior {
    /// Rollback the transaction when dropped (default)
    Rollback,
    /// Commit the transaction when dropped
    Commit,
    /// Panic if the transaction is dropped without explicit commit/rollback
    Panic,
    /// Do nothing when dropped (dangerous - for special cases only)
    Ignore,
}

impl<'conn> Transaction<'conn> {
    /// Begin a new transaction
    ///
    /// This is called internally by `Session::transaction()`.
    /// The transaction will automatically roll back when dropped unless committed.
    pub(crate) fn begin(session: &'conn Session) -> Result<Self> {
        // Execute BEGIN TRANSACTION
        session.coordinator()
            .process_query("BEGIN TRANSACTION", session.id())
            .map_err(|e| Error::Transaction(format!("Failed to begin transaction: {}", e)))?;

        Ok(Transaction {
            session,
            committed: false,
            drop_behavior: DropBehavior::Rollback,
        })
    }

    /// Execute a GQL statement within this transaction
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
    /// # let session = db.session("admin")?;
    /// let mut tx = session.transaction()?;
    /// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    /// tx.execute("CREATE (p:Person {name: 'Bob'})")?;
    /// tx.commit()?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn execute(&mut self, statement: &str) -> Result<()> {
        if self.committed {
            return Err(Error::Transaction("Transaction already committed".to_string()));
        }

        self.session.coordinator()
            .process_query(statement, self.session.id())
            .map_err(|e| Error::Transaction(format!("Execute failed: {}", e)))?;

        Ok(())
    }

    /// Execute a query within this transaction and return results
    ///
    /// # Arguments
    ///
    /// * `query` - GQL query to execute
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let mut tx = session.transaction()?;
    /// tx.execute("CREATE (p:Person {name: 'Alice', age: 30})")?;
    /// let result = tx.query("MATCH (p:Person) RETURN p")?;
    /// tx.commit()?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn query(&mut self, query: &str) -> Result<QueryResult> {
        if self.committed {
            return Err(Error::Transaction("Transaction already committed".to_string()));
        }

        self.session.coordinator()
            .process_query(query, self.session.id())
            .map_err(|e| Error::Transaction(format!("Query failed: {}", e)))
    }

    /// Commit the transaction
    ///
    /// Persists all changes made within this transaction. After calling commit(),
    /// the transaction is consumed and cannot be used further.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let mut tx = session.transaction()?;
    /// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    /// tx.commit()?;  // Changes are now persistent
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn commit(mut self) -> Result<()> {
        self.commit_internal()
    }

    /// Rollback the transaction
    ///
    /// Discards all changes made within this transaction. This is called
    /// automatically when the transaction is dropped, so explicit rollback
    /// is rarely needed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::GraphLite;
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let mut tx = session.transaction()?;
    /// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    /// tx.rollback()?;  // Explicit rollback (optional, automatic on drop)
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn rollback(mut self) -> Result<()> {
        self.rollback_internal()
    }

    /// Set the behavior when this transaction is dropped
    ///
    /// By default, transactions roll back when dropped. This can be changed to:
    /// - `DropBehavior::Commit` - Auto-commit on drop
    /// - `DropBehavior::Panic` - Panic if dropped without explicit commit/rollback
    /// - `DropBehavior::Ignore` - Do nothing (dangerous)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use graphlite_sdk::{GraphLite, transaction::DropBehavior};
    /// # let db = GraphLite::open("./mydb")?;
    /// # let session = db.session("admin")?;
    /// let mut tx = session.transaction()?;
    /// tx.set_drop_behavior(DropBehavior::Panic);
    /// tx.execute("CREATE (p:Person {name: 'Alice'})")?;
    /// // Must explicitly commit or rollback, or will panic on drop
    /// tx.commit()?;
    /// # Ok::<(), graphlite_sdk::Error>(())
    /// ```
    pub fn set_drop_behavior(&mut self, behavior: DropBehavior) {
        self.drop_behavior = behavior;
    }

    /// Internal commit implementation
    fn commit_internal(&mut self) -> Result<()> {
        if self.committed {
            return Err(Error::Transaction("Transaction already committed".to_string()));
        }

        self.session.coordinator()
            .process_query("COMMIT", self.session.id())
            .map_err(|e| Error::Transaction(format!("Failed to commit: {}", e)))?;

        self.committed = true;
        Ok(())
    }

    /// Internal rollback implementation
    fn rollback_internal(&mut self) -> Result<()> {
        if self.committed {
            return Ok(()); // Already committed, nothing to rollback
        }

        self.session.coordinator()
            .process_query("ROLLBACK", self.session.id())
            .map_err(|e| Error::Transaction(format!("Failed to rollback: {}", e)))?;

        self.committed = true; // Mark as finished
        Ok(())
    }
}

impl<'conn> Drop for Transaction<'conn> {
    fn drop(&mut self) {
        if self.committed {
            return; // Already committed or rolled back
        }

        match self.drop_behavior {
            DropBehavior::Rollback => {
                // Attempt to rollback, log error if it fails
                if let Err(e) = self.rollback_internal() {
                    eprintln!("Warning: Failed to rollback transaction on drop: {}", e);
                }
            }
            DropBehavior::Commit => {
                // Attempt to commit, log error if it fails
                if let Err(e) = self.commit_internal() {
                    eprintln!("Warning: Failed to commit transaction on drop: {}", e);
                }
            }
            DropBehavior::Panic => {
                if !std::thread::panicking() {
                    panic!("Transaction dropped without explicit commit or rollback");
                }
            }
            DropBehavior::Ignore => {
                // Do nothing
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_behavior() {
        assert_eq!(DropBehavior::Rollback, DropBehavior::Rollback);
        assert_ne!(DropBehavior::Rollback, DropBehavior::Commit);
    }
}
