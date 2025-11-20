//! Test utilities for GraphLite integration tests
//!
//! Two fixture types available:
//! - TestFixture: Direct component access (legacy)
//! - CliFixture: CLI-based testing (recommended for new tests)
//!
//! Both provide schema isolation for test independence.

pub mod cli_fixture;
pub mod sample_data_generator;
pub mod test_fixture;
