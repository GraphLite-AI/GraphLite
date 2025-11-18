// Copyright (c) 2024-2025 DeepGraph Inc.
// SPDX-License-Identifier: Apache-2.0
//
//! Catalog providers registration system
//!
//! This module contains the registration system for all catalog providers.
//! Adding a new catalog requires only implementing the CatalogProvider trait
//! and adding one line to the register_all_catalogs function.

use super::registry::CatalogRegistry;

// Individual catalog provider modules
pub mod index;
pub mod graph_metadata; // Graph definitions (not metadata tracking) - needed for CREATE GRAPH
pub mod security;
pub mod schema;

// Re-export GraphTypeCatalog from schema module
pub use crate::schema::catalog::graph_type::GraphTypeCatalog;

// These will be implemented as the catalog providers are created
// pub mod timeseries;
// pub mod document;
// pub mod spatial;

/// Register all available catalogs
///
/// This function is called during catalog registry initialization to register
/// all available catalog providers. Adding a new catalog is as simple as
/// adding one line to this function.
///
/// # Arguments
/// * `registry` - Mutable reference to the catalog registry
///
/// # Example
/// To add a new catalog, simply add a line like:
/// ```ignore
/// registry.register("mycatalog", mycatalog::MyCatalog::new());
/// ```
pub fn register_all_catalogs(registry: &mut CatalogRegistry) {
    // Register implemented catalog providers
    registry.register("index", index::IndexCatalog::new());
    registry.register("graph_metadata", graph_metadata::GraphMetadataCatalog::new()); // Graph definitions (needed for CREATE GRAPH)
    registry.register("security", security::SecurityCatalog::new());
    registry.register("schema", schema::SchemaCatalog::new());
    registry.register("graph_type", Box::new(GraphTypeCatalog::new()));

    // TODO: Register additional catalog providers as they are implemented:
    // registry.register("timeseries", timeseries::TimeSeriesCatalog::new());
    // registry.register("document", document::DocumentCatalog::new());
    // registry.register("spatial", spatial::SpatialCatalog::new());
    
    log::info!("Catalog provider registration complete");
}
