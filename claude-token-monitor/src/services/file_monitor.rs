use crate::models::*;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tokio::fs;
use walkdir::WalkDir;

/// Claude usage entry from JSONL files
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UsageEntry {
    pub timestamp: DateTime<Utc>,
    pub usage: TokenUsage,
    pub model: Option<String>,
    pub message_id: Option<String>,
    pub request_id: Option<String>,
}

/// Token usage information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
}

impl TokenUsage {
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens 
            + self.output_tokens 
            + self.cache_creation_input_tokens.unwrap_or(0)
            + self.cache_read_input_tokens.unwrap_or(0)
    }
}

/// File-based Claude token monitor that reads JSONL files
pub struct FileBasedTokenMonitor {
    claude_data_paths: Vec<PathBuf>,
    usage_entries: Vec<UsageEntry>,
    _last_scan: DateTime<Utc>,
}

impl FileBasedTokenMonitor {
    pub fn new() -> Result<Self> {
        let claude_data_paths = Self::discover_claude_paths()?;
        
        if claude_data_paths.is_empty() {
            log::warn!("No Claude data directories found. Token monitoring may not work correctly.");
        } else {
            log::info!("Found Claude data paths: {:?}", claude_data_paths);
        }

        Ok(Self {
            claude_data_paths,
            usage_entries: Vec::new(),
            _last_scan: Utc::now(),
        })
    }

    /// Discover Claude data directories based on standard locations
    pub fn discover_claude_paths() -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        
        // Standard Claude data locations
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
        
        let standard_paths = vec![
            home_dir.join(".claude").join("projects"),
            home_dir.join(".config").join("claude").join("projects"),
        ];
        
        // Check environment variables
        if let Ok(env_paths) = std::env::var("CLAUDE_DATA_PATHS") {
            for path_str in env_paths.split(':') {
                paths.push(PathBuf::from(path_str));
            }
        }
        
        if let Ok(env_path) = std::env::var("CLAUDE_DATA_PATH") {
            paths.push(PathBuf::from(env_path));
        }
        
        // Add standard paths
        paths.extend(standard_paths);
        
