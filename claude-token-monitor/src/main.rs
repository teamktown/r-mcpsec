use clap::{Parser, Subcommand};
use claude_token_monitor::{
    models::*,
    services::{
        SessionService,
        session_tracker::SessionTracker, 
        file_monitor::{FileBasedTokenMonitor, explain_how_this_works},
    },
    ui::{TerminalUI, RatatuiTerminalUI},
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use chrono::Utc;
use log::debug;

#[derive(Parser)]
#[command(name = "claude-token-monitor")]
#[command(about = "A lightweight Rust client for Claude token usage monitoring")]
#[command(version = "0.2.6")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Update interval in seconds
    #[arg(short, long, default_value = "3")]
    interval: u64,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
    
    /// Force use of mock data instead of reading JSONL files (development only)
    #[arg(long)]
    force_mock: bool,
    
    /// Use basic terminal UI instead of enhanced Ratatui interface
    #[arg(long)]
    basic_ui: bool,
    
    /// Explain in detail how this tool works and what it monitors
    #[arg(long)]
    explain_how_this_works: bool,
    
    /// Show about information including version, author, and contributors
    #[arg(long)]
    about: bool,
}


#[derive(Subcommand)]
enum Commands {
    /// Start real-time monitoring (passive observation)
    Monitor {
        /// Plan type hint for calculations
        #[arg(short, long, default_value = "pro")]
        plan: String,
    },
    /// Show current observed session status
    Status,
    /// Show observed session history
    History {
        /// Number of sessions to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Configure the monitor
    Config {
        /// Set default plan hint
        #[arg(long)]
        plan: Option<String>,
        /// Set update interval
        #[arg(long)]
        interval: Option<u64>,
        /// Set warning threshold (0.0-1.0)
        #[arg(long)]
        threshold: Option<f64>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Add overflow checks in debug mode - PUT IT HERE
    #[cfg(debug_assertions)]
    std::panic::set_hook(Box::new(|panic_info| {
        debug!("PANIC: {panic_info}");
        std::process::exit(1);
    }));
    
    // Handle special flags first
    if cli.about {
        show_about();
        return Ok(());
    }
    
    if cli.explain_how_this_works {
        explain_how_this_works();
        return Ok(());
    }
    
    // Initialize logging
    if cli.verbose {
    // Log to file when verbose
    use std::fs::OpenOptions;
    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug.log")?;
    
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();
} else {
    // Normal logging to stderr for info/warn/error
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();
}

    // Setup data directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claude-token-monitor");
    
    std::fs::create_dir_all(&data_dir)?;
    
    // Load configuration
    let config = load_or_create_config(&data_dir)?;
    
    // Initialize services (passive observation)
    let session_tracker = SessionTracker::new(data_dir.join("observed_sessions.json"))?;
    let session_service = Arc::new(RwLock::new(session_tracker));
    
    // Update observed sessions from JSONL data
    session_service.write().await.update_observed_sessions().await?;
    
    // Initialize file-based token monitor
    let file_monitor = if cli.force_mock {
        println!("🔧 Running in forced mock mode - using simulated data");
        None
    } else {
        match FileBasedTokenMonitor::new() {
            Ok(mut monitor) => {
                println!("🔍 Scanning Claude usage files...");
                monitor.scan_usage_files().await?;
                println!("✅ Found {} usage entries", monitor.entry_count());
                if let Some((start, end)) = monitor.entry_time_range() {
                    println!("📊 Data range: {} to {}", 
                        humantime::format_rfc3339(start.into()),
                        humantime::format_rfc3339(end.into())
                    );
                }
                Some(monitor)
            }
            Err(e) => {
                debug!("⚠️ Failed to initialize file monitor: {e}");
                debug!("💡 Tip: Use --force-mock for development/testing with simulated data");
                debug!("📁 Make sure Claude Code has created usage files in ~/.claude/projects/");
                None
            }
        }
    };
    
    // Handle commands
    match cli.command {
        Some(Commands::Monitor { plan }) => {
            let plan_type = parse_plan_type(&plan)?;
            run_monitor(session_service, file_monitor, plan_type, config, cli.basic_ui, cli.force_mock).await?;
        }
        Some(Commands::Status) => {
            show_status(session_service).await?;
        }
        Some(Commands::History { limit }) => {
            show_history(session_service, limit).await?;
        }
        Some(Commands::Config { plan, interval, threshold }) => {
            configure_monitor(data_dir, plan, interval, threshold).await?;
        }
        None => {
            // Default to monitoring with Pro plan
            let plan_type = PlanType::Pro;
            run_monitor(session_service, file_monitor, plan_type, config, cli.basic_ui, cli.force_mock).await?;
        }
    }
    
    Ok(())
}


async fn run_monitor(
    session_service: Arc<RwLock<SessionTracker>>,
    file_monitor: Option<FileBasedTokenMonitor>,
    plan_type: PlanType,
    config: UserConfig,
    use_basic_ui: bool,
    use_mock: bool,
) -> Result<()> {
    println!("🧠 Claude Token Monitor - File-Based Edition");
    println!("Starting monitoring with plan: {plan_type:?}");
    
    // Update observed sessions from JSONL data (passive monitoring)
    session_service.write().await.update_observed_sessions().await?;
    
    // Calculate metrics from observed data
    let metrics = if use_mock {
        // Generate mock metrics for development
        let mock_session = TokenSession {
            id: "mock-session".to_string(),
            start_time: Utc::now() - chrono::Duration::minutes(30),
            end_time: None,
            plan_type: plan_type.clone(),
            tokens_used: 1500,
            tokens_limit: plan_type.default_limit(),
            is_active: true,
            reset_time: Utc::now() + chrono::Duration::hours(4),
        };
        generate_mock_metrics(mock_session)
    } else if let Some(ref monitor) = file_monitor {
        monitor.calculate_metrics().unwrap_or_else(|| {
            // If no data is available, create a placeholder using observed plan type if available
            println!("📝 No Claude usage data found in JSONL files");
            let observed_plan = monitor.derive_current_session()
                .map(|session| session.plan_type)
                .unwrap_or_else(|| plan_type.clone());
            
            debug!("Using plan type: {:?} (observed: {}, CLI hint: {:?})", 
                   observed_plan, 
                   monitor.derive_current_session().is_some(),
                   plan_type);
            
            UsageMetrics {
                current_session: TokenSession {
                    id: "no-data".to_string(),
                    start_time: Utc::now(),
                    end_time: None,
                    plan_type: observed_plan.clone(),
                    tokens_used: 0,
                    tokens_limit: observed_plan.default_limit(),
                    is_active: false,
                    reset_time: Utc::now() + chrono::Duration::hours(5),
                },
                usage_rate: 0.0,
                session_progress: 0.0,
                efficiency_score: 1.0,
                projected_depletion: None,
                usage_history: Vec::new(),
                
                // Default values for enhanced analytics
                cache_hit_rate: 0.0,
                cache_creation_rate: 0.0,
                token_consumption_rate: 0.0,
                input_output_ratio: 1.0,
            }
        })
    } else {
        debug!("❌ No file monitor available and not in mock mode");
        std::process::exit(1);
    };
    
    // Initialize and run UI based on CLI flag (Ratatui is default)
    // Try interactive UI first, fall back to status display if it fails
    let ui_result: Result<(), anyhow::Error> = if use_basic_ui {
        // Use basic terminal UI
        let mut ui = TerminalUI::new(config);
        match ui.init() {
            Ok(()) => {
                let result = ui.run(&metrics).await;
                let _ = ui.cleanup();
                result.map_err(|e| e.into())
            }
            Err(e) => Err(e.into())
        }
    } else {
        // Use enhanced Ratatui interface (default)
        match RatatuiTerminalUI::new(config) {
            Ok(mut ratatui_ui) => {
                let result = ratatui_ui.run(&metrics).await;
                let _ = ratatui_ui.cleanup();
                result
            }
            Err(e) => {
                debug!("💡 Enhanced UI not available: {e}");
                debug!("   Falling back to summary display...");
                Err(e)
            }
        }
    };
    
    // If UI fails, show status and exit gracefully
    if let Err(_) = ui_result {
        println!("📊 Token Usage Summary:");
        println!("  Session: {} ({})", metrics.current_session.id, 
                if metrics.current_session.is_active { "ACTIVE" } else { "INACTIVE" });
        println!("  Plan: {:?}", metrics.current_session.plan_type);
        println!("  Usage: {} / {} tokens ({:.1}%)", 
                metrics.current_session.tokens_used,
                metrics.current_session.tokens_limit,
                (metrics.current_session.tokens_used as f64 / metrics.current_session.tokens_limit as f64) * 100.0);
        println!("  Rate: {:.2} tokens/minute", metrics.usage_rate);
        println!("  Efficiency: {:.2}", metrics.efficiency_score);
        if let Some(depletion) = &metrics.projected_depletion {
            println!("  Projected depletion: {}", humantime::format_rfc3339((*depletion).into()));
        }
        println!();
        println!("💡 Interactive UI not available in this environment.");
        println!("   Use 'claude-token-monitor status' for quick checks.");
    }
    
    Ok(())
}

fn generate_mock_metrics(session: TokenSession) -> UsageMetrics {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    
    let mock_tokens_used = rng.gen_range(1000..5000);
    let usage_rate = rng.gen_range(50.0..200.0);
    let session_progress = rng.gen_range(0.1..0.8);
    let efficiency_score = rng.gen_range(0.6..1.0);
    
    let mut updated_session = session;
    updated_session.tokens_used = mock_tokens_used;
    
    UsageMetrics {
        current_session: updated_session,
        usage_rate,
        session_progress,
        efficiency_score,
        projected_depletion: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
        usage_history: Vec::new(),
        
        // Mock values for enhanced analytics
        cache_hit_rate: rng.gen_range(0.1..0.8),
        cache_creation_rate: rng.gen_range(10.0..50.0),
        token_consumption_rate: usage_rate,
        input_output_ratio: rng.gen_range(1.5..3.0),
    }
}

async fn show_status(session_service: Arc<RwLock<SessionTracker>>) -> Result<()> {
    let session_service = session_service.read().await;
    let active_session = session_service.get_active_session().await?;
    
    match active_session {
        Some(session) => {
            println!("📊 Current Session Status:");
            println!("  ID: {}", session.id);
            println!("  Plan: {:?}", session.plan_type);
            println!("  Tokens Used: {} / {}", session.tokens_used, session.tokens_limit);
            println!("  Usage: {:.1}%", (session.tokens_used as f64 / session.tokens_limit as f64) * 100.0);
            println!("  Started: {}", humantime::format_rfc3339(session.start_time.into()));
            println!("  Resets: {}", humantime::format_rfc3339(session.reset_time.into()));
            println!("  Status: {}", if session.is_active { "ACTIVE" } else { "INACTIVE" });
        }
        None => {
            println!("❌ No active session found");
        }
    }
    
    Ok(())
}

// Session creation/ending functions removed - this is a passive monitoring tool
// Sessions are observed from JSONL data, not created or managed by this tool

async fn show_history(
    session_service: Arc<RwLock<SessionTracker>>,
    limit: usize,
) -> Result<()> {
    let session_service = session_service.read().await;
    let sessions = session_service.get_session_history(limit).await?;
    
    if sessions.is_empty() {
        println!("📝 No session history found");
        return Ok(());
    }
    
    println!("📝 Session History ({} sessions):", sessions.len());
    println!("┌─────────────────────────────────────────────────────────────────────┐");
    println!("│ ID       │ Plan  │ Tokens    │ Started             │ Status   │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    
    for session in sessions {
        let status = if session.is_active { "ACTIVE" } else { "ENDED" };
        let usage_percent = (session.tokens_used as f64 / session.tokens_limit as f64) * 100.0;
        
        println!("│ {:<8} │ {:<5} │ {:<9} │ {:<19} │ {:<8} │",
            &session.id[..8],
            format!("{:?}", session.plan_type),
            format!("{}/{} ({:.1}%)", session.tokens_used, session.tokens_limit, usage_percent),
            humantime::format_rfc3339(session.start_time.into()),
            status
        );
    }
    
    println!("└─────────────────────────────────────────────────────────────────────┘");
    Ok(())
}

async fn configure_monitor(
    data_dir: PathBuf,
    plan: Option<String>,
    interval: Option<u64>,
    threshold: Option<f64>,
) -> Result<()> {
    let config_path = data_dir.join("config.json");
    let mut config = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        serde_json::from_str(&content)?
    } else {
        UserConfig::default()
    };
    
    if let Some(plan_str) = plan {
        config.default_plan = parse_plan_type(&plan_str)?;
        println!("✅ Set default plan to: {:?}", config.default_plan);
    }
    
    if let Some(interval_val) = interval {
        config.update_interval_seconds = interval_val;
        println!("✅ Set update interval to: {interval_val} seconds");
    }
    
    if let Some(threshold_val) = threshold {
        if (0.0..=1.0).contains(&threshold_val) {
            config.warning_threshold = threshold_val;
            println!("✅ Set warning threshold to: {:.1}%", threshold_val * 100.0);
        } else {
            println!("❌ Warning threshold must be between 0.0 and 1.0");
        }
    }
    
    // Save configuration
    let content = serde_json::to_string_pretty(&config)?;
    std::fs::write(&config_path, content)?;
    
    Ok(())
}

fn parse_plan_type(plan: &str) -> Result<PlanType> {
    match plan.to_lowercase().as_str() {
        "pro" => Ok(PlanType::Pro),
        "max5" => Ok(PlanType::Max5),
        "max20" => Ok(PlanType::Max20),
        _ => {
            if let Ok(limit) = plan.parse::<u32>() {
                Ok(PlanType::Custom(limit))
            } else {
                Err(anyhow::anyhow!("Invalid plan type: {}. Use 'pro', 'max5', 'max20', or a custom limit number", plan))
            }
        }
    }
}

fn load_or_create_config(data_dir: &PathBuf) -> Result<UserConfig> {
    let config_path = data_dir.join("config.json");
    
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        let config = UserConfig::default();
        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&config_path, content)?;
        Ok(config)
    }
}

