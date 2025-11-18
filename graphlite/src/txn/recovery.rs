// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! WAL Recovery Manager for crash recovery and replay operations

use std::collections::HashMap;
use chrono::{DateTime, Utc};

use super::wal::{PersistentWAL, WALEntry, WALEntryType, WALError};
use super::state::{TransactionId, OperationType};
use crate::storage::StorageManager;

/// Recovery Manager for WAL-based crash recovery
#[allow(dead_code)] // ROADMAP v0.3.0 - WAL-based crash recovery manager (3-phase ARIES-style)
pub struct RecoveryManager {
    /// WAL instance to read from
    wal: PersistentWAL,
    /// Tracks recovered transaction states
    recovered_transactions: HashMap<TransactionId, TransactionRecoveryState>,
    /// Storage manager for applying recovered operations
    storage_manager: Option<StorageManager>,
}

/// State of a transaction during recovery
#[derive(Debug, Clone)]
#[allow(dead_code)] // ROADMAP v0.3.0 - Transaction state tracking during recovery
pub struct TransactionRecoveryState {
    pub transaction_id: TransactionId,
    pub status: RecoveryStatus,
    pub operations: Vec<WALEntry>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

/// Status of a transaction during recovery
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)] // ROADMAP v0.3.0 - Recovery status enumeration (InProgress/Committed/RolledBack/NeedsAbort)
pub enum RecoveryStatus {
    /// Transaction was started but not completed
    InProgress,
    /// Transaction was successfully committed
    Committed,
    /// Transaction was rolled back
    RolledBack,
    /// Transaction needs to be aborted during recovery
    NeedsAbort,
}

impl RecoveryManager {
    /// Create a new RecoveryManager with database path
    ///
    /// # Arguments
    /// * `db_path` - The database directory path where WAL files are stored
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery (see ROADMAP.md §3)
    pub fn new(db_path: std::path::PathBuf) -> Self {
        let wal = PersistentWAL::new(db_path)
            .expect("Failed to initialize WAL for recovery");

        Self {
            wal,
            recovered_transactions: HashMap::new(),
            storage_manager: None,
        }
    }

    /// Create RecoveryManager with storage manager
    ///
    /// # Arguments
    /// * `db_path` - The database directory path where WAL files are stored
    /// * `storage_manager` - The storage manager for recovery operations
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery (see ROADMAP.md §3)
    pub fn with_storage(db_path: std::path::PathBuf, storage_manager: StorageManager) -> Self {
        let wal = PersistentWAL::new(db_path)
            .expect("Failed to initialize WAL for recovery");

        Self {
            wal,
            recovered_transactions: HashMap::new(),
            storage_manager: Some(storage_manager),
        }
    }

    /// Perform crash recovery by replaying WAL
    #[allow(dead_code)] // ROADMAP v0.3.0 - Main recovery procedure (Analysis→Redo→Undo phases)
    pub fn recover(&mut self) -> Result<RecoveryReport, RecoveryError> {
        let mut report = RecoveryReport::new();
        
        // Phase 1: Analysis - Scan WAL to determine transaction states
        self.analysis_phase(&mut report)?;
        
        // Phase 2: Redo - Replay committed transactions
        self.redo_phase(&mut report)?;
        
        // Phase 3: Undo - Rollback incomplete transactions
        self.undo_phase(&mut report)?;
        
        Ok(report)
    }

    /// Analysis phase: scan WAL to determine transaction states
    fn analysis_phase(&mut self, report: &mut RecoveryReport) -> Result<(), RecoveryError> {
        log::debug!("🔍 Starting WAL analysis phase...");
        
        // Read all WAL files
        let mut file_number = 1u64;
        loop {
            match self.wal.read_wal_file(file_number) {
                Ok(entries) => {
                    report.total_wal_entries += entries.len();
                    
                    for entry in entries {
                        self.process_entry_analysis(entry)?;
                    }
                    
                    file_number += 1;
                }
                Err(WALError::IOError(msg)) if msg.contains("not found") => {
                    // No more WAL files
                    break;
                }
                Err(e) => {
                    return Err(RecoveryError::WALReadError(e.to_string()));
                }
            }
        }
        
        // Determine which transactions need recovery
        for (txn_id, state) in &self.recovered_transactions {
            match state.status {
                RecoveryStatus::InProgress => {
                    report.incomplete_transactions.push(*txn_id);
                }
                RecoveryStatus::Committed => {
                    report.committed_transactions.push(*txn_id);
                }
                RecoveryStatus::RolledBack => {
                    report.rolled_back_transactions.push(*txn_id);
                }
                _ => {}
            }
        }
        
        log::debug!("✅ Analysis phase complete: {} transactions to recover", 
                 report.incomplete_transactions.len() + report.committed_transactions.len());
        
        Ok(())
    }

