use clap::{Parser, Subcommand};
use claude_token_monitor::{
    models::*,
    services::{
        SessionService, TokenMonitorService, 
        session_tracker::SessionTracker, 
        token_monitor::TokenMonitor,
        api_client::ApiClient
    },
    ui::{TerminalUI, RatatuiTerminalUI},
    commands::auth,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use humantime;

#[derive(Parser)]
#[command(name = "claude-token-monitor")]
#[command(about = "A lightweight Rust client for Claude token usage monitoring")]
#[command(version = "0.2.0")]
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
    
    /// Force use of mock data instead of real API calls (development only)
    #[arg(long)]
    force_mock: bool,
    
    /// API key for Claude API (can also use CLAUDE_API_KEY env var or ~/.claude/.credentials.json)
    #[arg(long)]
    api_key: Option<String>,
    
    /// Use basic terminal UI instead of enhanced Ratatui interface
    #[arg(long)]
    basic_ui: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start real-time monitoring
    Monitor {
        /// Plan type to use
        #[arg(short, long, default_value = "pro")]
        plan: String,
    },
    /// Show current session status
    Status,
    /// Create a new session
    Create {
        /// Plan type for new session
        #[arg(short, long, default_value = "pro")]
        plan: String,
    },
    /// End current session
    End,
    /// Show session history
    History {
        /// Number of sessions to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Configure the monitor
    Config {
        /// Set default plan
        #[arg(long)]
        plan: Option<String>,
        /// Set update interval
        #[arg(long)]
        interval: Option<u64>,
        /// Set warning threshold (0.0-1.0)
        #[arg(long)]
        threshold: Option<f64>,
    },
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        auth_command: AuthCommands,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Check authentication status
    Status,
    /// Show authentication help
    Help,
    /// Validate current credentials
    Validate,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level))
        .init();

    // Setup data directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("claude-token-monitor");
    
    std::fs::create_dir_all(&data_dir)?;
    
    // Load configuration
    let config = load_or_create_config(&data_dir)?;
    
    // Initialize services
    let session_tracker = SessionTracker::new(data_dir.join("sessions.json"));
    let session_service = Arc::new(RwLock::new(session_tracker));
    
    // Load existing sessions
    session_service.write().await.load_sessions().await?;
    
    // Initialize token monitor with API client or fail if no credentials
    let token_monitor = if cli.force_mock {
        println!("🔧 Running in forced mock mode - using simulated data");
        TokenMonitor::new(Arc::clone(&session_service), cli.interval)
    } else {
        match initialize_api_client(cli.api_key).await {
            Ok(api_client) => {
                println!("✅ Connected to Claude API");
                TokenMonitor::with_api_client(
                    Arc::clone(&session_service),
                    cli.interval,
                    api_client
                )
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to Claude API: {}", e);
                eprintln!("💡 Tip: Use --force-mock for development/testing with simulated data");
                eprintln!("🔧 Or set up credentials: https://docs.anthropic.com/claude/reference/getting-started");
                std::process::exit(1);
            }
        }
    };
    
    // Handle commands
    match cli.command {
        Some(Commands::Monitor { plan }) => {
            let plan_type = parse_plan_type(&plan)?;
            run_monitor(session_service, token_monitor, plan_type, config, cli.basic_ui).await?;
        }
        Some(Commands::Status) => {
            show_status(session_service).await?;
        }
        Some(Commands::Create { plan }) => {
            let plan_type = parse_plan_type(&plan)?;
            create_session(session_service, plan_type).await?;
        }
        Some(Commands::End) => {
            end_session(session_service).await?;
        }
        Some(Commands::History { limit }) => {
            show_history(session_service, limit).await?;
        }
        Some(Commands::Config { plan, interval, threshold }) => {
            configure_monitor(data_dir, plan, interval, threshold).await?;
        }
        Some(Commands::Auth { auth_command }) => {
            match auth_command {
                AuthCommands::Status => {
                    auth::check_auth_status().await?;
                }
                AuthCommands::Help => {
                    auth::show_auth_help();
                }
                AuthCommands::Validate => {
                    auth::validate_credentials().await?;
                }
            }
        }
        None => {
            // Default to monitoring with Pro plan
            let plan_type = PlanType::Pro;
            run_monitor(session_service, token_monitor, plan_type, config, cli.basic_ui).await?;
        }
    }
    
    Ok(())
}

