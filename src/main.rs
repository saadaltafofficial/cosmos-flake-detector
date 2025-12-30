use clap::Parser;
use colored::Colorize;
use hdrhistogram::Histogram;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[command(name = "flake-detector")]
#[command(about = "Detect flaky Cosmos RPC endpoints with query-specific testing", long_about = None)]
struct Args {
    #[arg(short, long, value_delimiter = ',', help = "Comma-separated list of RPC endpoints to test")]
    endpoints: Vec<String>,

    #[arg(short, long, default_value = "60", help = "Test duration in seconds")]
    duration: u64,

    #[arg(short, long, value_delimiter = ',', 
          default_value = "health,status,abci_info,net_info,genesis",
          help = "Comma-separated list of RPC queries to test")]
    queries: Vec<String>,

    #[arg(short, long, help = "Output JSON file path (optional)")]
    output: Option<String>,

    #[arg(short = 'c', long, default_value = "10", help = "Concurrent requests per endpoint")]
    concurrency: usize,

    #[arg(short = 't', long, default_value = "5", help = "Request timeout in seconds")]
    timeout: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct QueryResult {
    query: String,
    success_count: u64,
    failure_count: u64,
    total_requests: u64,
    failure_rate: f64,
    p50_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    avg_latency_ms: f64,
    min_latency_ms: f64,
    max_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EndpointReport {
    endpoint: String,
    overall_success_rate: f64,
    overall_failure_rate: f64,
    flakiness_score: f64,
    total_requests: u64,
    test_duration_secs: u64,
    queries: Vec<QueryResult>,
}

#[derive(Debug)]
struct TestMetrics {
    success_count: u64,
    failure_count: u64,
    latencies: Histogram<u64>,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            success_count: 0,
            failure_count: 0,
            latencies: Histogram::<u64>::new(3).unwrap(),
        }
    }
}

