// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Transaction manager implementation
//!
//! This module provides the main transaction management functionality.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::exec::error::ExecutionError;
use crate::session::SessionManager;

use super::isolation::IsolationLevel;
use super::state::{AccessMode, OperationType, TransactionId, TransactionState, TxnIsolationLevel};
use super::wal::{PersistentWAL, WALEntry, WALEntryType};

/// Transaction manager handles the lifecycle of all transactions
pub struct TransactionManager {
    /// Map of active transactions by ID
    active_transactions: Arc<RwLock<HashMap<TransactionId, Arc<Mutex<TransactionState>>>>>,
    /// Default transaction characteristics
    default_isolation_level: IsolationLevel,
    /// Next transaction characteristics (set by SET TRANSACTION)
    next_transaction_characteristics: Arc<Mutex<Option<(IsolationLevel, AccessMode)>>>,
    /// Persistent Write-Ahead Log
    wal: Arc<PersistentWAL>,
    /// Session manager for transaction-session association
    session_manager: Option<Arc<SessionManager>>,
}

impl TransactionManager {
    /// Create a new transaction manager with database path
    ///
    /// # Arguments
    /// * `db_path` - The database directory path. Used for WAL storage.
    ///
    /// # Returns
    /// A new TransactionManager instance with WAL initialized in the database directory
    pub fn new(db_path: std::path::PathBuf) -> Result<Self, ExecutionError> {
        let wal = PersistentWAL::new(db_path).map_err(|e| {
            ExecutionError::RuntimeError(format!("Failed to initialize WAL: {}", e))
        })?;

        Ok(Self {
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            default_isolation_level: IsolationLevel::ReadCommitted,
            next_transaction_characteristics: Arc::new(Mutex::new(None)),
            wal: Arc::new(wal),
            session_manager: None,
        })
    }

