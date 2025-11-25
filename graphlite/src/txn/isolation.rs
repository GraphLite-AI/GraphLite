// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Transaction isolation level management
//!
//! This module defines isolation levels and their behavior for transaction management.

use serde::{Deserialize, Serialize};

/// Transaction isolation levels as defined in SQL standard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsolationLevel {
    /// READ UNCOMMITTED - Allows dirty reads, non-repeatable reads, and phantom reads
    ReadUncommitted,
    /// READ COMMITTED - Prevents dirty reads, but allows non-repeatable reads and phantom reads
    ReadCommitted,
    /// REPEATABLE READ - Prevents dirty reads and non-repeatable reads, but allows phantom reads
    RepeatableRead,
    /// SERIALIZABLE - Prevents dirty reads, non-repeatable reads, and phantom reads
    Serializable,
}

impl IsolationLevel {
    /// Get string representation for display
    pub fn as_str(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }

    /// Check if this isolation level allows dirty reads
    /// Dirty read: Reading uncommitted changes from other transactions
    pub fn allows_dirty_reads(&self) -> bool {
        matches!(self, IsolationLevel::ReadUncommitted)
    }

    /// Check if this isolation level allows non-repeatable reads
    /// Non-repeatable read: Reading different values for the same data in a single transaction
    pub fn allows_non_repeatable_reads(&self) -> bool {
        matches!(
            self,
            IsolationLevel::ReadUncommitted | IsolationLevel::ReadCommitted
        )
    }

    /// Check if this isolation level allows phantom reads
    /// Phantom read: New rows appearing in repeated queries within the same transaction
    pub fn allows_phantom_reads(&self) -> bool {
        matches!(
            self,
            IsolationLevel::ReadUncommitted
                | IsolationLevel::ReadCommitted
                | IsolationLevel::RepeatableRead
        )
    }

    /// Get the strictness level (higher number = more strict)
    pub fn strictness_level(&self) -> u8 {
        match self {
            IsolationLevel::ReadUncommitted => 0,
            IsolationLevel::ReadCommitted => 1,
            IsolationLevel::RepeatableRead => 2,
            IsolationLevel::Serializable => 3,
        }
    }

    /// Check if this isolation level is at least as strict as another
    pub fn is_at_least_as_strict_as(&self, other: &IsolationLevel) -> bool {
        self.strictness_level() >= other.strictness_level()
    }

    /// Get the default isolation level for the system
    pub fn default() -> Self {
        IsolationLevel::ReadCommitted
    }
}

impl std::fmt::Display for IsolationLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for IsolationLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "READ UNCOMMITTED" | "READ_UNCOMMITTED" => Ok(IsolationLevel::ReadUncommitted),
            "READ COMMITTED" | "READ_COMMITTED" => Ok(IsolationLevel::ReadCommitted),
            "REPEATABLE READ" | "REPEATABLE_READ" => Ok(IsolationLevel::RepeatableRead),
            "SERIALIZABLE" => Ok(IsolationLevel::Serializable),
            _ => Err(format!("Unknown isolation level: {}", s)),
        }
    }
}

/// Convert from AST isolation level to transaction isolation level
impl From<crate::ast::IsolationLevel> for IsolationLevel {
    fn from(ast_level: crate::ast::IsolationLevel) -> Self {
        match ast_level {
            crate::ast::IsolationLevel::ReadUncommitted => IsolationLevel::ReadUncommitted,
            crate::ast::IsolationLevel::ReadCommitted => IsolationLevel::ReadCommitted,
            crate::ast::IsolationLevel::RepeatableRead => IsolationLevel::RepeatableRead,
            crate::ast::IsolationLevel::Serializable => IsolationLevel::Serializable,
        }
    }
}

/// Convert to AST isolation level from transaction isolation level
impl From<IsolationLevel> for crate::ast::IsolationLevel {
    fn from(txn_level: IsolationLevel) -> Self {
        match txn_level {
            IsolationLevel::ReadUncommitted => crate::ast::IsolationLevel::ReadUncommitted,
            IsolationLevel::ReadCommitted => crate::ast::IsolationLevel::ReadCommitted,
            IsolationLevel::RepeatableRead => crate::ast::IsolationLevel::RepeatableRead,
            IsolationLevel::Serializable => crate::ast::IsolationLevel::Serializable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolation_level_strictness() {
        assert!(
            IsolationLevel::Serializable.is_at_least_as_strict_as(&IsolationLevel::ReadCommitted)
        );
        assert!(IsolationLevel::ReadCommitted
            .is_at_least_as_strict_as(&IsolationLevel::ReadUncommitted));
        assert!(!IsolationLevel::ReadUncommitted
            .is_at_least_as_strict_as(&IsolationLevel::Serializable));
    }

    #[test]
    fn test_isolation_level_properties() {
        assert!(IsolationLevel::ReadUncommitted.allows_dirty_reads());
        assert!(!IsolationLevel::ReadCommitted.allows_dirty_reads());

        assert!(IsolationLevel::ReadCommitted.allows_non_repeatable_reads());
        assert!(!IsolationLevel::RepeatableRead.allows_non_repeatable_reads());

        assert!(IsolationLevel::RepeatableRead.allows_phantom_reads());
        assert!(!IsolationLevel::Serializable.allows_phantom_reads());
    }

    #[test]
    fn test_isolation_level_parsing() {
        assert_eq!(
            "READ COMMITTED".parse::<IsolationLevel>().unwrap(),
            IsolationLevel::ReadCommitted
        );
        assert_eq!(
            "serializable".parse::<IsolationLevel>().unwrap(),
            IsolationLevel::Serializable
        );
    }
}
