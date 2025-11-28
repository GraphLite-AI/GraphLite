// Benchmark for GraphLite startup time

use graphlite::QueryCoordinator;
use std::time::Instant;
use tempfile::TempDir;

fn main() {
    println!("=== GraphLite Startup Time Benchmark ===\n");

    // Test 1: First startup (cold start)
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path();

    println!("Test 1: Cold Start (new database)");
    let start = Instant::now();
    let coordinator = QueryCoordinator::from_path(db_path).unwrap();
    let cold_start = start.elapsed();
    println!("  Time: {:?} ({:.2} ms)\n", cold_start, cold_start.as_secs_f64() * 1000.0);

    drop(coordinator);

    // Test 2: Warm start (existing database)
    println!("Test 2: Warm Start (existing database)");
    let start = Instant::now();
    let coordinator = QueryCoordinator::from_path(db_path).unwrap();
    let warm_start = start.elapsed();
    println!("  Time: {:?} ({:.2} ms)\n", warm_start, warm_start.as_secs_f64() * 1000.0);

    drop(coordinator);

    // Test 3: Multiple sequential startups
    println!("Test 3: 10 Sequential Startups (average)");
    let mut total = std::time::Duration::ZERO;
    for i in 0..10 {
        let start = Instant::now();
        let coordinator = QueryCoordinator::from_path(db_path).unwrap();
        let elapsed = start.elapsed();
        total += elapsed;
        println!("  Run {}: {:?} ({:.2} ms)", i + 1, elapsed, elapsed.as_secs_f64() * 1000.0);
        drop(coordinator);
    }
    let average = total / 10;
    println!("  Average: {:?} ({:.2} ms)\n", average, average.as_secs_f64() * 1000.0);

    // Test 4: Component-by-component timing
    println!("Test 4: Component Initialization Breakdown");
    measure_component_timing(db_path);

    println!("\n=== Summary ===");
    println!("Cold start: {:.2} ms", cold_start.as_secs_f64() * 1000.0);
    println!("Warm start: {:.2} ms", warm_start.as_secs_f64() * 1000.0);
    println!("Average (10 runs): {:.2} ms", average.as_secs_f64() * 1000.0);
}

fn measure_component_timing(db_path: &std::path::Path) {
    use graphlite::*;
    use std::sync::{Arc, RwLock};

    let total_start = Instant::now();

    // Storage
    let start = Instant::now();
    let storage = Arc::new(
        storage::StorageManager::new(
            db_path,
            storage::StorageMethod::DiskOnly,
            storage::StorageType::Sled,
        )
        .unwrap(),
    );
    println!("  Storage Manager: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    // Catalog
    let start = Instant::now();
    let catalog_manager = Arc::new(RwLock::new(catalog::manager::CatalogManager::new(storage.clone())));
    println!("  Catalog Manager: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    // Transaction Manager
    let start = Instant::now();
    let txn_manager = Arc::new(txn::TransactionManager::new(db_path).unwrap());
    println!("  Transaction Manager: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    // Cache Manager
    let start = Instant::now();
    let cache_config = cache::CacheConfig::default();
    let cache_manager = Some(Arc::new(cache::CacheManager::new(cache_config).unwrap()));
    println!("  Cache Manager: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    // Session Provider
    let start = Instant::now();
    let session_provider: Arc<dyn session::SessionProvider> = Arc::new(
        session::InstanceSessionProvider::new(
            txn_manager.clone(),
            storage.clone(),
            catalog_manager.clone(),
        )
    );
    println!("  Session Provider: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    // Query Executor
    let start = Instant::now();
    let _executor = Arc::new(
        exec::QueryExecutor::new(
            storage.clone(),
            catalog_manager.clone(),
            txn_manager.clone(),
            session_provider,
            cache_manager,
        )
        .unwrap(),
    );
    println!("  Query Executor: {:?} ({:.2} ms)", start.elapsed(), start.elapsed().as_secs_f64() * 1000.0);

    println!("  Total: {:?} ({:.2} ms)", total_start.elapsed(), total_start.elapsed().as_secs_f64() * 1000.0);
}
