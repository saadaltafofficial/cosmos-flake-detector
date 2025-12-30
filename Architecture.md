# 🏗️ Architecture Documentation

Technical design and implementation details for cosmos-flake-detector.

## System Overview

```
┌─────────────────┐
│   CLI Input     │ --endpoints, --queries, --duration
└────────┬────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Argument Parser (Clap)            │
│   - Validates inputs                 │
│   - Sets defaults                    │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Tokio Async Runtime                │
│   - Multi-threaded executor          │
│   - Spawns concurrent test tasks     │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Per-Endpoint Test Coordinator      │
│   - Spawns N concurrent workers      │
│   - Shares metrics via Arc<Mutex>    │
└────────┬────────────────────────────┘
         │
         ▼
    ┌────┴────┐
    │         │
┌───▼───┐ ┌──▼────┐  ... (N workers)
│Worker1│ │Worker2│
└───┬───┘ └──┬────┘
    │        │
    ▼        ▼
┌──────────────────────────┐
│   HTTP Client (Reqwest)   │
│   - Connection pooling    │
│   - Automatic retries     │
│   - Timeout handling      │
└────────┬─────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   RPC Endpoint (External)            │
│   - Cosmos chain RPC node            │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Response Processing                │
│   - Success/failure classification   │
│   - Latency measurement              │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   HDR Histogram (Thread-Safe)        │
│   - Records latency in microseconds  │
│   - Calculates p50/p95/p99           │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Metrics Aggregation                │
│   - Success/failure rates            │
│   - Latency percentiles              │
└────────┬────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────┐
│   Flakiness Score Calculation        │
│   Score = (fail_rate × 0.7) +        │
│           (latency_norm × 0.3) × 100 │
└────────┬────────────────────────────┘
         │
         ▼
    ┌───┴─────┐
    │         │
┌───▼────┐ ┌─▼──────┐
│Terminal│ │  JSON  │
│ Output │ │ Export │
└────────┘ └────────┘
```

## Core Components

### 1. Async Runtime (Tokio)

**Purpose**: Handle concurrent requests efficiently without blocking

**Implementation**:
```rust
#[tokio::main]
async fn main() {
    // Multi-threaded runtime spawns thread pool
}
```

**Why Tokio?**
- Mature async runtime with excellent performance
- Built-in utilities for concurrency (spawn, channels, mutexes)
- Non-blocking I/O for HTTP requests
- Can handle 1000+ concurrent connections on modest hardware

**Resource Usage**:
- Thread pool: Number of CPU cores
- Memory per task: ~2KB
- Network connections: Pooled and reused

### 2. HTTP Client (Reqwest)

**Purpose**: Make HTTP requests to RPC endpoints

**Key Features**:
```rust
let client = Client::builder()
    .timeout(Duration::from_secs(5))  // Configurable timeout
    .build()
    .unwrap();
```

**Connection Pooling**:
- Reuses TCP connections automatically
- Reduces overhead of TCP handshake
- Keeps ~10 connections alive per endpoint

**Timeout Strategy**:
- Default: 5 seconds per request
- Prevents hanging on slow endpoints
- Configurable via `--timeout` flag

### 3. Concurrent Testing Engine

**Architecture**:
```rust
async fn run_continuous_tests(
    client: Client,           // Shared HTTP client
    endpoint: String,         // RPC endpoint URL
    query: String,            // Query path (e.g., "status")
    duration: Duration,       // How long to test
    metrics: Arc<Mutex<...>>, // Thread-safe metrics
) {
    let end_time = Instant::now() + duration;
    
    while Instant::now() < end_time {
        // Make request, record result
        match test_endpoint_query(...).await {
            Ok(latency) => { /* record success */ },
            Err(_) => { /* record failure */ },
        }
        sleep(Duration::from_millis(100)).await;
    }
}
```

**Concurrency Model**:
- Spawns N workers per (endpoint, query) pair
- Each worker runs independent request loop
- Workers share metrics via `Arc<Mutex<TestMetrics>>`
- No worker coordination needed (embarrassingly parallel)