/// Display about information including version, author, and contributors
fn show_about() {
    use colored::Colorize;
    
    println!("{}", "📱 Claude Token Monitor".bright_cyan().bold());
    println!();
    println!("{}", "📋 Version Information:".bright_yellow().bold());
    println!("  Version: {}", "v0.2.6".bright_green());
    println!("  Name: {}", "claude-token-monitor".bright_white());
    println!("  Description: A lightweight Rust client for Claude token usage monitoring");
    println!();
    
    println!("{}", "👨‍💻 Author:".bright_yellow().bold());
    println!("  Chris Phillips, Email: {}", "tools-claude-token-monitor@adiuco.com".bright_blue());
    println!();
    
    println!("{}", "🛠️ Built Using:".bright_yellow().bold());
    println!("  • {}", "ruv-swarm".bright_magenta().bold());
    println!("  • Rust programming language");
    println!("  • Tokio async runtime");
    println!("  • Ratatui terminal UI framework");
    println!();
    
    println!("{}", "🙏 Attribution & Contributors:".bright_yellow().bold());
    println!("  Original concept by: {}", "@Maciek-roboblog".bright_cyan());
    println!("  Repository: {}", "https://github.com/Maciek-roboblog/Claude-Code-Usage-Monitor".bright_blue());
    println!();
    
    println!("{}", "💡 Usage:".bright_green().bold());
    println!("  claude-token-monitor --help");
    println!("  claude-token-monitor --explain-how-this-works");
    println!("  claude-token-monitor monitor --plan pro");
}