// Read Engine - Query Execution
//
// This module handles all read operations (queries) in GraphLite.
// It contains the physical plan executor and related query processing components.

// pub mod physical_executor;  // TODO: Extract from executor.rs
pub mod operators;
pub mod processors;

// pub use physical_executor::PhysicalExecutor;
