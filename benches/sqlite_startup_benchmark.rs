// SQLite startup time benchmark for comparison

use rusqlite::Connection;
use std::time::Instant;
use tempfile::NamedTempFile;

fn main() {
    println!("=== SQLite Startup Time Benchmark ===\n");

    // Test 1: Cold start
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path();

    println!("Test 1: Cold Start (new database)");
    let start = Instant::now();
    let conn = Connection::open(db_path).unwrap();
    let cold_start = start.elapsed();
    println!("  Time: {:?} ({:.2} ms)\n", cold_start, cold_start.as_secs_f64() * 1000.0);
    drop(conn);

    // Test 2: Warm start
    println!("Test 2: Warm Start (existing database)");
    let start = Instant::now();
    let conn = Connection::open(db_path).unwrap();
    let warm_start = start.elapsed();
    println!("  Time: {:?} ({:.2} ms)\n", warm_start, warm_start.as_secs_f64() * 1000.0);
    drop(conn);

    // Test 3: Multiple sequential startups
    println!("Test 3: 10 Sequential Startups (average)");
    let mut total = std::time::Duration::ZERO;
    for i in 0..10 {
        let start = Instant::now();
        let conn = Connection::open(db_path).unwrap();
        let elapsed = start.elapsed();
        total += elapsed;
        println!("  Run {}: {:?} ({:.2} ms)", i + 1, elapsed, elapsed.as_secs_f64() * 1000.0);
        drop(conn);
    }
    let average = total / 10;
    println!("  Average: {:?} ({:.2} ms)\n", average, average.as_secs_f64() * 1000.0);

    // Test 4: With schema creation
    println!("Test 4: With Schema Creation");
    let temp_file2 = NamedTempFile::new().unwrap();
    let start = Instant::now();
    let conn = Connection::open(temp_file2.path()).unwrap();
    conn.execute_batch(
        "CREATE TABLE nodes (id INTEGER PRIMARY KEY, data TEXT);
         CREATE TABLE edges (id INTEGER PRIMARY KEY, from_id INTEGER, to_id INTEGER);
         CREATE INDEX idx_from ON edges(from_id);
         CREATE INDEX idx_to ON edges(to_id);"
    ).unwrap();
    let with_schema = start.elapsed();
    println!("  Time: {:?} ({:.2} ms)\n", with_schema, with_schema.as_secs_f64() * 1000.0);

    // Test 5: In-memory database
    println!("Test 5: In-Memory Database (10 runs average)");
    let mut total = std::time::Duration::ZERO;
    for i in 0..10 {
        let start = Instant::now();
        let conn = Connection::open_in_memory().unwrap();
        let elapsed = start.elapsed();
        total += elapsed;
        println!("  Run {}: {:?} ({:.2} µs)", i + 1, elapsed, elapsed.as_micros());
        drop(conn);
    }
    let average_memory = total / 10;
    println!("  Average: {:?} ({:.2} µs)\n", average_memory, average_memory.as_micros());

    println!("\n=== Summary ===");
    println!("Cold start (file): {:.2} ms", cold_start.as_secs_f64() * 1000.0);
    println!("Warm start (file): {:.2} ms", warm_start.as_secs_f64() * 1000.0);
    println!("Average (file, 10 runs): {:.2} ms", average.as_secs_f64() * 1000.0);
    println!("With schema creation: {:.2} ms", with_schema.as_secs_f64() * 1000.0);
    println!("In-memory: {:.2} µs", average_memory.as_micros());
}