async fn test_endpoint_query(
    client: &Client,
    endpoint: &str,
    query: &str,
) -> Result<Duration, String> {
    let url = format!("{}/{}", endpoint.trim_end_matches('/'), query);
    let start = Instant::now();
    
    match client.get(&url).send().await {
        Ok(response) => {
            let elapsed = start.elapsed();
            if response.status().is_success() {
                Ok(elapsed)
            } else {
                Err(format!("HTTP {}", response.status()))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

async fn run_continuous_tests(
    client: Client,
    endpoint: String,
    query: String,
    duration: Duration,
    metrics: Arc<Mutex<TestMetrics>>,
) {
    let end_time = Instant::now() + duration;
    
    while Instant::now() < end_time {
        match test_endpoint_query(&client, &endpoint, &query).await {
            Ok(latency) => {
                let mut m = metrics.lock().await;
                m.success_count += 1;
                let _ = m.latencies.record(latency.as_micros() as u64);
            }
            Err(_) => {
                let mut m = metrics.lock().await;
                m.failure_count += 1;
            }
        }
        
        sleep(Duration::from_millis(100)).await;
    }
}

fn calculate_flakiness_score(failure_rate: f64, p99_latency_ms: f64) -> f64 {
    let latency_threshold = 1000.0;
    let latency_severity = (p99_latency_ms / latency_threshold).min(1.0);
    
    let score = (failure_rate * 0.7) + (latency_severity * 0.3);
    (score * 100.0).min(100.0)
}

fn get_status_emoji(score: f64) -> &'static str {
    if score < 10.0 {
        "🟢"
    } else if score < 30.0 {
        "🟡"
    } else if score < 60.0 {
        "🟠"
    } else {
        "🔴"
    }
}

async fn test_endpoint(
    endpoint: &str,
    queries: &[String],
    duration: u64,
    concurrency: usize,
    timeout: u64,
) -> EndpointReport {
    println!("\n{} Testing endpoint: {}", "🔍".bright_blue(), endpoint.bright_cyan());
    
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()
        .unwrap();
    
    let test_duration = Duration::from_secs(duration);
    let mut query_results = Vec::new();
    let mut total_success = 0u64;
    let mut total_failure = 0u64;
    
    for query in queries {
        println!("  {} Testing query: {}", "→".bright_white(), query.bright_white());
        
        let metrics = Arc::new(Mutex::new(TestMetrics::new()));
        let mut tasks = vec![];
        
        for _ in 0..concurrency {
            let client_clone = client.clone();
            let endpoint_clone = endpoint.to_string();
            let query_clone = query.to_string();
            let metrics_clone = metrics.clone();
            
            tasks.push(tokio::spawn(run_continuous_tests(
                client_clone,
                endpoint_clone,
                query_clone,
                test_duration,
                metrics_clone,
            )));
        }
        
        for task in tasks {
            let _ = task.await;
        }
        
        let final_metrics = metrics.lock().await;
        let success = final_metrics.success_count;
        let failure = final_metrics.failure_count;
        let total = success + failure;
        
        total_success += success;
        total_failure += failure;
        
        let failure_rate = if total > 0 {
            failure as f64 / total as f64
        } else {
            0.0
        };
        
        let (p50, p95, p99, avg, min, max) = if !final_metrics.latencies.is_empty() {
            (
                final_metrics.latencies.value_at_quantile(0.5) as f64 / 1000.0,
                final_metrics.latencies.value_at_quantile(0.95) as f64 / 1000.0,
                final_metrics.latencies.value_at_quantile(0.99) as f64 / 1000.0,
                final_metrics.latencies.mean() / 1000.0,
                final_metrics.latencies.min() as f64 / 1000.0,
                final_metrics.latencies.max() as f64 / 1000.0,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
        };
        
        println!("    ✓ Success: {} | ✗ Failure: {} | Rate: {:.1}%", 
                 success.to_string().bright_green(),
                 failure.to_string().bright_red(),
                 (failure_rate * 100.0).to_string().bright_yellow());
        println!("    Latency: p50={:.1}ms p95={:.1}ms p99={:.1}ms", p50, p95, p99);
        
        query_results.push(QueryResult {
            query: query.clone(),
            success_count: success,
            failure_count: failure,
            total_requests: total,
            failure_rate,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            avg_latency_ms: avg,
            min_latency_ms: min,
            max_latency_ms: max,
        });
    }
    
    let total_requests = total_success + total_failure;
    let overall_failure_rate = if total_requests > 0 {
        total_failure as f64 / total_requests as f64
    } else {
        0.0
    };
    
    let avg_p99 = query_results.iter()
        .map(|r| r.p99_latency_ms)
        .sum::<f64>() / query_results.len() as f64;
    
    let flakiness_score = calculate_flakiness_score(overall_failure_rate, avg_p99);
    
    EndpointReport {
        endpoint: endpoint.to_string(),
        overall_success_rate: 1.0 - overall_failure_rate,
        overall_failure_rate,
        flakiness_score,
        total_requests,
        test_duration_secs: duration,
        queries: query_results,
    }
}

fn print_summary(reports: &[EndpointReport]) {
    println!("\n{}", "═══════════════════════════════════════════════════".bright_blue());
    println!("{}", "           FLAKINESS DETECTION SUMMARY".bright_white().bold());
    println!("{}", "═══════════════════════════════════════════════════".bright_blue());
    
    for report in reports {
        let emoji = get_status_emoji(report.flakiness_score);
        let score_color = if report.flakiness_score < 10.0 {
            report.flakiness_score.to_string().bright_green()
        } else if report.flakiness_score < 30.0 {
            report.flakiness_score.to_string().bright_yellow()
        } else if report.flakiness_score < 60.0 {
            report.flakiness_score.to_string().bright_magenta()
        } else {
            report.flakiness_score.to_string().bright_red()
        };
        
        println!("\n{} {} - Flakiness Score: {}/100",
                 emoji,
                 report.endpoint.bright_cyan(),
                 score_color.bold());
        println!("  Success Rate: {:.1}% | Total Requests: {}",
                 (report.overall_success_rate * 100.0).to_string().bright_green(),
                 report.total_requests);
    }
    
    println!("\n{}", "═══════════════════════════════════════════════════".bright_blue());
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    println!("{}", "╔══════════════════════════════════════════════════╗".bright_blue());
    println!("{}", "║     COSMOS RPC FLAKE DETECTOR v0.1.0             ║".bright_white().bold());
    println!("{}", "╚══════════════════════════════════════════════════╝".bright_blue());
    
    println!("\n{} Configuration:", "⚙".bright_yellow());
    println!("  Endpoints: {}", args.endpoints.len());
    println!("  Test Duration: {}s", args.duration);
    println!("  Queries: {}", args.queries.join(", "));
    println!("  Concurrency: {}", args.concurrency);
    
    let mut reports = Vec::new();
    
    for endpoint in &args.endpoints {
        let report = test_endpoint(
            endpoint,
            &args.queries,
            args.duration,
            args.concurrency,
            args.timeout,
        ).await;
        reports.push(report);
    }
    
    print_summary(&reports);
    
    if let Some(output_path) = args.output {
        match std::fs::write(&output_path, serde_json::to_string_pretty(&reports).unwrap()) {
            Ok(_) => println!("\n{} Results exported to: {}", 
                            "💾".bright_green(), 
                            output_path.bright_cyan()),
            Err(e) => eprintln!("\n{} Failed to write output: {}", 
                               "❌".bright_red(), e),
        }
    }
    
    println!("\n{} Testing complete!\n", "✅".bright_green());
}