// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Transaction state management
//!
//! This module defines the transaction state and lifecycle management.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique identifier for a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(u64);

impl TransactionId {
    /// Generate a new unique transaction ID based on system time
    pub fn new() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        TransactionId(timestamp)
    }

    /// Get the underlying ID value
    pub fn id(&self) -> u64 {
        self.0
    }

    /// Create TransactionId from u64 (used by WAL deserialization)
    pub fn from_u64(id: u64) -> Self {
        TransactionId(id)
    }
}

impl std::fmt::Display for TransactionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "txn_{}", self.0)
    }
}

/// Transaction lifecycle states
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionStatus {
    /// Transaction is active and can perform operations
    Active,
    /// Transaction is preparing to commit (2PC first phase)
    Preparing,
    /// Transaction has been committed successfully
    Committed,
    /// Transaction has been rolled back
    RolledBack,
    /// Transaction is in an error state
    Failed(String),
}

/// Transaction access mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

/// Transaction isolation level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TxnIsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Complete transaction state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionState {
    /// Unique transaction identifier
    pub id: TransactionId,
    /// Session ID that owns this transaction
    pub session_id: Option<String>,
    /// Current transaction status
    pub status: TransactionStatus,
    /// Transaction isolation level
    pub isolation_level: TxnIsolationLevel,
    /// Transaction access mode
    pub access_mode: AccessMode,
    /// Timestamp when transaction started
    pub start_time: SystemTime,
    /// Timestamp when transaction ended (if applicable)
    pub end_time: Option<SystemTime>,
    /// List of operations performed in this transaction
    pub operations: Vec<TransactionOperation>,
    /// Sequence number for tracking operation order
    pub sequence_number: u64,
}

/// Record of an operation performed within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOperation {
    /// Type of operation
    pub operation_type: OperationType,
    /// Timestamp when operation was performed
    pub timestamp: SystemTime,
    /// Description of the operation
    pub description: String,
    /// Sequence number of this operation within the transaction
    pub sequence_number: u64,
}

/// Types of operations that can be performed in a transaction
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationType {
    /// Read operations
    Select,
    Match,
    /// Write operations
    Insert,
    Update,
    Set,
    Delete,
    Remove,
    /// Schema operations
    CreateTable,
    CreateGraph,
    AlterTable,
    DropTable,
    DropGraph,
    /// Security operations
    CreateUser,
    DropUser,
    CreateRole,
    DropRole,
    GrantRole,
    RevokeRole,
    /// Transaction control
    Begin,
    Commit,
    Rollback,
    /// Other operation types
    Other,
}

impl TransactionState {
    /// Create a new transaction with default settings
    pub fn new(isolation_level: TxnIsolationLevel, access_mode: AccessMode) -> Self {
        Self {
            id: TransactionId::new(),
            session_id: None,
            status: TransactionStatus::Active,
            isolation_level,
            access_mode,
            start_time: SystemTime::now(),
            end_time: None,
            operations: Vec::new(),
            sequence_number: 0,
        }
    }

    /// Create a new transaction with session context
    pub fn new_with_session(
        isolation_level: TxnIsolationLevel,
        access_mode: AccessMode,
        session_id: String,
    ) -> Self {
        Self {
            id: TransactionId::new(),
            session_id: Some(session_id),
            status: TransactionStatus::Active,
            isolation_level,
            access_mode,
            start_time: SystemTime::now(),
            end_time: None,
            operations: Vec::new(),
            sequence_number: 0,
        }
    }

    /// Add an operation to this transaction
    pub fn add_operation(&mut self, operation_type: OperationType, description: String) {
        // Increment sequence number for data modification operations
        if matches!(
            operation_type,
            OperationType::Insert
                | OperationType::Update
                | OperationType::Set
                | OperationType::Delete
                | OperationType::Remove
        ) {
            self.sequence_number += 1;
        }

        self.operations.push(TransactionOperation {
            operation_type,
            timestamp: SystemTime::now(),
            description,
            sequence_number: self.sequence_number,
        });
    }

    /// Mark transaction as committed
    pub fn commit(&mut self) {
        self.status = TransactionStatus::Committed;
        self.end_time = Some(SystemTime::now());
    }

    /// Mark transaction as rolled back
    pub fn rollback(&mut self) {
        self.status = TransactionStatus::RolledBack;
        self.end_time = Some(SystemTime::now());
    }

    /// Mark transaction as failed with error message
    pub fn fail(&mut self, error: String) {
        self.status = TransactionStatus::Failed(error);
        self.end_time = Some(SystemTime::now());
    }

    /// Check if transaction is active
    pub fn is_active(&self) -> bool {
        self.status == TransactionStatus::Active
    }

    /// Check if transaction is read-only
    pub fn is_read_only(&self) -> bool {
        self.access_mode == AccessMode::ReadOnly
    }

    /// Get transaction duration
    pub fn duration(&self) -> std::time::Duration {
        let end_time = self.end_time.unwrap_or_else(SystemTime::now);
        end_time.duration_since(self.start_time).unwrap_or_default()
    }

    /// Get current sequence number
    pub fn get_sequence_number(&self) -> u64 {
        self.sequence_number
    }
}

impl Default for TransactionState {
    fn default() -> Self {
        Self::new(TxnIsolationLevel::ReadCommitted, AccessMode::ReadWrite)
    }
}
