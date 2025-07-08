# Rust Claude Token Usage Client - System Architecture

## Overview

This document defines the architecture for a lightweight, high-performance Rust client for tracking Claude API token usage, providing real-time monitoring, historical analytics, and cost optimization insights.

## Core Design Principles

1. **Performance First**: Zero-copy parsing, async I/O, minimal allocations
2. **Memory Efficiency**: Smart caching, streaming data processing
3. **Extensibility**: Plugin architecture for custom metrics and exporters
4. **Reliability**: Robust error handling, graceful degradation
5. **Observability**: Rich telemetry, structured logging, metrics export

## Crate Structure

```
claude-token-tracker/
├── Cargo.toml                    # Main workspace configuration
├── src/
│   ├── lib.rs                    # Public API and re-exports
│   ├── main.rs                   # CLI entry point
│   └── bin/
│       └── claude-monitor.rs     # Alternative monitoring daemon
├── crates/
│   ├── claude-core/              # Core token tracking logic
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs         # Claude API client
│   │       ├── tracker.rs        # Token usage tracking
│   │       ├── metrics.rs        # Metrics collection
│   │       └── types.rs          # Core data structures
│   ├── claude-config/            # Configuration management
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── settings.rs       # Configuration structures
│   │       └── validation.rs     # Config validation
│   ├── claude-storage/           # Data persistence layer
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── sqlite.rs         # SQLite backend
│   │       ├── memory.rs         # In-memory backend
│   │       └── traits.rs         # Storage abstractions
│   ├── claude-monitor/           # Real-time monitoring
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── realtime.rs       # Real-time tracking
│   │       ├── alerts.rs         # Alert system
│   │       └── dashboard.rs      # Terminal dashboard
│   └── claude-export/            # Data export and reporting
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── csv.rs            # CSV export
│           ├── json.rs           # JSON export
│           └── prometheus.rs     # Prometheus metrics
├── tests/                        # Integration tests
│   ├── integration/
│   └── fixtures/
├── examples/                     # Usage examples
│   ├── basic_usage.rs
│   ├── monitoring_dashboard.rs
│   └── custom_metrics.rs
├── docs/                         # Documentation
│   ├── architecture.md
│   ├── configuration.md
│   └── api.md
└── scripts/                      # Build and deployment scripts
    ├── build.sh
    └── install.sh
```

## Core Dependencies

### Runtime Dependencies
- `tokio` (1.0+) - Async runtime and utilities
- `reqwest` (0.11+) - HTTP client for Claude API
- `serde` (1.0+) - Serialization/deserialization
- `serde_json` (1.0+) - JSON handling
- `clap` (4.0+) - CLI argument parsing
- `anyhow` (1.0+) - Error handling
- `thiserror` (1.0+) - Custom error types
- `tracing` (0.1+) - Structured logging
- `tracing-subscriber` (0.3+) - Log formatting
- `config` (0.13+) - Configuration management
- `sqlx` (0.7+) - Database operations (optional)
- `crossterm` (0.27+) - Terminal UI for dashboard
- `tui` (0.19+) - Terminal user interface
- `chrono` (0.4+) - Date/time handling
- `uuid` (1.0+) - Unique identifiers

### Development Dependencies
- `criterion` (0.5+) - Benchmarking
- `tempfile` (3.0+) - Temporary files for testing
- `tokio-test` (0.4+) - Async testing utilities
- `wiremock` (0.5+) - HTTP mocking for tests

## Core Module Architecture

### 1. claude-core Crate

**Purpose**: Core token tracking logic and Claude API integration

**Key Components**:

```rust
// client.rs - Claude API client
pub struct ClaudeClient {
    http_client: reqwest::Client,
    api_key: SecretString,
    base_url: Url,
    rate_limiter: RateLimiter,
}

impl ClaudeClient {
    pub async fn send_request(&self, request: ChatRequest) -> Result<ChatResponse>;
    pub async fn get_usage_stats(&self) -> Result<UsageStats>;
    pub fn track_request(&self, request: &ChatRequest) -> TokenUsage;
}

// tracker.rs - Token usage tracking
pub struct TokenTracker {
    storage: Arc<dyn StorageBackend>,
    metrics: MetricsCollector,
    config: TrackerConfig,
}

impl TokenTracker {
    pub async fn track_usage(&self, usage: TokenUsage) -> Result<()>;
    pub async fn get_session_stats(&self, session_id: &str) -> Result<SessionStats>;
    pub async fn get_daily_stats(&self, date: NaiveDate) -> Result<DailyStats>;
}

// metrics.rs - Metrics collection
pub struct MetricsCollector {
    gauges: HashMap<String, f64>,
    counters: HashMap<String, u64>,
    histograms: HashMap<String, Vec<f64>>,
}

impl MetricsCollector {
    pub fn increment_counter(&mut self, name: &str, value: u64);
    pub fn set_gauge(&mut self, name: &str, value: f64);
    pub fn record_histogram(&mut self, name: &str, value: f64);
}
```