        // Filter to only existing directories
        let existing_paths: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| path.exists() && path.is_dir())
            .collect();
        
        Ok(existing_paths)
    }

    /// Scan all Claude data directories for JSONL files and parse usage data
    pub async fn scan_usage_files(&mut self) -> Result<()> {
        let mut all_entries = Vec::new();
        
        for data_path in &self.claude_data_paths {
            log::debug!("Scanning directory: {:?}", data_path);
            
            // Find all .jsonl files recursively
            for entry in WalkDir::new(data_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "jsonl"))
            {
                let file_path = entry.path();
                log::debug!("Parsing JSONL file: {:?}", file_path);
                
                match self.parse_jsonl_file(file_path).await {
                    Ok(mut entries) => {
                        all_entries.append(&mut entries);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse JSONL file {:?}: {}", file_path, e);
                    }
                }
            }
        }
        
        // Sort entries by timestamp
        all_entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        
        // Deduplicate based on message_id and request_id
        let mut dedup_map = HashMap::new();
        for entry in all_entries {
            let key = (entry.message_id.clone(), entry.request_id.clone());
            dedup_map.insert(key, entry);
        }
        
        self.usage_entries = dedup_map.into_values().collect();
        self.usage_entries.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        
        log::info!("Loaded {} usage entries from JSONL files", self.usage_entries.len());
        Ok(())
    }

    /// Parse a single JSONL file for usage entries
    async fn parse_jsonl_file(&self, file_path: &Path) -> Result<Vec<UsageEntry>> {
        let content = fs::read_to_string(file_path).await?;
        let mut entries = Vec::new();
        
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(json) => {
                    if let Ok(entry) = self.parse_usage_entry(json) {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    log::debug!("Skipping invalid JSON line {} in {:?}: {}", line_num + 1, file_path, e);
                }
            }
        }
        
        Ok(entries)
    }

    /// Parse a JSON value into a UsageEntry
    fn parse_usage_entry(&self, json: serde_json::Value) -> Result<UsageEntry> {
        // Extract timestamp
        let timestamp = if let Some(ts_str) = json.get("timestamp").and_then(|v| v.as_str()) {
            DateTime::parse_from_rfc3339(ts_str)?.with_timezone(&Utc)
        } else {
            return Err(anyhow!("Missing or invalid timestamp"));
        };

        // Extract usage information
        let usage = if let Some(usage_obj) = json.get("usage") {
            TokenUsage {
                input_tokens: usage_obj.get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                output_tokens: usage_obj.get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                cache_creation_input_tokens: usage_obj.get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
                cache_read_input_tokens: usage_obj.get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32),
            }
        } else {
            return Err(anyhow!("Missing usage information"));
        };

        Ok(UsageEntry {
            timestamp,
            usage,
            model: json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string()),
            message_id: json.get("message_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            request_id: json.get("request_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        })
    }

    /// Calculate current usage metrics from loaded entries
    pub fn calculate_metrics(&self, session: &TokenSession) -> UsageMetrics {
        let now = Utc::now();
        let session_start = session.start_time;
        let one_hour_ago = now - chrono::Duration::hours(1);
        
        // Filter entries for current session (within session timeframe)
        let session_entries: Vec<&UsageEntry> = self.usage_entries
            .iter()
            .filter(|entry| entry.timestamp >= session_start && entry.timestamp <= now)
            .collect();
        
        // Filter entries for last hour (for burn rate calculation)
        let recent_entries: Vec<&UsageEntry> = self.usage_entries
            .iter()
            .filter(|entry| entry.timestamp >= one_hour_ago)
            .collect();
        
        // Calculate total tokens used in current session
        let total_tokens_used: u32 = session_entries
            .iter()
            .map(|entry| entry.usage.total_tokens())
            .sum();
        
        // Calculate tokens used in last hour (for future burn rate analysis)
        let _tokens_last_hour: u32 = recent_entries
            .iter()
            .map(|entry| entry.usage.total_tokens())
            .sum();
        
        // Calculate time elapsed
        let time_elapsed = now.signed_duration_since(session_start);
        let time_elapsed_minutes = time_elapsed.num_minutes() as f64;
        
        // Calculate usage rate (tokens per minute)
        let usage_rate = if time_elapsed_minutes > 0.0 {
            total_tokens_used as f64 / time_elapsed_minutes
        } else {
            0.0
        };
        
        // Calculate session progress (0.0 to 1.0)
        let session_duration_minutes = 5.0 * 60.0; // 5 hours in minutes
        let session_progress = (time_elapsed_minutes / session_duration_minutes).min(1.0);
        
        // Calculate efficiency score
        let efficiency_score = if session_progress > 0.0 {
            let expected_rate = session.tokens_limit as f64 / session_duration_minutes;
            let actual_rate = if usage_rate > 0.0 { usage_rate } else { 0.1 };
            (expected_rate / actual_rate).min(1.0).max(0.0)
        } else {
            1.0
        };
        
        // Calculate projected depletion
        let projected_depletion = if usage_rate > 0.0 {
            let remaining_tokens = session.tokens_limit.saturating_sub(total_tokens_used);
            let minutes_remaining = remaining_tokens as f64 / usage_rate;
            Some(now + chrono::Duration::minutes(minutes_remaining as i64))
        } else {
            None
        };
        
        // Update session with actual token count
        let mut updated_session = session.clone();
        updated_session.tokens_used = total_tokens_used;
        
        UsageMetrics {
            current_session: updated_session,
            usage_rate,
            session_progress,
            efficiency_score,
            projected_depletion,
            usage_history: Vec::new(), // Will be populated in future iterations
        }
    }

    /// Get the number of usage entries loaded
    pub fn entry_count(&self) -> usize {
        self.usage_entries.len()
    }

    /// Get the time range of loaded entries
    pub fn entry_time_range(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        if self.usage_entries.is_empty() {
            None
        } else {
            Some((
                self.usage_entries.first().unwrap().timestamp,
                self.usage_entries.last().unwrap().timestamp,
            ))
        }
    }

    /// Start file system watcher for real-time updates
    pub fn start_file_watcher(&self) -> Result<mpsc::Receiver<notify::Result<Event>>> {
        let (tx, rx) = mpsc::channel();
        
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        
        // Watch all Claude data directories
        for path in &self.claude_data_paths {
            watcher.watch(path, RecursiveMode::Recursive)?;
            log::info!("Watching directory for changes: {:?}", path);
        }
        
        // Keep watcher alive by storing it in a static or similar
        // For now, we'll return the receiver and let the caller manage the watcher
        std::mem::forget(watcher);
        
        Ok(rx)
    }
}

