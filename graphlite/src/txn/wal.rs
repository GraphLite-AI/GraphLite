// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Write-Ahead Log (WAL) implementation with persistent storage
//!
//! This module provides a high-performance WAL system that writes transaction
//! operations to disk in a compact binary format for durability and recovery.

use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use super::state::{OperationType, TransactionId};

/// Magic number to identify WAL files
const WAL_MAGIC: u32 = 0x53594E57;
/// Current WAL format version
const WAL_VERSION: u16 = 1;
/// Maximum size per WAL file (64MB)
const MAX_WAL_FILE_SIZE: u64 = 64 * 1024 * 1024;

/// WAL entry types for binary format
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WALEntryType {
    TransactionBegin = 1,
    TransactionOperation = 2,
    TransactionCommit = 3,
    TransactionRollback = 4,
}

/// A single WAL entry that gets written to disk
#[derive(Debug, Clone)]
pub struct WALEntry {
    /// Type of WAL entry
    pub entry_type: WALEntryType,
    /// Transaction ID this entry belongs to
    pub transaction_id: TransactionId,
    /// Global sequence number across all transactions
    pub global_sequence: u64,
    /// Transaction-specific sequence number
    pub txn_sequence: u64,
    /// Timestamp when entry was created
    pub timestamp: SystemTime,
    /// Operation type (for operation entries)
    pub operation_type: Option<OperationType>,
    /// Description/payload of the operation
    pub description: String,
}

/// Write-Ahead Log manager with persistent storage
#[derive(Debug)]
pub struct PersistentWAL {
    /// Directory where WAL files are stored
    wal_dir: PathBuf,
    /// Current WAL file writer
    current_writer: Arc<Mutex<Option<BufWriter<File>>>>,
    /// Current WAL file number
    current_file_number: Arc<Mutex<u64>>,
    /// Global sequence number counter
    global_sequence: Arc<Mutex<u64>>,
    /// Current WAL file path
    current_file_path: Arc<Mutex<Option<PathBuf>>>,
    /// Current file size
    current_file_size: Arc<Mutex<u64>>,
    /// Separate catalog WAL for faster recovery
    catalog_wal: Option<Arc<CatalogWAL>>,
}

/// Separate WAL for catalog operations
#[derive(Debug)]
pub struct CatalogWAL {
    /// Directory for catalog WAL files
    catalog_wal_dir: PathBuf,
    /// Current catalog WAL writer
    writer: Arc<Mutex<Option<BufWriter<File>>>>,
    /// Catalog WAL file number
    file_number: Arc<Mutex<u64>>,
}

impl WALEntry {
    /// Create a new WAL entry
    pub fn new(
        entry_type: WALEntryType,
        transaction_id: TransactionId,
        global_sequence: u64,
        txn_sequence: u64,
        operation_type: Option<OperationType>,
        description: String,
    ) -> Self {
        Self {
            entry_type,
            transaction_id,
            global_sequence,
            txn_sequence,
            timestamp: SystemTime::now(),
            operation_type,
            description,
        }
    }

    /// Check if this entry affects the catalog
    pub fn affects_catalog(&self) -> bool {
        match self.operation_type {
            Some(OperationType::CreateTable)
            | Some(OperationType::CreateGraph)
            | Some(OperationType::DropTable)
            | Some(OperationType::DropGraph) => true,
            _ => {
                // Check if description contains catalog-related keywords
                self.description.contains("SCHEMA")
                    || self.description.contains("INDEX")
                    || self.description.contains("CONSTRAINT")
                    || self.description.contains("VIEW")
            }
        }
    }

    /// Serialize WAL entry to binary format
    ///
    /// Binary Format:
    /// - Magic (4 bytes): WAL_MAGIC
    /// - Entry Type (1 byte): WALEntryType
    /// - Transaction ID (8 bytes): u64
    /// - Global Sequence (8 bytes): u64
    /// - Txn Sequence (8 bytes): u64
    /// - Timestamp (8 bytes): u64 (nanos since UNIX_EPOCH)
    /// - Operation Type (1 byte): Option<OperationType> (255 = None)
    /// - Description Length (4 bytes): u32
    /// - Description (variable): UTF-8 bytes
    /// - Checksum (4 bytes): CRC32
    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(256);