### 2. claude-config Crate

**Purpose**: Configuration management and validation

**Key Components**:

```rust
// settings.rs - Configuration structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api: ApiConfig,
    pub storage: StorageConfig,
    pub monitoring: MonitoringConfig,
    pub export: ExportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub backend: StorageBackend,
    pub sqlite_path: Option<PathBuf>,
    pub retention_days: u32,
}

// validation.rs - Config validation
impl AppConfig {
    pub fn validate(&self) -> Result<(), ConfigError>;
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError>;
    pub fn load_from_env() -> Result<Self, ConfigError>;
}
```

### 3. claude-storage Crate

**Purpose**: Data persistence and retrieval

**Key Components**:

```rust
// traits.rs - Storage abstractions
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn store_usage(&self, usage: &TokenUsage) -> Result<()>;
    async fn get_usage_history(&self, filters: &UsageFilter) -> Result<Vec<TokenUsage>>;
    async fn get_session_stats(&self, session_id: &str) -> Result<SessionStats>;
    async fn cleanup_old_data(&self, retention_days: u32) -> Result<u64>;
}

// sqlite.rs - SQLite implementation
pub struct SqliteBackend {
    pool: SqlitePool,
}

impl SqliteBackend {
    pub async fn new(database_url: &str) -> Result<Self>;
    pub async fn migrate(&self) -> Result<()>;
}

// memory.rs - In-memory implementation
pub struct MemoryBackend {
    data: Arc<RwLock<HashMap<String, TokenUsage>>>,
}
```

### 4. claude-monitor Crate

**Purpose**: Real-time monitoring and alerting

**Key Components**:

```rust
// realtime.rs - Real-time tracking
pub struct RealtimeMonitor {
    tracker: Arc<TokenTracker>,
    alert_manager: AlertManager,
    dashboard: Option<Dashboard>,
}

impl RealtimeMonitor {
    pub async fn start_monitoring(&self) -> Result<()>;
    pub async fn stop_monitoring(&self) -> Result<()>;
    pub fn get_live_stats(&self) -> LiveStats;
}

// alerts.rs - Alert system
pub struct AlertManager {
    rules: Vec<AlertRule>,
    channels: Vec<Box<dyn AlertChannel>>,
}

pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub cooldown: Duration,
}

// dashboard.rs - Terminal dashboard
pub struct Dashboard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    live_stats: Arc<RwLock<LiveStats>>,
}

impl Dashboard {
    pub async fn render(&mut self) -> Result<()>;
    pub fn handle_events(&mut self) -> Result<bool>;
}
```

## Data Structures

### Core Token Tracking

```rust
// Core usage tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub id: Uuid,
    pub session_id: String,
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub cost_usd: f64,
    pub request_duration_ms: u64,
    pub metadata: HashMap<String, String>,
}

// Session-level statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub total_requests: u32,
    pub total_tokens: u32,
    pub total_cost: f64,
    pub average_tokens_per_request: f64,
    pub models_used: HashSet<String>,
}

// Daily aggregated statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStats {
    pub date: NaiveDate,
    pub total_requests: u32,
    pub total_tokens: u32,
    pub total_cost: f64,
    pub unique_sessions: u32,
    pub models_breakdown: HashMap<String, ModelStats>,
}

// Model-specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub model: String,
    pub requests: u32,
    pub tokens: u32,
    pub cost: f64,
    pub avg_tokens_per_request: f64,
}

// Real-time monitoring data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveStats {
    pub current_session: Option<String>,
    pub requests_last_minute: u32,
    pub tokens_last_minute: u32,
    pub cost_last_minute: f64,
    pub requests_per_second: f64,
    pub tokens_per_second: f64,
    pub active_sessions: u32,
    pub uptime: Duration,
}
```

## Configuration Management

### Configuration File Structure