    /// Process a WAL entry during analysis
    fn process_entry_analysis(&mut self, entry: WALEntry) -> Result<(), RecoveryError> {
        let txn_id = entry.transaction_id;
        
        match entry.entry_type {
            WALEntryType::TransactionBegin => {
                let state = TransactionRecoveryState {
                    transaction_id: txn_id,
                    status: RecoveryStatus::InProgress,
                    operations: Vec::new(),
                    start_time: DateTime::<Utc>::from(entry.timestamp),
                    end_time: None,
                };
                self.recovered_transactions.insert(txn_id, state);
            }
            
            WALEntryType::TransactionCommit => {
                if let Some(state) = self.recovered_transactions.get_mut(&txn_id) {
                    state.status = RecoveryStatus::Committed;
                    state.end_time = Some(DateTime::<Utc>::from(entry.timestamp));
                }
            }
            
            WALEntryType::TransactionRollback => {
                if let Some(state) = self.recovered_transactions.get_mut(&txn_id) {
                    state.status = RecoveryStatus::RolledBack;
                    state.end_time = Some(DateTime::<Utc>::from(entry.timestamp));
                }
            }
            
            WALEntryType::TransactionOperation => {
                if let Some(state) = self.recovered_transactions.get_mut(&txn_id) {
                    state.operations.push(entry);
                }
            }
        }
        
        Ok(())
    }

    /// Redo phase: replay committed transactions
    fn redo_phase(&mut self, report: &mut RecoveryReport) -> Result<(), RecoveryError> {
        log::debug!("🔄 Starting redo phase...");
        
        let txn_ids = report.committed_transactions.clone();
        for txn_id in &txn_ids {
            // Clone operations to avoid borrow issues
            let operations = if let Some(state) = self.recovered_transactions.get(txn_id) {
                log::debug!("  Replaying committed transaction: {}", txn_id.id());
                state.operations.clone()
            } else {
                continue;
            };
            
            for operation in &operations {
                // Apply operation if storage manager is available
                if self.storage_manager.is_some() {
                    self.apply_operation(operation)?;
                }
                report.operations_replayed += 1;
            }
        }
        
        log::debug!("✅ Redo phase complete: {} operations replayed", report.operations_replayed);
        Ok(())
    }

    /// Undo phase: rollback incomplete transactions
    fn undo_phase(&mut self, report: &mut RecoveryReport) -> Result<(), RecoveryError> {
        log::debug!("↩️ Starting undo phase...");
        
        let txn_ids = report.incomplete_transactions.clone();
        for txn_id in &txn_ids {
            // Clone operations to avoid borrow issues
            let operations = if let Some(state) = self.recovered_transactions.get(txn_id) {
                log::debug!("  Rolling back incomplete transaction: {}", txn_id.id());
                state.operations.clone()
            } else {
                continue;
            };
            
            // Process operations in reverse order for undo
            for operation in operations.iter().rev() {
                // Generate compensating operation if needed
                if self.storage_manager.is_some() {
                    self.undo_operation(operation)?;
                }
                report.operations_undone += 1;
            }
        }
        
        log::debug!("✅ Undo phase complete: {} operations undone", report.operations_undone);
        Ok(())
    }