async fn initialize_api_client(api_key: Option<String>) -> Result<ApiClient> {
    // Show available credential sources
    println!("🔍 Checking available credential sources:");
    for source_info in ApiClient::check_credential_sources() {
        println!("  {}", source_info);
    }
    println!();

    let client = if let Some(key) = api_key {
        // Use provided API key directly
        println!("🔑 Using API key provided via --api-key flag");
        let credential_source = CredentialSource::Direct(key);
        ApiClient::with_credentials(credential_source)?
    } else {
        // Try Claude CLI credentials first, then fallback to environment variables
        match ApiClient::from_claude_cli() {
            Ok(client) => {
                println!("✅ Using Claude CLI credentials from ~/.claude/.credentials.json");
                
                // Show credential info if available
                if let Ok(creds) = CredentialManager::check_claude_cli_credentials() {
                    println!("📋 Credential Info: {}", creds.get_info_for_logging());
                }
                
                client
            }
            Err(e) => {
                println!("⚠️ Claude CLI credentials not available: {}", e);
                println!("🔄 Falling back to environment variables...");
                ApiClient::from_env()?
            }
        }
    };
    
    // Test the connection
    if client.test_connection().await? {
        println!("🔗 API connection test successful");
        println!("📊 Config: {}", client.get_config_info());
    } else {
        return Err(anyhow::anyhow!("API connection test failed"));
    }
    
    Ok(client)
}

async fn run_monitor(
    session_service: Arc<RwLock<SessionTracker>>,
    mut token_monitor: TokenMonitor<SessionTracker>,
    plan_type: PlanType,
    config: UserConfig,
    use_basic_ui: bool,
) -> Result<()> {
    println!("🧠 Claude Token Monitor - Hive Mind Edition");
    println!("Starting monitoring with plan: {:?}", plan_type);
    
    // Ensure we have an active session
    let active_session = session_service.read().await.get_active_session().await?;
    if active_session.is_none() {
        println!("No active session found. Creating new session...");
        let mut session_service_write = session_service.write().await;
        session_service_write.create_session(plan_type).await?;
    }
    
    // Start monitoring
    token_monitor.start_monitoring().await?;
    
    // Wait for initial metrics
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Get current metrics
    let metrics = token_monitor.get_current_usage().await?;
    
    // Initialize and run UI based on CLI flag (Ratatui is default)
    if use_basic_ui {
        // Use basic terminal UI
        let mut ui = TerminalUI::new(config);
        ui.init()?;
        ui.run(&metrics).await?;
        ui.cleanup()?;
    } else {
        // Use enhanced Ratatui interface (default)
        let mut ratatui_ui = RatatuiTerminalUI::new(config)?;
        ratatui_ui.run(&metrics).await?;
        ratatui_ui.cleanup()?;
    }
    
    token_monitor.stop_monitoring().await?;
    Ok(())
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

async fn create_session(
    session_service: Arc<RwLock<SessionTracker>>,
    plan_type: PlanType,
) -> Result<()> {
    let mut session_service = session_service.write().await;
    let session = session_service.create_session(plan_type).await?;
    
    println!("✅ Created new session:");
    println!("  ID: {}", session.id);
    println!("  Plan: {:?}", session.plan_type);
    println!("  Limit: {} tokens", session.tokens_limit);
    println!("  Resets: {}", humantime::format_rfc3339(session.reset_time.into()));
    
    Ok(())
}

async fn end_session(session_service: Arc<RwLock<SessionTracker>>) -> Result<()> {
    let session_service_read = session_service.read().await;
    let active_session = session_service_read.get_active_session().await?;
    drop(session_service_read);
    
    match active_session {
        Some(session) => {
            let mut session_service_write = session_service.write().await;
            session_service_write.end_session(&session.id).await?;
            println!("✅ Ended session: {}", session.id);
        }
        None => {
            println!("❌ No active session to end");
        }
    }
    
    Ok(())
}

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
        println!("✅ Set update interval to: {} seconds", interval_val);
    }
    
    if let Some(threshold_val) = threshold {
        if threshold_val >= 0.0 && threshold_val <= 1.0 {
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