```toml
# claude-config.toml
[api]
api_key = "${CLAUDE_API_KEY}"
base_url = "https://api.anthropic.com"
timeout_seconds = 30
retry_attempts = 3

[storage]
backend = "sqlite"  # "sqlite" | "memory"
sqlite_path = "~/.claude-tracker/data.db"
retention_days = 90

[monitoring]
enabled = true
update_interval_ms = 1000
dashboard_enabled = true
dashboard_refresh_rate_ms = 500

[monitoring.alerts]
enabled = true
cost_threshold_usd = 50.0
token_rate_threshold = 1000  # tokens per minute
session_duration_threshold_minutes = 60

[export]
formats = ["json", "csv", "prometheus"]
output_dir = "~/.claude-tracker/exports"
auto_export_interval_hours = 24

[logging]
level = "info"
format = "json"
file_path = "~/.claude-tracker/logs/app.log"
```

### Configuration Loading Priority

1. Command line arguments
2. Environment variables (CLAUDE_*)
3. Configuration file (~/.claude-tracker/config.toml)
4. Default values

## CLI Interface Design

### Command Structure

```bash
# Main commands
claude-tracker [OPTIONS] [COMMAND]

# Commands:
#   start      Start monitoring Claude API usage
#   stop       Stop monitoring daemon
#   status     Show current monitoring status
#   stats      Display usage statistics
#   export     Export usage data
#   config     Manage configuration
#   dashboard  Launch interactive dashboard

# Global options:
#   --config <FILE>    Configuration file path
#   --verbose          Enable verbose logging
#   --quiet            Suppress output
#   --help             Show help information
#   --version          Show version information
```

### Command Examples

```bash
# Start monitoring with default config
claude-tracker start

# Start monitoring with custom config
claude-tracker start --config ./my-config.toml

# Show usage stats for last 7 days
claude-tracker stats --days 7

# Show stats for specific session
claude-tracker stats --session abc123

# Export data to CSV
claude-tracker export --format csv --output ./usage-report.csv

# Launch interactive dashboard
claude-tracker dashboard

# Configure API key
claude-tracker config set api.api_key "your-api-key"

# Show current configuration
claude-tracker config show
```

## Real-time Monitoring Implementation

### Architecture Overview

```rust
// Main monitoring loop
pub struct MonitoringService {
    tracker: Arc<TokenTracker>,
    config: MonitoringConfig,
    shutdown_tx: broadcast::Sender<()>,
}

impl MonitoringService {
    pub async fn run(&self) -> Result<()> {
        let mut interval = tokio::time::interval(
            Duration::from_millis(self.config.update_interval_ms)
        );
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.collect_metrics().await?;
                    self.check_alerts().await?;
                    self.update_dashboard().await?;
                }
                _ = self.shutdown_tx.subscribe().recv() => {
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    async fn collect_metrics(&self) -> Result<()> {
        // Collect live metrics
        let live_stats = self.calculate_live_stats().await?;
        
        // Update metrics store
        self.tracker.update_live_stats(live_stats).await?;
        
        Ok(())
    }
}
```

### Dashboard Implementation

```rust
// Terminal-based dashboard using tui-rs
pub struct Dashboard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    app_state: DashboardState,
}

#[derive(Debug, Clone)]
pub struct DashboardState {
    pub current_view: ViewType,
    pub live_stats: LiveStats,
    pub session_stats: Vec<SessionStats>,
    pub selected_session: Option<String>,
}

impl Dashboard {
    pub async fn render(&mut self) -> Result<()> {
        self.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(0),     // Main content
                    Constraint::Length(3),  // Footer
                ])
                .split(frame.size());
            
            self.render_header(frame, chunks[0]);
            self.render_main_content(frame, chunks[1]);
            self.render_footer(frame, chunks[2]);
        })?;
        
        Ok(())
    }
}
```

## Error Handling Strategy

### Error Types Hierarchy

```rust
// Main error type
#[derive(Error, Debug)]
pub enum ClaudeTrackerError {
    #[error("API error: {0}")]
    ApiError(#[from] ApiError),
    
    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),
    
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    
    #[error("Monitoring error: {0}")]
    MonitoringError(#[from] MonitoringError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

// API-specific errors
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    
    #[error("Service unavailable")]
    ServiceUnavailable,
}

// Storage-specific errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database connection failed")]
    ConnectionFailed,
    
    #[error("Migration failed: {message}")]
    MigrationFailed { message: String },
    
    #[error("Data corruption detected")]
    DataCorruption,
}
```

### Error Handling Patterns

