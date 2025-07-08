# Claude Token Monitor - Rust Edition

🧠 **Hive Mind Swarm Build** - A lightweight, high-performance Rust client for monitoring Claude AI token usage.

## Features

- 🔥 **Real-time monitoring** with customizable update intervals
- 📊 **Visual progress bars** and color-coded terminal UI
- 🤖 **Smart predictions** for token depletion timing
- 📈 **Usage analytics** and efficiency scoring
- 🔄 **Session management** with persistent storage
- ⚡ **Lightning fast** - built with Rust for performance
- 🛠️ **Configurable** plans and thresholds
- 📱 **Cross-platform** support (Linux, macOS, Windows)

## Installation

### From Source

1. Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clone and build:
```bash
git clone https://github.com/teamktown/claude-token-monitor
cd claude-token-monitor
cargo build --release
```

3. Install globally:
```bash
cargo install --path .
```

## Usage

### Quick Start

Start monitoring with default settings:
```bash
claude-token-monitor
```

### Commands

#### Monitor in Real-time
```bash
# Start monitoring with Pro plan
claude-token-monitor monitor --plan pro

# Monitor with custom update interval
claude-token-monitor monitor --plan max5 --interval 5
```

#### Session Management
```bash
# Create a new session
claude-token-monitor create --plan pro

# Check current session status
claude-token-monitor status

# End current session
claude-token-monitor end

# View session history
claude-token-monitor history --limit 20
```

#### Configuration
```bash
# Set default plan
claude-token-monitor config --plan max20

# Set update interval
claude-token-monitor config --interval 2

# Set warning threshold (85% = 0.85)
claude-token-monitor config --threshold 0.9
```

### Plan Types

- **pro**: 40,000 tokens per 5-hour session
- **max5**: 20,000 tokens per 5-hour session  
- **max20**: 100,000 tokens per 5-hour session
- **custom**: Specify custom token limit (e.g., `--plan 50000`)

## Interface

The terminal UI provides:

```
╔═══════════════════════════════════════════════════════════════════════════════╗
║                            Claude Token Monitor                               ║
║                        Rust Edition - Hive Mind Build                        ║
╚═══════════════════════════════════════════════════════════════════════════════╝

Session Information:
  Plan Type: Pro
  Status: ACTIVE
  Session ID: a1b2c3d4
  Started: 2025-07-08 01:30:00 UTC
  Resets: 2025-07-08 06:30:00 UTC

Token Usage Progress:
  ████████████████████████████████████████████████░░ 85.2%
  34,080 / 40,000 tokens used

Usage Statistics:
  Usage Rate: 125.50 tokens/minute
  Session Progress: 67.3%
  Efficiency Score: 0.92

Predictions:
  Projected Depletion: 2h 15m (04:45:00 UTC)

Controls: [Q]uit | [R]efresh | [Ctrl+C] Exit
```

## Configuration

Configuration is stored in:
- Linux: `~/.local/share/claude-token-monitor/config.json`
- macOS: `~/Library/Application Support/claude-token-monitor/config.json`
- Windows: `%APPDATA%\claude-token-monitor\config.json`

Example configuration:
```json
{
  "default_plan": "Pro",
  "timezone": "UTC",
  "update_interval_seconds": 3,
  "warning_threshold": 0.85,
  "auto_switch_plans": true,
  "color_scheme": {
    "progress_bar_full": "green",
    "progress_bar_empty": "gray",
    "warning_color": "yellow",
    "success_color": "green",
    "error_color": "red",
    "info_color": "blue"
  }
}
```

## Architecture

The client is built with a modular architecture:

```
src/
├── main.rs              # CLI interface and command handling
├── lib.rs               # Library exports
├── models/
│   └── mod.rs          # Data structures (TokenSession, UsageMetrics, etc.)
├── services/
│   ├── mod.rs          # Service traits and interfaces
│   ├── session_tracker.rs  # Session management and persistence
│   └── token_monitor.rs     # Real-time monitoring logic
└── ui/
    └── mod.rs          # Terminal UI and display components
```

### Key Components

- **TokenSession**: Represents a Claude usage session with metadata
- **SessionTracker**: Manages session lifecycle and persistence
- **TokenMonitor**: Real-time monitoring with async updates
- **TerminalUI**: Full-screen terminal interface with progress bars
- **UsageMetrics**: Analytics and predictions for token consumption

## Performance

Built with Rust for maximum performance:
- **Memory efficient**: Minimal memory footprint
- **Fast startup**: Sub-second initialization
- **Concurrent**: Async/await for non-blocking operations
- **Cross-platform**: Works on Linux, macOS, and Windows

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Linting

```bash
cargo clippy
```

### Formatting

```bash
cargo fmt
```

## API Integration

Currently uses simulated data for demonstration. To integrate with Claude's actual API:

1. Add your API key to environment variables
2. Implement the `fetch_current_token_usage()` method in `TokenMonitor`
3. Add proper error handling for API failures

Example API integration:
```rust
async fn fetch_current_token_usage(&self) -> Result<u32> {
    let client = reqwest::Client::new();
    let response = client.get("https://api.anthropic.com/v1/usage")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;
    
    let usage_data: TokenUsageResponse = response.json().await?;
    Ok(usage_data.tokens_used)
}
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Run `cargo test` and `cargo clippy`
6. Submit a pull request

## License

MIT License - see LICENSE file for details.

## Acknowledgments

- Inspired by [Claude-Code-Usage-Monitor](https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor)
- Built with the Hive Mind Swarm collective intelligence system
- Powered by Rust and tokio for high performance

---

**🧠 Built by the Hive Mind Swarm - Collective Intelligence in Action**