    /// Start a new transaction
    pub fn start_transaction(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<TransactionId, ExecutionError> {
        self.start_transaction_with_session(isolation_level, access_mode, None)
    }

    /// Start a new transaction with session context
    pub fn start_transaction_with_session(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
        session_id: Option<String>,
    ) -> Result<TransactionId, ExecutionError> {
        // Determine transaction characteristics
        let (final_isolation_level, final_access_mode) = {
            let next_characteristics = self
                .next_transaction_characteristics
                .lock()
                .map_err(|_| ExecutionError::RuntimeError("Failed to acquire lock".to_string()))?;

            match (isolation_level, access_mode, next_characteristics.as_ref()) {
                // Explicit characteristics provided
                (Some(iso), Some(acc), _) => (iso, acc),
                (Some(iso), None, Some((_, next_acc))) => (iso, *next_acc),
                (Some(iso), None, None) => (iso, AccessMode::ReadWrite),
                (None, Some(acc), Some((next_iso, _))) => (*next_iso, acc),
                (None, Some(acc), None) => (self.default_isolation_level, acc),
                // Use next characteristics if set
                (None, None, Some((next_iso, next_acc))) => (*next_iso, *next_acc),
                // Use defaults
                (None, None, None) => (self.default_isolation_level, AccessMode::ReadWrite),
            }
        };

        // Clear next characteristics after using them
        *self
            .next_transaction_characteristics
            .lock()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire lock".to_string()))? =
            None;

        // Convert to transaction module types
        let txn_isolation_level = match final_isolation_level {
            IsolationLevel::ReadUncommitted => TxnIsolationLevel::ReadUncommitted,
            IsolationLevel::ReadCommitted => TxnIsolationLevel::ReadCommitted,
            IsolationLevel::RepeatableRead => TxnIsolationLevel::RepeatableRead,
            IsolationLevel::Serializable => TxnIsolationLevel::Serializable,
        };

        // Check if isolation level is supported
        if !matches!(final_isolation_level, IsolationLevel::ReadCommitted) {
            return Err(ExecutionError::UnsupportedOperator(
                format!("Isolation level {} not yet supported. Only READ COMMITTED is currently implemented.", 
                        final_isolation_level.as_str())
            ));
        }

        // Validate session exists if provided
        if let Some(ref session_id) = session_id {
            if let Some(ref session_manager) = self.session_manager {
                if session_manager.get_session(session_id).is_none() {
                    return Err(ExecutionError::RuntimeError(format!(
                        "Session {} not found",
                        session_id
                    )));
                }

                // Session activity is managed by the session itself
                // No explicit update needed here
            }
        }

        // Create new transaction with optional session context
        let mut transaction = if let Some(session_id) = session_id.clone() {
            TransactionState::new_with_session(txn_isolation_level, final_access_mode, session_id)
        } else {
            TransactionState::new(txn_isolation_level, final_access_mode)
        };
        let transaction_id = transaction.id;

        // Write BEGIN entry to WAL with session context
        let begin_description = if let Some(ref session_id) = session_id {
            format!(
                "BEGIN TRANSACTION (Session: {}) - {} isolation level, {} access mode",
                session_id,
                final_isolation_level.as_str(),
                if final_access_mode == AccessMode::ReadOnly {
                    "READ ONLY"
                } else {
                    "READ WRITE"
                }
            )
        } else {
            format!(
                "BEGIN TRANSACTION - {} isolation level, {} access mode",
                final_isolation_level.as_str(),
                if final_access_mode == AccessMode::ReadOnly {
                    "READ ONLY"
                } else {
                    "READ WRITE"
                }
            )
        };

        let wal_entry = WALEntry::new(
            WALEntryType::Begin,
            transaction_id,
            self.wal.next_global_sequence(),
            0,    // BEGIN is sequence 0
            None, // No operation type for BEGIN
            begin_description.clone(),
        );

        if let Err(e) = self.wal.write_entry(wal_entry) {
            return Err(ExecutionError::RuntimeError(format!(
                "Failed to write BEGIN to WAL: {}",
                e
            )));
        }

        // Also log to in-memory transaction log
        transaction.add_operation(OperationType::Other, begin_description);

        // Add to active transactions
        let mut active_txns = self.active_transactions.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        active_txns.insert(transaction_id, Arc::new(Mutex::new(transaction)));

        Ok(transaction_id)
    }

    /// Commit a transaction
    pub fn commit_transaction(&self, transaction_id: TransactionId) -> Result<(), ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        if let Some(txn_arc) = active_txns.get(&transaction_id) {
            let mut transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            if !transaction.is_active() {
                return Err(ExecutionError::RuntimeError(format!(
                    "Transaction {} is not active",
                    transaction_id
                )));
            }

            let final_sequence = transaction.get_sequence_number();
            let commit_description = format!(
                "COMMIT TRANSACTION - final sequence number: {}",
                final_sequence
            );

            // Write COMMIT entry to WAL
            let wal_entry = WALEntry::new(
                WALEntryType::Commit,
                transaction_id,
                self.wal.next_global_sequence(),
                final_sequence,
                None,
                commit_description.clone(),
            );

            if let Err(e) = self.wal.write_entry(wal_entry) {
                return Err(ExecutionError::RuntimeError(format!(
                    "Failed to write COMMIT to WAL: {}",
                    e
                )));
            }

            // Also log to in-memory transaction log
            transaction.add_operation(OperationType::Other, commit_description);
            transaction.commit();

            Ok(())
        } else {
            Err(ExecutionError::RuntimeError(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Rollback a transaction
    pub fn rollback_transaction(
        &self,
        transaction_id: TransactionId,
    ) -> Result<(), ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        if let Some(txn_arc) = active_txns.get(&transaction_id) {
            let mut transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            if !transaction.is_active() {
                return Err(ExecutionError::RuntimeError(format!(
                    "Transaction {} is not active",
                    transaction_id
                )));
            }

            let final_sequence = transaction.get_sequence_number();
            let rollback_description = format!(
                "ROLLBACK TRANSACTION - final sequence number: {}",
                final_sequence
            );

            // Write ROLLBACK entry to WAL
            let wal_entry = WALEntry::new(
                WALEntryType::Rollback,
                transaction_id,
                self.wal.next_global_sequence(),
                final_sequence,
                None,
                rollback_description.clone(),
            );

            if let Err(e) = self.wal.write_entry(wal_entry) {
                return Err(ExecutionError::RuntimeError(format!(
                    "Failed to write ROLLBACK to WAL: {}",
                    e
                )));
            }

            // Also log to in-memory transaction log
            transaction.add_operation(OperationType::Other, rollback_description);
            transaction.rollback();

            Ok(())
        } else {
            Err(ExecutionError::RuntimeError(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Set characteristics for the next transaction
    pub fn set_next_transaction_characteristics(
        &self,
        isolation_level: Option<IsolationLevel>,
        access_mode: Option<AccessMode>,
    ) -> Result<(), ExecutionError> {
        let mut next_characteristics = self
            .next_transaction_characteristics
            .lock()
            .map_err(|_| ExecutionError::RuntimeError("Failed to acquire lock".to_string()))?;

        // Only update provided characteristics, keep existing ones for unspecified
        let current = next_characteristics.as_ref();
        let final_isolation_level = isolation_level.unwrap_or_else(|| {
            current
                .map(|(iso, _)| *iso)
                .unwrap_or(self.default_isolation_level)
        });
        let final_access_mode = access_mode.unwrap_or_else(|| {
            current
                .map(|(_, acc)| *acc)
                .unwrap_or(AccessMode::ReadWrite)
        });

        *next_characteristics = Some((final_isolation_level, final_access_mode));
        Ok(())
    }

    /// Get transaction state
    pub fn get_transaction(
        &self,
        transaction_id: TransactionId,
    ) -> Result<Option<Arc<Mutex<TransactionState>>>, ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        Ok(active_txns.get(&transaction_id).cloned())
    }

    /// Get all active transaction IDs
    pub fn get_active_transaction_ids(&self) -> Result<Vec<TransactionId>, ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        let mut active_ids = Vec::new();
        for (id, txn_arc) in active_txns.iter() {
            let transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;
            if transaction.is_active() {
                active_ids.push(*id);
            }
        }

        Ok(active_ids)
    }

    /// Clean up completed transactions (should be called periodically)
    pub fn cleanup_completed_transactions(&self) -> Result<usize, ExecutionError> {
        let mut active_txns = self.active_transactions.write().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        let mut to_remove = Vec::new();

        for (id, txn_arc) in active_txns.iter() {
            let transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;
            if !transaction.is_active() {
                to_remove.push(*id);
            }
        }

        let removed_count = to_remove.len();
        for id in to_remove {
            active_txns.remove(&id);
        }

        Ok(removed_count)
    }

    /// Log a transaction operation to WAL and in-memory log
    pub fn log_operation(
        &self,
        transaction_id: TransactionId,
        operation_type: OperationType,
        description: String,
    ) -> Result<(), ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        if let Some(txn_arc) = active_txns.get(&transaction_id) {
            let mut transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            if !transaction.is_active() {
                return Err(ExecutionError::RuntimeError(format!(
                    "Transaction {} is not active",
                    transaction_id
                )));
            }

            // Add to in-memory log first to get the sequence number
            transaction.add_operation(operation_type.clone(), description.clone());
            let txn_sequence = transaction.get_sequence_number();

            // Write operation entry to WAL
            let wal_entry = WALEntry::new(
                WALEntryType::Operation,
                transaction_id,
                self.wal.next_global_sequence(),
                txn_sequence,
                Some(operation_type),
                description,
            );

            if let Err(e) = self.wal.write_entry(wal_entry) {
                return Err(ExecutionError::RuntimeError(format!(
                    "Failed to write operation to WAL: {}",
                    e
                )));
            }

            Ok(())
        } else {
            Err(ExecutionError::RuntimeError(format!(
                "Transaction {} not found",
                transaction_id
            )))
        }
    }

    /// Get all active transactions for a session
    pub fn get_session_transactions(
        &self,
        session_id: &str,
    ) -> Result<Vec<TransactionId>, ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        let mut session_txns = Vec::new();

        for (id, txn_arc) in active_txns.iter() {
            let transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            if transaction.is_active() {
                if let Some(ref txn_session_id) = transaction.session_id {
                    if txn_session_id == session_id {
                        session_txns.push(*id);
                    }
                }
            }
        }

        Ok(session_txns)
    }

    /// Get current graph for a transaction based on its session
    pub fn get_transaction_current_graph(
        &self,
        transaction_id: TransactionId,
    ) -> Result<Option<String>, ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        if let Some(txn_arc) = active_txns.get(&transaction_id) {
            let transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            if let Some(ref session_id) = transaction.session_id {
                if let Some(ref session_manager) = self.session_manager {
                    if let Some(session_arc) = session_manager.get_session(session_id) {
                        if let Ok(session) = session_arc.read() {
                            return Ok(session.current_graph.clone());
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Get transaction statistics
    pub fn get_statistics(&self) -> Result<TransactionStatistics, ExecutionError> {
        let active_txns = self.active_transactions.read().map_err(|_| {
            ExecutionError::RuntimeError("Failed to acquire transactions lock".to_string())
        })?;

        let mut stats = TransactionStatistics::default();

        for txn_arc in active_txns.values() {
            let transaction = txn_arc.lock().map_err(|_| {
                ExecutionError::RuntimeError("Failed to acquire transaction lock".to_string())
            })?;

            stats.total_transactions += 1;

            match &transaction.status {
                super::state::TransactionStatus::Active => stats.active_transactions += 1,
                super::state::TransactionStatus::Committed => stats.committed_transactions += 1,
                super::state::TransactionStatus::RolledBack => stats.rolled_back_transactions += 1,
                super::state::TransactionStatus::Failed(_) => stats.failed_transactions += 1,
                _ => {}
            }
        }

        Ok(stats)
    }
}

/// Transaction statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct TransactionStatistics {
    pub total_transactions: u64,
    pub active_transactions: u64,
    pub committed_transactions: u64,
    pub rolled_back_transactions: u64,
    pub failed_transactions: u64,
}