```rust
// Graceful degradation for non-critical errors
impl TokenTracker {
    pub async fn track_usage(&self, usage: TokenUsage) -> Result<()> {
        // Primary storage attempt
        if let Err(e) = self.storage.store_usage(&usage).await {
            tracing::warn!("Primary storage failed: {}", e);
            
            // Fallback to in-memory storage
            self.fallback_storage.store_usage(&usage).await?;
        }
        
        // Metrics collection (non-blocking)
        if let Err(e) = self.metrics.record_usage(&usage).await {
            tracing::debug!("Metrics collection failed: {}", e);
            // Continue execution - metrics are not critical
        }
        
        Ok(())
    }
}

// Retry logic with exponential backoff
pub struct RetryPolicy {
    max_attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
}

impl RetryPolicy {
    pub async fn execute<F, T, E>(&self, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>>>>,
        E: std::fmt::Debug,
    {
        let mut delay = self.initial_delay;
        
        for attempt in 1..=self.max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) if attempt == self.max_attempts => return Err(e),
                Err(e) => {
                    tracing::warn!("Attempt {} failed: {:?}", attempt, e);
                    tokio::time::sleep(delay).await;
                    delay = (delay * self.backoff_multiplier as u32).min(self.max_delay);
                }
            }
        }
        
        unreachable!()
    }
}
```

## Performance Considerations

### Memory Management

1. **Streaming Processing**: Use iterators and async streams for large datasets
2. **Smart Caching**: LRU cache for frequently accessed data
3. **Zero-Copy Deserialization**: Use `serde` with zero-copy where possible
4. **Memory Pooling**: Reuse allocations for frequently created objects

### Concurrency Model

1. **Async-First**: All I/O operations are async
2. **Actor Pattern**: Isolated state management for different components
3. **Backpressure Handling**: Proper flow control for high-throughput scenarios
4. **Resource Limits**: Configurable limits on concurrent operations

### Database Optimization

1. **Connection Pooling**: Efficient database connection management
2. **Batch Operations**: Bulk inserts and updates
3. **Indexed Queries**: Proper indexing for common query patterns
4. **Data Archiving**: Automatic cleanup of old data

## Testing Strategy

### Unit Tests
- Test individual components in isolation
- Mock external dependencies
- Focus on business logic and edge cases

### Integration Tests
- Test component interactions
- Use test databases and mock APIs
- Verify end-to-end workflows

### Performance Tests
- Benchmark critical paths
- Memory usage profiling
- Load testing with high token volumes

### Property-Based Tests
- Use `quickcheck` for generating test data
- Test invariants and edge cases
- Verify serialization/deserialization roundtrips

## Deployment and Distribution

### Build Configuration

```toml
# Cargo.toml optimization settings
[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Distribution Methods

1. **Cargo**: Published to crates.io
2. **Binary Releases**: GitHub releases with pre-built binaries
3. **Docker**: Container image for easy deployment
4. **Package Managers**: Homebrew, APT, etc.

### Installation Scripts

```bash
# install.sh
#!/bin/bash
set -e

# Detect platform
OS=$(uname -s)
ARCH=$(uname -m)

# Download and install binary
curl -fsSL https://github.com/user/claude-tracker/releases/latest/download/claude-tracker-${OS}-${ARCH}.tar.gz | tar -xz
sudo mv claude-tracker /usr/local/bin/
sudo chmod +x /usr/local/bin/claude-tracker

echo "Claude Tracker installed successfully!"
```

## Future Extensibility

### Plugin Architecture

```rust
// Plugin trait for extensibility
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<()>;
    async fn on_token_usage(&self, usage: &TokenUsage) -> Result<()>;
    async fn on_session_end(&self, stats: &SessionStats) -> Result<()>;
    async fn cleanup(&self) -> Result<()>;
}

// Plugin manager
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
    
    pub async fn notify_token_usage(&self, usage: &TokenUsage) -> Result<()> {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_token_usage(usage).await {
                tracing::warn!("Plugin {} failed: {}", plugin.name(), e);
            }
        }
        Ok(())
    }
}
```

### Export Formats

1. **Standard Formats**: JSON, CSV, XML
2. **Metrics Systems**: Prometheus, StatsD, DataDog
3. **Databases**: InfluxDB, TimescaleDB
4. **Cloud Services**: AWS CloudWatch, Google Cloud Monitoring

This architecture provides a solid foundation for a lightweight, performant, and extensible Rust client for Claude token usage tracking. The modular design allows for easy maintenance and feature additions while maintaining high performance and reliability.