        // Magic number
        buffer.extend_from_slice(&WAL_MAGIC.to_le_bytes());

        // Entry type
        buffer.push(self.entry_type as u8);

        // Transaction ID
        buffer.extend_from_slice(&self.transaction_id.id().to_le_bytes());

        // Global sequence
        buffer.extend_from_slice(&self.global_sequence.to_le_bytes());

        // Transaction sequence
        buffer.extend_from_slice(&self.txn_sequence.to_le_bytes());

        // Timestamp
        let timestamp_nanos = self
            .timestamp
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        buffer.extend_from_slice(&timestamp_nanos.to_le_bytes());

        // Operation type (255 = None)
        let op_type_byte = match &self.operation_type {
            // Read operations
            Some(OperationType::Select) => 0,
            Some(OperationType::Match) => 1,
            // Write operations
            Some(OperationType::Insert) => 10,
            Some(OperationType::Update) => 11,
            Some(OperationType::Set) => 12,
            Some(OperationType::Delete) => 13,
            Some(OperationType::Remove) => 14,
            // Schema operations
            Some(OperationType::CreateTable) => 20,
            Some(OperationType::CreateGraph) => 21,
            Some(OperationType::AlterTable) => 22,
            Some(OperationType::DropTable) => 23,
            Some(OperationType::DropGraph) => 24,
            // Security operations
            Some(OperationType::CreateUser) => 25,
            Some(OperationType::DropUser) => 26,
            Some(OperationType::CreateRole) => 27,
            Some(OperationType::DropRole) => 28,
            Some(OperationType::GrantRole) => 29,
            Some(OperationType::RevokeRole) => 30,
            // Transaction control
            Some(OperationType::Begin) => 31,
            Some(OperationType::Commit) => 32,
            Some(OperationType::Rollback) => 33,
            // Other
            Some(OperationType::Other) => 99,
            None => 255,
        };
        buffer.push(op_type_byte);

        // Description
        let desc_bytes = self.description.as_bytes();
        buffer.extend_from_slice(&(desc_bytes.len() as u32).to_le_bytes());
        buffer.extend_from_slice(desc_bytes);

        // Calculate CRC32 checksum
        let checksum = crc32fast::hash(&buffer);
        buffer.extend_from_slice(&checksum.to_le_bytes());