/// Display detailed explanation of how the tool works
pub fn explain_how_this_works() {
    println!("{}", "🧠 Claude Token Monitor - How It Works".bright_cyan().bold());
    println!();
    println!("{}", "📋 Overview:".bright_yellow().bold());
    println!("This tool monitors your Claude AI token usage by reading local files that Claude Code");
    println!("writes during your conversations. No API calls or authentication required!");
    println!();
    
    println!("{}", "📁 What Files It Monitors:".bright_yellow().bold());
    println!("• ~/.claude/projects/**/*.jsonl (primary location)");
    println!("• ~/.config/claude/projects/**/*.jsonl (alternative location)");
    println!("• Custom paths from CLAUDE_DATA_PATHS or CLAUDE_DATA_PATH environment variables");
    println!();
    
    println!("{}", "🔍 What Data It Reads:".bright_yellow().bold());
    println!("• Token usage counts (input, output, cache tokens)");
    println!("• Timestamps of each Claude interaction");
    println!("• Model information and message IDs");
    println!("• Session and request identifiers");
    println!();
    
    println!("{}", "📊 How It Calculates Metrics:".bright_yellow().bold());
    println!("• Usage Rate: Total tokens ÷ Time elapsed (tokens/minute)");
    println!("• Session Progress: Time elapsed ÷ Session duration (5 hours)");
    println!("• Efficiency Score: Expected rate ÷ Actual rate (0.0-1.0)");
    println!("• Projected Depletion: Remaining tokens ÷ Current usage rate");
    println!();
    
    println!("{}", "⚡ Real-time Updates:".bright_yellow().bold());
    println!("• Watches file system for new .jsonl files");
    println!("• Updates metrics when Claude Code writes new usage data");
    println!("• Scans directories every few seconds for changes");
    println!();
    
    println!("{}", "🔒 Privacy & Security:".bright_yellow().bold());
    println!("• No network connections to Claude servers");
    println!("• No API keys or authentication required");
    println!("• Only reads existing local files written by Claude Code");
    println!("• Does not access conversation content, only token counts");
    println!();
    
    println!("{}", "🎯 Session Management:".bright_yellow().bold());
    println!("• Tracks multiple Claude plan types (Pro, Max5, Max20)");
    println!("• Maintains session history in ~/.local/share/claude-token-monitor/");
    println!("• Calculates token limits and reset times based on plan type");
    println!("• Provides warnings when approaching token limits");
    println!();
    
    println!("{}", "💡 Pro Tips:".bright_yellow().bold());
    println!("• Use with Claude Code for automatic token tracking");
    println!("• Set CLAUDE_DATA_PATHS to monitor custom directories");
    println!("• Check 'Settings' tab in the UI for technical details");
    println!("• Monitor shows both current session and hourly burn rates");
    println!();
    
    println!("{}", "🚀 Getting Started:".bright_green().bold());
    println!("1. Make sure you have Claude Code installed and configured");
    println!("2. Start a conversation with Claude Code to generate usage data");
    println!("3. Run: claude-token-monitor");
    println!("4. The tool will automatically find and monitor your usage files");
    println!();
}

// Re-export from colored crate for the explanation function
use colored::Colorize;