// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Index Catalog Provider - Stub implementation

use crate::catalog::error::{CatalogError, CatalogResult};
use crate::catalog::operations::{CatalogOperation, CatalogResponse};
use crate::catalog::traits::{CatalogProvider, CatalogSchema};
use crate::storage::StorageManager;
use std::sync::Arc;

pub struct IndexCatalog {}

impl IndexCatalog {
    pub fn new() -> Box<Self> {
        Box::new(Self {})
    }
}

impl CatalogProvider for IndexCatalog {
    fn init(&mut self, _storage: Arc<StorageManager>) -> CatalogResult<()> {
        Ok(())
    }

    fn execute(&mut self, _op: CatalogOperation) -> CatalogResult<CatalogResponse> {
        Err(CatalogError::NotSupported(
            "Index catalog not yet implemented".to_string(),
        ))
    }

    fn save(&self) -> CatalogResult<Vec<u8>> {
        Ok(Vec::new())
    }

    fn load(&mut self, _data: &[u8]) -> CatalogResult<()> {
        Ok(())
    }

    fn schema(&self) -> CatalogSchema {
        CatalogSchema {
            name: "index".to_string(),
            version: "0.1.0".to_string(),
            entities: vec![],
            operations: vec![],
        }
    }

    fn supported_operations(&self) -> Vec<String> {
        vec![]
    }
}
