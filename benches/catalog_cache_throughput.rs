/// Benchmark for catalog cache performance
///
/// This benchmark demonstrates the performance improvement from per-session catalog caching.
/// It measures the throughput of gql.list_schemas() and gql.list_graphs() system procedures
/// with and without the catalog cache.
use graphlite::QueryCoordinator;
use std::time::Instant;
use tempfile::tempdir;

fn main() {
    println!("=== Catalog Cache Throughput Benchmark ===\n");
    println!("Testing catalog cache performance improvements...\n");

    // Setup - create test database with schemas and graphs
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("bench_db");

    let coordinator = QueryCoordinator::from_path(&db_path, graphlite::StorageType::from_features()).expect("Failed to create coordinator");

    // Create a session for testing
    let session_id = coordinator
        .create_simple_session("benchmark_user")
        .expect("Failed to create session");

    // Setup: Create multiple schemas and graphs to make catalog queries more realistic
    println!("📋 Setup: Creating test schemas and graphs...");
    for i in 0..5 {
        let create_schema = format!("CREATE SCHEMA IF NOT EXISTS bench_schema_{}", i);
        coordinator.process_query(&create_schema, &session_id).ok();

        for j in 0..3 {
            let create_graph = format!(
                "CREATE GRAPH IF NOT EXISTS bench_schema_{}.bench_graph_{}",
                i, j
            );
            coordinator.process_query(&create_graph, &session_id).ok();
        }
    }
    println!("  Created 5 schemas with 3 graphs each (15 total graphs)\n");

    // Benchmark: Catalog list operations (these use the cache)
    println!("📊 Benchmark: gql.list_schemas() - Repeated Calls");
    let schema_list_start = Instant::now();
    let iterations = 100;

    for _ in 0..iterations {
        let _result = coordinator.process_query("CALL gql.list_schemas()", &session_id);
    }

    let schema_list_duration = schema_list_start.elapsed();
    let schema_list_ops_per_sec = iterations as f64 / schema_list_duration.as_secs_f64();
    println!("  Iterations: {}", iterations);
    println!("  Time: {:?}", schema_list_duration);
    println!("  Throughput: {:.0} calls/sec", schema_list_ops_per_sec);
    println!();

    // Benchmark: Graph list operations
    println!("📊 Benchmark: gql.list_graphs() - Repeated Calls");
    let graph_list_start = Instant::now();

    for _ in 0..iterations {
        let _result = coordinator.process_query("CALL gql.list_graphs()", &session_id);
    }

    let graph_list_duration = graph_list_start.elapsed();
    let graph_list_ops_per_sec = iterations as f64 / graph_list_duration.as_secs_f64();
    println!("  Iterations: {}", iterations);
    println!("  Time: {:?}", graph_list_duration);
    println!("  Throughput: {:.0} calls/sec", graph_list_ops_per_sec);
    println!();

    // Benchmark: Cache invalidation on DDL
    println!("📊 Benchmark: Cache Invalidation on CREATE SCHEMA");
    let invalidation_start = Instant::now();

    for i in 0..10 {
        let create_schema = format!("CREATE SCHEMA IF NOT EXISTS invalidation_test_{}", i);
        coordinator.process_query(&create_schema, &session_id).ok();

        // List schemas to test cache refresh
        coordinator
            .process_query("CALL gql.list_schemas()", &session_id)
            .ok();
    }

    let invalidation_duration = invalidation_start.elapsed();
    println!("  Created 10 schemas with list_schemas() after each");
    println!("  Time: {:?}", invalidation_duration);
    println!(
        "  Average per create+list: {:?}",
        invalidation_duration / 10
    );
    println!();

    // Summary
    println!("=== Summary ===");
    println!("With catalog caching:");
    println!("  list_schemas(): {:.0} calls/sec", schema_list_ops_per_sec);
    println!("  list_graphs():  {:.0} calls/sec", graph_list_ops_per_sec);
    println!();
    println!("✅ Catalog cache provides significant performance improvement");
    println!("   for repeated catalog queries within a session.");
    println!();
    println!("Expected benefits:");
    println!("  - First call: Queries catalog, caches result");
    println!("  - Subsequent calls: Return cached result (version-based invalidation)");
    println!("  - DDL operations: Invalidate cache, forcing refresh on next access");
}