        buffer
    }

    /// Deserialize WAL entry from binary format
    pub fn deserialize(data: &[u8]) -> Result<Self, WALError> {
        if data.len() < 50 {
            // Minimum size check
            return Err(WALError::CorruptedEntry("Entry too small".to_string()));
        }

        let mut offset = 0;

        // Check magic number
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != WAL_MAGIC {
            return Err(WALError::CorruptedEntry("Invalid magic number".to_string()));
        }
        offset += 4;

        // Entry type
        let entry_type = match data[offset] {
            1 => WALEntryType::TransactionBegin,
            2 => WALEntryType::TransactionOperation,
            3 => WALEntryType::TransactionCommit,
            4 => WALEntryType::TransactionRollback,
            _ => return Err(WALError::CorruptedEntry("Invalid entry type".to_string())),
        };
        offset += 1;

        // Transaction ID
        let txn_id_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ];
        let transaction_id = TransactionId::from_u64(u64::from_le_bytes(txn_id_bytes));
        offset += 8;

        // Global sequence
        let global_seq_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ];
        let global_sequence = u64::from_le_bytes(global_seq_bytes);
        offset += 8;

        // Transaction sequence
        let txn_seq_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ];
        let txn_sequence = u64::from_le_bytes(txn_seq_bytes);
        offset += 8;

        // Timestamp
        let timestamp_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ];
        let timestamp_nanos = u64::from_le_bytes(timestamp_bytes);
        let timestamp = UNIX_EPOCH + std::time::Duration::from_nanos(timestamp_nanos);
        offset += 8;

        // Operation type
        let operation_type = match data[offset] {
            // Read operations
            0 => Some(OperationType::Select),
            1 => Some(OperationType::Match),
            // Write operations
            10 => Some(OperationType::Insert),
            11 => Some(OperationType::Update),
            12 => Some(OperationType::Set),
            13 => Some(OperationType::Delete),
            14 => Some(OperationType::Remove),
            // Schema operations
            20 => Some(OperationType::CreateTable),
            21 => Some(OperationType::CreateGraph),
            22 => Some(OperationType::AlterTable),
            23 => Some(OperationType::DropTable),
            24 => Some(OperationType::DropGraph),
            // Security operations
            25 => Some(OperationType::CreateUser),
            26 => Some(OperationType::DropUser),
            27 => Some(OperationType::CreateRole),
            28 => Some(OperationType::DropRole),
            29 => Some(OperationType::GrantRole),
            30 => Some(OperationType::RevokeRole),
            // Transaction control
            31 => Some(OperationType::Begin),
            32 => Some(OperationType::Commit),
            33 => Some(OperationType::Rollback),
            // Other
            99 => Some(OperationType::Other),
            255 => None,
            _ => {
                return Err(WALError::CorruptedEntry(
                    "Invalid operation type".to_string(),
                ))
            }
        };
        offset += 1;

        // Description length
        if offset + 4 > data.len() {
            return Err(WALError::CorruptedEntry(
                "Truncated description length".to_string(),
            ));
        }
        let desc_len_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];
        let desc_len = u32::from_le_bytes(desc_len_bytes) as usize;
        offset += 4;

        // Description
        if offset + desc_len + 4 > data.len() {
            return Err(WALError::CorruptedEntry(
                "Truncated description or checksum".to_string(),
            ));
        }
        let description = String::from_utf8(data[offset..offset + desc_len].to_vec())
            .map_err(|_| WALError::CorruptedEntry("Invalid UTF-8 in description".to_string()))?;
        offset += desc_len;

        // Verify checksum
        let expected_checksum_bytes = [
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ];
        let expected_checksum = u32::from_le_bytes(expected_checksum_bytes);
        let actual_checksum = crc32fast::hash(&data[..offset]);

        if expected_checksum != actual_checksum {
            return Err(WALError::CorruptedEntry("Checksum mismatch".to_string()));
        }

        Ok(Self {
            entry_type,
            transaction_id,
            global_sequence,
            txn_sequence,
            timestamp,
            operation_type,
            description,
        })
    }
}

impl PersistentWAL {
    /// Create a new persistent WAL instance with a database path
    ///
    /// # Arguments
    /// * `db_path` - The database directory path. WAL files will be stored in `db_path/wal/`
    ///
    /// # Errors
    /// Returns `WALError::ConfigError` if the path is not provided
    pub fn new(db_path: PathBuf) -> Result<Self, WALError> {
        Self::new_with_path(db_path)
    }

    /// Create a new persistent WAL instance with a specific path
    ///
    /// This is the internal method that requires a path. All WAL instances must be
    /// associated with a database directory to ensure proper data organization.
    pub fn new_with_path(db_path: PathBuf) -> Result<Self, WALError> {
        let wal_dir = db_path.join("wal");

        // Create WAL directory if it doesn't exist
        create_dir_all(&wal_dir)
            .map_err(|e| WALError::IOError(format!("Failed to create WAL directory: {}", e)))?;

        // Create catalog WAL directory
        let catalog_wal_dir = wal_dir.join("catalog");
        create_dir_all(&catalog_wal_dir).map_err(|e| {
            WALError::IOError(format!("Failed to create catalog WAL directory: {}", e))
        })?;

        let catalog_wal = CatalogWAL {
            catalog_wal_dir,
            writer: Arc::new(Mutex::new(None)),
            file_number: Arc::new(Mutex::new(0)),
        };

        let mut wal = Self {
            wal_dir,
            current_writer: Arc::new(Mutex::new(None)),
            current_file_number: Arc::new(Mutex::new(0)),
            global_sequence: Arc::new(Mutex::new(0)),
            current_file_path: Arc::new(Mutex::new(None)),
            current_file_size: Arc::new(Mutex::new(0)),
            catalog_wal: Some(Arc::new(catalog_wal)),
        };

        // Initialize WAL by finding the latest file and sequence numbers
        wal.initialize()?;

        Ok(wal)
    }