**Request Pacing**:
- 100ms sleep between requests per worker
- With 10 workers: ~100 requests/second per query
- Configurable via concurrency level

### 4. Latency Measurement (HDR Histogram)

**Purpose**: Capture accurate latency distribution

**Why HDR Histogram?**
- Maintains accuracy across 7 orders of magnitude
- O(1) recording and percentile calculation
- No coordination overhead between workers
- Memory efficient (~8KB per histogram)

**Implementation**:
```rust
use hdrhistogram::Histogram;

let mut hist = Histogram::<u64>::new(3).unwrap(); // 3 sig figs
hist.record(latency.as_micros() as u64);

// Later: Extract percentiles
let p50 = hist.value_at_quantile(0.50);
let p95 = hist.value_at_quantile(0.95);
let p99 = hist.value_at_quantile(0.99);
```

**Why microsecond precision?**
- Milliseconds too coarse for fast RPCs
- Can measure 10μs to 60,000ms range
- No loss of accuracy at any scale

### 5. Thread-Safe Metrics Sharing

**Challenge**: Multiple async tasks need to update shared metrics

**Solution**: `Arc<Mutex<TestMetrics>>`
```rust
struct TestMetrics {
    success_count: u64,
    failure_count: u64,
    latencies: Histogram<u64>,
}

// In each worker
let metrics = Arc::clone(&shared_metrics);
tokio::spawn(async move {
    let mut m = metrics.lock().await;  // Acquire lock
    m.success_count += 1;
    m.latencies.record(latency);
    // Lock released automatically
});
```

**Lock Contention**:
- Minimal: Each lock held for <1μs
- Dominated by network I/O (milliseconds)
- No performance impact in practice

### 6. Flakiness Scoring Algorithm

**Formula**:
```rust
fn calculate_flakiness_score(failure_rate: f64, p99_latency_ms: f64) -> f64 {
    let latency_threshold = 1000.0; // 1 second
    let latency_severity = (p99_latency_ms / latency_threshold).min(1.0);
    
    let score = (failure_rate × 0.7) + (latency_severity × 0.3);
    (score × 100.0).min(100.0)
}
```

**Rationale**:
- **70% weight on failures**: Reliability matters most
- **30% weight on latency**: Performance matters too
- **1000ms threshold**: Reasonable expectation for RPC
- **Capped at 100**: Normalized 0-100 scale

**Examples**:
| Failure Rate | p99 Latency | Score | Rating |
|--------------|-------------|-------|--------|
| 0% | 100ms | 3.0 | 🟢 Excellent |
| 5% | 500ms | 18.5 | 🟡 Good |
| 10% | 800ms | 31.0 | 🟠 Warning |
| 30% | 2000ms | 51.0 | 🔴 Critical |

### 7. Output Formatting

**Terminal Output** (colored):
- Emoji indicators (🟢🟡🟠🔴)
- ANSI color codes via `colored` crate
- Real-time progress updates
- Human-readable summary

**JSON Output**:
```rust
#[derive(Serialize)]
struct EndpointReport {
    endpoint: String,
    flakiness_score: f64,
    queries: Vec<QueryResult>,
    // ...
}
```

Serialized with `serde_json` for machine parsing.

## Data Flow

### 1. Initialization Phase
```
Parse CLI args → Validate inputs → Configure client → Print config
```

### 2. Testing Phase (per endpoint)
```
For each query:
  ├─ Spawn N concurrent workers
  │  └─ Each worker:
  │     ├─ Make HTTP request
  │     ├─ Measure latency
  │     └─ Update shared metrics
  │
  └─ Wait for duration to elapse
```

### 3. Aggregation Phase
```
For each query:
  ├─ Calculate failure rate
  ├─ Extract latency percentiles
  └─ Store QueryResult

Calculate overall metrics:
  ├─ Combined failure rate
  ├─ Average p99 latency
  └─ Flakiness score
```

### 4. Output Phase
```
Print terminal summary → Optionally write JSON → Exit
```