    /// Apply a recovered operation
    fn apply_operation(&mut self, entry: &WALEntry) -> Result<(), RecoveryError> {
        // This would integrate with the actual storage operations
        match entry.operation_type {
            Some(OperationType::Insert) | 
            Some(OperationType::Update) |
            Some(OperationType::Delete) => {
                // Apply data modification
                // Note: Actual implementation would parse entry.description 
                // and apply to storage_manager
                log::debug!("    Applying: {:?} - {}", entry.operation_type, entry.description);
            }
            
            Some(OperationType::CreateTable) |
            Some(OperationType::CreateGraph) |
            Some(OperationType::DropTable) |
            Some(OperationType::DropGraph) => {
                // Apply schema changes
                log::debug!("    Applying schema change: {:?}", entry.operation_type);
            }
            
            _ => {
                // Skip non-modifying operations
            }
        }
        
        Ok(())
    }

    /// Undo a recovered operation
    fn undo_operation(&mut self, entry: &WALEntry) -> Result<(), RecoveryError> {
        // Generate and apply compensating operation
        match entry.operation_type {
            Some(OperationType::Insert) => {
                // Compensate with delete
                log::debug!("    Undoing insert: {}", entry.description);
            }
            
            Some(OperationType::Delete) => {
                // Compensate with insert (if we have before-image)
                log::debug!("    Undoing delete: {}", entry.description);
            }
            
            Some(OperationType::Update) => {
                // Compensate with reverse update (if we have before-image)
                log::debug!("    Undoing update: {}", entry.description);
            }
            
            _ => {
                // Some operations may not need compensation
            }
        }
        
        Ok(())
    }

    /// Check if a specific transaction was recovered
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery (see ROADMAP.md §3)
    pub fn was_transaction_recovered(&self, txn_id: &TransactionId) -> bool {
        self.recovered_transactions.contains_key(txn_id)
    }

    /// Get the recovery state of a transaction
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery (see ROADMAP.md §3)
    pub fn get_transaction_state(&self, txn_id: &TransactionId) -> Option<&TransactionRecoveryState> {
        self.recovered_transactions.get(txn_id)
    }
}

/// Report of recovery operations
#[derive(Debug)]
#[allow(dead_code)] // ROADMAP v0.6.0 - Recovery statistics reporting for observability
pub struct RecoveryReport {
    pub total_wal_entries: usize,
    pub committed_transactions: Vec<TransactionId>,
    pub rolled_back_transactions: Vec<TransactionId>,
    pub incomplete_transactions: Vec<TransactionId>,
    pub operations_replayed: usize,
    pub operations_undone: usize,
    pub recovery_time_ms: u64,
}

impl RecoveryReport {
    fn new() -> Self {
        Self {
            total_wal_entries: 0,
            committed_transactions: Vec::new(),
            rolled_back_transactions: Vec::new(),
            incomplete_transactions: Vec::new(),
            operations_replayed: 0,
            operations_undone: 0,
            recovery_time_ms: 0,
        }
    }
}

/// Recovery-specific errors
#[derive(Debug)]
pub enum RecoveryError {
    WALReadError(String),
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery error handling (see ROADMAP.md §3)
    OperationReplayError(String),
    #[allow(dead_code)] // ROADMAP v0.3.0 - ARIES-style crash recovery error handling (see ROADMAP.md §3)
    StorageError(String),
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoveryError::WALReadError(msg) => write!(f, "WAL Read Error: {}", msg),
            RecoveryError::OperationReplayError(msg) => write!(f, "Operation Replay Error: {}", msg),
            RecoveryError::StorageError(msg) => write!(f, "Storage Error: {}", msg),
        }
    }
}

impl std::error::Error for RecoveryError {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_recovery_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = RecoveryManager::new(temp_dir.path().to_path_buf());
        assert!(manager.recovered_transactions.is_empty());
    }

    #[test]
    fn test_empty_recovery() {
        // Create temp directory for WAL
        let temp_dir = TempDir::new().unwrap();

        // Create manager with explicit path
        let mut manager = RecoveryManager::new(temp_dir.path().to_path_buf());
        
        let report = manager.recover().unwrap();
        assert_eq!(report.total_wal_entries, 0);
        assert_eq!(report.committed_transactions.len(), 0);
        assert_eq!(report.incomplete_transactions.len(), 0);
    }
}
