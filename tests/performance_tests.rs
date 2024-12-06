use std::process::Command;
use std::time::Duration;
use std::thread;

#[derive(Debug)]
struct BenchmarkResult {
    requests_per_sec: f64,
    transfer_per_sec: String,
    avg_latency: String,
    stdev_within_pct: f64,
}

fn parse_number(s: &str) -> f64 {
    s.trim()
        .replace(',', "")
        .parse()
        .unwrap_or_default()
}

fn run_wrk_benchmark(threads: u32, connections: u32, duration_secs: u32, endpoint: &str) -> BenchmarkResult {
    // Allow some time for the server to be ready
    thread::sleep(Duration::from_secs(1));

    let output = Command::new("wrk")
        .args([
            &format!("-t{}", threads),
            &format!("-c{}", connections),
            &format!("-d{}s", duration_secs),
            endpoint,
        ])
        .output()
        .expect("Failed to execute wrk benchmark");

    let output_str = String::from_utf8_lossy(&output.stdout);
    
    // Parse the wrk output
    let lines: Vec<&str> = output_str.lines().collect();
    
    let requests_line = lines.iter()
        .find(|l| l.contains("Requests/sec:"))
        .map(|l| l.split(':').nth(1).unwrap_or("0"))
        .unwrap_or("0");

    let transfer_line = lines.iter()
        .find(|l| l.contains("Transfer/sec:"))
        .map(|l| l.split(':').nth(1).unwrap_or("0"))
        .unwrap_or("0");

    let latency_line = lines.iter()
        .find(|l| l.contains("Latency"))
        .unwrap_or(&"Latency 0ms");

    let stdev_str = lines.iter()
        .find(|l| l.contains("+/- Stdev"))
        .and_then(|l| l.split('+').nth(1))
        .and_then(|l| l.split('%').next())
        .unwrap_or("0")
        .trim()
        .trim_start_matches("/- Stdev");

    BenchmarkResult {
        requests_per_sec: parse_number(requests_line),
        transfer_per_sec: transfer_line.trim().to_string(),
        avg_latency: latency_line.split_whitespace().nth(1).unwrap_or("0ms").to_string(),
        stdev_within_pct: parse_number(stdev_str),
    }
}

#[test]
fn test_image_serving_performance() {
    let test_image = "eva-p.jpg"; // You might want to make this configurable
    let endpoint = format!("http://localhost:8081/images/{}", test_image);

    println!("\nRunning performance tests for image serving...");

    // Test 1: Baseline (moderate concurrency)
    println!("\n1. Baseline Test (4 threads, 50 connections):");
    let baseline = run_wrk_benchmark(4, 50, 30, &endpoint);
    println!("   Requests/sec: {:.2}", baseline.requests_per_sec);
    println!("   Transfer/sec: {}", baseline.transfer_per_sec);
    println!("   Avg Latency: {}", baseline.avg_latency);
    println!("   Within Stdev: {:.2}%", baseline.stdev_within_pct);
    
    assert!(baseline.requests_per_sec > 15000.0, "Baseline performance below threshold");

    // Test 2: High Concurrency
    println!("\n2. High Concurrency Test (8 threads, 100 connections):");
    let high_concurrency = run_wrk_benchmark(8, 100, 30, &endpoint);
    println!("   Requests/sec: {:.2}", high_concurrency.requests_per_sec);
    println!("   Transfer/sec: {}", high_concurrency.transfer_per_sec);
    println!("   Avg Latency: {}", high_concurrency.avg_latency);
    println!("   Within Stdev: {:.2}%", high_concurrency.stdev_within_pct);
    
    assert!(high_concurrency.requests_per_sec > 15000.0, "High concurrency performance below threshold");

    // Test 3: Low Concurrency
    println!("\n3. Low Concurrency Test (2 threads, 10 connections):");
    let low_concurrency = run_wrk_benchmark(2, 10, 30, &endpoint);
    println!("   Requests/sec: {:.2}", low_concurrency.requests_per_sec);
    println!("   Transfer/sec: {}", low_concurrency.transfer_per_sec);
    println!("   Avg Latency: {}", low_concurrency.avg_latency);
    println!("   Within Stdev: {:.2}%", low_concurrency.stdev_within_pct);
    
    assert!(low_concurrency.requests_per_sec > 10000.0, "Low concurrency performance below threshold");

    // Additional assertions to ensure performance stability
    assert!(
        (high_concurrency.requests_per_sec / baseline.requests_per_sec) > 0.8,
        "Significant performance degradation under high concurrency"
    );
}

// Helper function to check if wrk is installed
fn check_wrk_installed() -> bool {
    Command::new("which")
        .arg("wrk")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

// Run this before performance tests
#[test]
fn test_prerequisites() {
    assert!(
        check_wrk_installed(),
        "wrk is not installed. Please install it using: brew install wrk"
    );
}