## Performance Characteristics

### Memory Usage
- **Base**: ~10MB (Rust runtime)
- **Per worker**: ~2KB (async task)
- **Per histogram**: ~8KB
- **Total**: ~50MB for 10 endpoints × 5 queries × 10 workers

### CPU Usage
- **Network-bound**: Mostly waiting on I/O
- **CPU spikes**: During histogram percentile calculation
- **Typical**: 5-15% CPU usage during tests

### Network Usage
- **Bandwidth**: Minimal (~1KB per request)
- **Connections**: ~10 per endpoint (pooled)
- **Total requests**: (concurrency × duration × 10) per query
  - Example: 10 workers × 60s × 10 req/s = 6000 requests

### Accuracy
- **Latency precision**: ±1μs
- **Percentile accuracy**: ±0.1%
- **Timing accuracy**: Limited by system clock (~1ms)

## Error Handling

### Network Errors
```rust
match client.get(url).send().await {
    Ok(response) => {
        if response.status().is_success() {
            // Success path
        } else {
            // HTTP error (4xx, 5xx)
        }
    }
    Err(e) => {
        // Connection error, timeout, DNS failure
    }
}
```

**Classified as failures**:
- Connection refused
- Timeout
- DNS resolution failure
- HTTP 4xx/5xx status codes

**Not retried**:
- Assumes transient failures are the signal we want to detect

### Lock Poisoning
```rust
let mut m = metrics.lock().await;
// If panic occurs, lock is poisoned
// We use panic=abort in release, so this never happens
```

### Overflow Protection
```rust
let score = (calculation).min(100.0); // Cap at 100
let latency_us = latency.as_micros().min(u64::MAX); // Cap at max
```

## Extensibility

### Adding New Queries
```rust
// In CLI
--queries "custom/module/v1/endpoint,another/endpoint"
```

No code changes needed!

### Custom Scoring
Edit `calculate_flakiness_score()`:
```rust
fn calculate_flakiness_score(...) -> f64 {
    // Adjust weights or threshold
    let score = (failure_rate × 0.8) + (latency_severity × 0.2);
    // ...
}
```

### Additional Metrics
Extend `TestMetrics`:
```rust
struct TestMetrics {
    // Existing fields...
    
    // New fields
    timeout_count: u64,
    connection_errors: u64,
    status_codes: HashMap<u16, u64>,
}
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_flakiness_score() {
        assert_eq!(calculate_flakiness_score(0.0, 100.0), 3.0);
        assert_eq!(calculate_flakiness_score(0.5, 2000.0), 65.0);
    }
}
```

### Integration Tests
```bash
# Test against mock server
cargo test --test integration
```

### Load Tests
```bash
# Stress test with high concurrency
./flake-detector --endpoints ... --concurrency 50 --duration 300
```

## Dependencies

| Crate | Version | Purpose | Size |
|-------|---------|---------|------|
| tokio | 1.35 | Async runtime | ~2MB |
| reqwest | 0.11 | HTTP client | ~1.5MB |
| clap | 4.4 | CLI parsing | ~500KB |
| serde | 1.0 | Serialization | ~300KB |
| serde_json | 1.0 | JSON output | ~200KB |
| hdrhistogram | 7.5 | Latency tracking | ~100KB |
| colored | 2.1 | Terminal colors | ~50KB |

**Total compiled size**: ~5MB (release build, stripped)

## Future Enhancements

1. **WebSocket support**: Test subscription endpoints
2. **Prometheus export**: `/metrics` endpoint for monitoring
3. **Historical tracking**: Store results in time-series DB
4. **Alerting**: Send alerts on degradation
5. **Query templates**: Presets for different Cosmos SDK versions
6. **gRPC support**: Test gRPC endpoints in addition to REST

## Security Considerations

- No API keys or credentials needed
- Read-only operations (HTTP GET)
- No persistent storage
- Safe to run against public RPCs

## License

MIT - See LICENSE file

---

**Questions about the architecture?** Open an issue on GitHub!