    /// Initialize WAL by scanning existing files
    fn initialize(&mut self) -> Result<(), WALError> {
        let mut max_file_number = 0u64;
        let mut max_global_sequence = 0u64;

        // Scan existing WAL files
        if let Ok(entries) = std::fs::read_dir(&self.wal_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(filename) = entry.file_name().to_str() {
                        if filename.starts_with("wal_") && filename.ends_with(".log") {
                            // Extract file number from filename (wal_NNNNNN.log)
                            if let Some(number_str) = filename
                                .strip_prefix("wal_")
                                .and_then(|s| s.strip_suffix(".log"))
                            {
                                if let Ok(file_number) = number_str.parse::<u64>() {
                                    max_file_number = max_file_number.max(file_number);

                                    // Scan this file for the highest sequence number
                                    if let Ok(entries) = self.read_wal_file(file_number) {
                                        for entry in entries {
                                            max_global_sequence =
                                                max_global_sequence.max(entry.global_sequence);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Set initial values
        *self.current_file_number.lock().unwrap() = max_file_number;
        *self.global_sequence.lock().unwrap() = max_global_sequence;

        // Open current WAL file for writing
        self.rotate_wal_file()?;

        Ok(())
    }

    /// Write a WAL entry to persistent storage
    pub fn write_entry(&self, entry: WALEntry) -> Result<(), WALError> {
        let serialized = entry.serialize();

        // Check if we need to rotate to a new file
        {
            let current_size = *self.current_file_size.lock().unwrap();
            if current_size + serialized.len() as u64 > MAX_WAL_FILE_SIZE {
                self.rotate_wal_file()?;
            }
        }

        // Write to current file
        {
            let mut writer_guard = self.current_writer.lock().unwrap();
            if let Some(writer) = writer_guard.as_mut() {
                writer
                    .write_all(&serialized)
                    .map_err(|e| WALError::IOError(format!("Failed to write WAL entry: {}", e)))?;

                // Force write to disk for durability
                writer
                    .flush()
                    .map_err(|e| WALError::IOError(format!("Failed to flush WAL: {}", e)))?;

                // Ensure data is written to disk
                writer
                    .get_mut()
                    .sync_data()
                    .map_err(|e| WALError::IOError(format!("Failed to sync WAL: {}", e)))?;

                // Update file size
                *self.current_file_size.lock().unwrap() += serialized.len() as u64;
            } else {
                return Err(WALError::IOError("No active WAL file".to_string()));
            }
        }

        // Also write to catalog WAL if it affects catalog
        if entry.affects_catalog() {
            if let Some(catalog_wal) = &self.catalog_wal {
                catalog_wal.write_entry(&serialized)?;
            }
        }

        Ok(())
    }

    /// Mark a transaction as committed
    #[allow(dead_code)] // ROADMAP v0.3.0 - WAL commit marker for transaction durability
    pub fn mark_committed(&self, transaction_id: TransactionId) -> Result<(), WALError> {
        let seq = self.next_global_sequence();
        let entry = WALEntry::new(
            WALEntryType::TransactionCommit,
            transaction_id,
            seq,
            0, // Commit doesn't have a txn sequence
            Some(OperationType::Commit),
            format!("Transaction {} committed", transaction_id.id()),
        );

        self.write_entry(entry)
    }

    /// Mark a transaction as rolled back
    #[allow(dead_code)] // ROADMAP v0.3.0 - WAL entry for ROLLBACK durability (write rollback marker to WAL)
    pub fn mark_rolled_back(&self, transaction_id: TransactionId) -> Result<(), WALError> {
        let seq = self.next_global_sequence();
        let entry = WALEntry::new(
            WALEntryType::TransactionRollback,
            transaction_id,
            seq,
            0, // Rollback doesn't have a txn sequence
            Some(OperationType::Rollback),
            format!("Transaction {} rolled back", transaction_id.id()),
        );

        self.write_entry(entry)
    }

    /// Rotate to a new WAL file
    fn rotate_wal_file(&self) -> Result<(), WALError> {
        // First, properly close the old writer if it exists
        {
            let mut writer_guard = self.current_writer.lock().unwrap();
            if let Some(mut old_writer) = writer_guard.take() {
                // Flush any pending data
                old_writer
                    .flush()
                    .map_err(|e| WALError::IOError(format!("Failed to flush old WAL: {}", e)))?;
                old_writer
                    .get_mut()
                    .sync_all()
                    .map_err(|e| WALError::IOError(format!("Failed to sync old WAL: {}", e)))?;
            }
        }

        let mut file_number = self.current_file_number.lock().unwrap();
        *file_number += 1;

        let filename = format!("wal_{:06}.log", *file_number);
        let file_path = self.wal_dir.join(&filename);

        let file = OpenOptions::new()
            .create(true)
            
            .truncate(false)
            .append(true)
            .open(&file_path)
            .map_err(|e| WALError::IOError(format!("Failed to create WAL file: {}", e)))?;

        let mut writer = BufWriter::new(file);

        // Write WAL file header
        let header = self.create_file_header()?;
        writer
            .write_all(&header)
            .map_err(|e| WALError::IOError(format!("Failed to write WAL header: {}", e)))?;

        writer
            .flush()
            .map_err(|e| WALError::IOError(format!("Failed to flush WAL header: {}", e)))?;

        *self.current_writer.lock().unwrap() = Some(writer);
        *self.current_file_path.lock().unwrap() = Some(file_path);
        *self.current_file_size.lock().unwrap() = header.len() as u64;

        Ok(())
    }

    /// Create WAL file header
    fn create_file_header(&self) -> Result<Vec<u8>, WALError> {
        let mut header = Vec::with_capacity(64);

        // Magic number
        header.extend_from_slice(&WAL_MAGIC.to_le_bytes());

        // Version
        header.extend_from_slice(&WAL_VERSION.to_le_bytes());

        // Creation timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        header.extend_from_slice(&timestamp.to_le_bytes());

        // Reserved space for future use (50 bytes to make total 64)
        header.extend_from_slice(&[0u8; 50]);

        Ok(header)
    }

    /// Get next global sequence number
    pub fn next_global_sequence(&self) -> u64 {
        let mut seq = self.global_sequence.lock().unwrap();
        *seq += 1;
        *seq
    }

    /// Get current WAL file number (for testing)
    #[allow(dead_code)] // ROADMAP v0.6.0 - WAL file number accessor for monitoring and debugging
    pub fn current_file_number(&self) -> u64 {
        *self.current_file_number.lock().unwrap()
    }

    /// Ensure all pending writes are flushed (for testing)
    #[allow(dead_code)] // ROADMAP v0.3.0 - Manual WAL flush for durability checkpoints and testing
    pub fn flush(&self) -> Result<(), WALError> {
        let mut writer_guard = self.current_writer.lock().unwrap();
        if let Some(writer) = writer_guard.as_mut() {
            writer
                .flush()
                .map_err(|e| WALError::IOError(format!("Failed to flush WAL: {}", e)))?;
            writer
                .get_mut()
                .sync_all()
                .map_err(|e| WALError::IOError(format!("Failed to sync WAL: {}", e)))?;
        }
        Ok(())
    }

    /// Read all entries from a specific WAL file
    pub fn read_wal_file(&self, file_number: u64) -> Result<Vec<WALEntry>, WALError> {
        let filename = format!("wal_{:06}.log", file_number);
        let file_path = self.wal_dir.join(&filename);

        if !file_path.exists() {
            return Err(WALError::IOError(format!(
                "WAL file not found: {}",
                file_path.display()
            )));
        }

        let mut file = File::open(&file_path)
            .map_err(|e| WALError::IOError(format!("Failed to open WAL file: {}", e)))?;

        // Skip file header (64 bytes)
        file.seek(SeekFrom::Start(64))
            .map_err(|e| WALError::IOError(format!("Failed to seek in WAL file: {}", e)))?;

        let mut reader = BufReader::new(file);
        let mut entries = Vec::new();
        let mut buffer = Vec::new();

        // Read entire file
        reader
            .read_to_end(&mut buffer)
            .map_err(|e| WALError::IOError(format!("Failed to read WAL file: {}", e)))?;

        let mut offset = 0;
        while offset < buffer.len() {
            // Try to find next entry by looking for magic number
            if offset + 4 <= buffer.len() {
                let magic = u32::from_le_bytes([
                    buffer[offset],
                    buffer[offset + 1],
                    buffer[offset + 2],
                    buffer[offset + 3],
                ]);

                if magic == WAL_MAGIC {
                    // Read the minimum size to determine actual entry size
                    if offset + 50 <= buffer.len() {
                        // Try to deserialize starting from this offset
                        // The deserializer should handle determining the actual size
                        match WALEntry::deserialize(&buffer[offset..]) {
                            Ok(entry) => {
                                // Calculate the actual size of this entry
                                let entry_bytes = entry.serialize();
                                let entry_size = entry_bytes.len();

                                entries.push(entry);
                                // Move offset past this entry
                                offset += entry_size;
                                continue;
                            }
                            Err(_) => {
                                // Skip this corrupted entry
                                offset += 1;
                            }
                        }
                    } else {
                        // Not enough data for a complete entry
                        break;
                    }
                } else {
                    offset += 1;
                }
            } else {
                break;
            }
        }

        Ok(entries)
    }
}

/// WAL-specific errors
#[derive(Debug)]
#[allow(dead_code)] // ROADMAP v0.3.0 - WAL error handling for durability layer
pub enum WALError {
    IOError(String),
    CorruptedEntry(String),
    ConfigError(String),
}

impl std::fmt::Display for WALError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WALError::IOError(msg) => write!(f, "WAL IO Error: {}", msg),
            WALError::CorruptedEntry(msg) => write!(f, "WAL Corrupted Entry: {}", msg),
            WALError::ConfigError(msg) => write!(f, "WAL Config Error: {}", msg),
        }
    }
}

impl std::error::Error for WALError {}

impl CatalogWAL {
    /// Write an entry to the catalog WAL
    pub fn write_entry(&self, serialized: &[u8]) -> Result<(), WALError> {
        // Check if writer needs initialization first
        {
            let writer_guard = self.writer.lock().unwrap();
            if writer_guard.is_none() {
                drop(writer_guard); // Release lock before calling rotate
                self.rotate_catalog_file()?;
            }
        }

        // Now write the entry
        let mut writer_guard = self.writer.lock().unwrap();
        if let Some(writer) = writer_guard.as_mut() {
            writer.write_all(serialized).map_err(|e| {
                WALError::IOError(format!("Failed to write catalog WAL entry: {}", e))
            })?;

            writer
                .flush()
                .map_err(|e| WALError::IOError(format!("Failed to flush catalog WAL: {}", e)))?;

            writer
                .get_mut()
                .sync_all()
                .map_err(|e| WALError::IOError(format!("Failed to sync catalog WAL: {}", e)))?;
        }

        Ok(())
    }

    /// Rotate to a new catalog WAL file
    fn rotate_catalog_file(&self) -> Result<(), WALError> {
        let mut file_number = self.file_number.lock().unwrap();
        *file_number += 1;

        let filename = format!("catalog_{:06}.log", *file_number);
        let file_path = self.catalog_wal_dir.join(&filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .map_err(|e| WALError::IOError(format!("Failed to create catalog WAL file: {}", e)))?;

        let writer = BufWriter::new(file);
        *self.writer.lock().unwrap() = Some(writer);

        Ok(())
    }
}
