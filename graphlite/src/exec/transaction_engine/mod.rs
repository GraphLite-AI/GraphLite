// Transaction Engine - Transaction Management
//
// This module handles transaction lifecycle (BEGIN, COMMIT, ROLLBACK) in GraphLite.
// It provides ACID guarantees and isolation management.

pub mod operations;
pub mod isolation;
