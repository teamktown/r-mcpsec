use crate::models::*;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use tokio::fs;
use walkdir::WalkDir;

// Security constants for JSON parsing limits
const MAX_JSON_SIZE: usize = 1024 * 1024; // 1MB max per JSON line
const MAX_JSON_DEPTH: usize = 32; // Maximum nesting depth
const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50MB max file size

/// Claude usage entry from JSONL files
#[derive(Clone, Deserialize, Serialize)]
pub struct UsageEntry {
    pub timestamp: DateTime<Utc>,
    pub usage: TokenUsage,
    pub model: Option<String>,
    pub message_id: Option<String>,
    pub request_id: Option<String>,
}

impl fmt::Debug for UsageEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsageEntry")
            .field("timestamp", &self.timestamp)
            .field("usage", &self.usage)
            .field("model", &self.model)
            .field("message_id", &self.message_id.as_ref().map(|_| "[REDACTED]")) // Redact message ID
            .field("request_id", &self.request_id.as_ref().map(|_| "[REDACTED]")) // Redact request ID
            .finish()
    }
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
    
    /// Calculate cache hit rate (cache read tokens / total input tokens)
    pub fn cache_hit_rate(&self) -> f64 {
        let total_input = self.input_tokens + self.cache_creation_input_tokens.unwrap_or(0);
        if total_input == 0 {
            0.0
        } else {
            self.cache_read_input_tokens.unwrap_or(0) as f64 / total_input as f64
        }
    }
    
    /// Get cache creation tokens
    pub fn cache_creation_tokens(&self) -> u32 {
        self.cache_creation_input_tokens.unwrap_or(0)
    }
    
    /// Get cache read tokens  
    pub fn cache_read_tokens(&self) -> u32 {
        self.cache_read_input_tokens.unwrap_or(0)
    }
}

/// File-based Claude token monitor that reads JSONL files
pub struct FileBasedTokenMonitor {
    claude_data_paths: Vec<PathBuf>,
    usage_entries: Vec<UsageEntry>,
    _last_scan: DateTime<Utc>,
    _watcher: Option<Arc<Mutex<RecommendedWatcher>>>,
}

impl FileBasedTokenMonitor {
    pub fn new() -> Result<Self> {
        let claude_data_paths = Self::discover_claude_paths()?;
        
        if claude_data_paths.is_empty() {
            log::warn!("No Claude data directories found. Token monitoring may not work correctly.");
        } else {
            log::info!("Found Claude data paths: {claude_data_paths:?}");
        }

        Ok(Self {
            claude_data_paths,
            usage_entries: Vec::new(),
            _last_scan: Utc::now(),
            _watcher: None,
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
        
        // Check environment variables with validation
        if let Ok(env_paths) = std::env::var("CLAUDE_DATA_PATHS") {
            for path_str in env_paths.split(':') {
                if let Ok(validated_path) = Self::validate_and_canonicalize_path(path_str) {
                    paths.push(validated_path);
                } else {
                    log::warn!("Invalid path in CLAUDE_DATA_PATHS: {path_str}");
                }
            }
        }
        
        if let Ok(env_path) = std::env::var("CLAUDE_DATA_PATH") {
            if let Ok(validated_path) = Self::validate_and_canonicalize_path(&env_path) {
                paths.push(validated_path);
            } else {
                log::warn!("Invalid path in CLAUDE_DATA_PATH: {env_path}");
            }
        }
        
        // Add standard paths
        paths.extend(standard_paths);
        
        // Filter to only existing directories and canonicalize
        let existing_paths: Vec<PathBuf> = paths
            .into_iter()
            .filter_map(|path| {
                if path.exists() && path.is_dir() {
                    path.canonicalize().ok()
                } else {
                    None
                }
            })
            .collect();
        
        Ok(existing_paths)
    }
    
    /// Validate and canonicalize a path to prevent directory traversal attacks
    fn validate_and_canonicalize_path(path_str: &str) -> Result<PathBuf> {
        // Reject empty paths
        if path_str.trim().is_empty() {
            return Err(anyhow!("Empty path not allowed"));
        }
        
        // Reject paths with null bytes
        if path_str.contains('\0') {
            return Err(anyhow!("Path contains null bytes"));
        }
        
        // Reject paths that are too long
        if path_str.len() > 4096 {
            return Err(anyhow!("Path too long (max 4096 characters)"));
        }
        
        let path = PathBuf::from(path_str);
        
        // Reject relative paths that try to escape
        if path_str.contains("../") || path_str.contains("..\\") {
            return Err(anyhow!("Relative path traversal not allowed"));
        }
        
        // Canonicalize the path to resolve symlinks and normalize
        let canonical_path = path.canonicalize()
            .map_err(|e| anyhow!("Failed to canonicalize path {}: {}", path_str, e))?;
        
        // Ensure the canonical path is within reasonable bounds (under home directory)
        if let Some(home_dir) = dirs::home_dir() {
            if !canonical_path.starts_with(&home_dir) {
                // Allow system directories that are commonly used for Claude data
                let allowed_system_paths = ["/opt/claude",
                    "/usr/local/share/claude",
                    "/var/lib/claude"];
                
                let is_allowed = allowed_system_paths.iter()
                    .any(|allowed| canonical_path.starts_with(allowed));
                
                if !is_allowed {
                    return Err(anyhow!("Path outside of allowed directories: {}", canonical_path.display()));
                }
            }
        }
        
        Ok(canonical_path)
    }

    /// Scan all Claude data directories for JSONL files and parse usage data
    pub async fn scan_usage_files(&mut self) -> Result<()> {
        let mut all_entries = Vec::new();
        
        for data_path in &self.claude_data_paths {
            log::debug!("Scanning directory: {data_path:?}");
            
            // Find all .jsonl files recursively
            for entry in WalkDir::new(data_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
            {
                let file_path = entry.path();
                log::debug!("Parsing JSONL file: {file_path:?}");
                
                match self.parse_jsonl_file(file_path).await {
                    Ok(mut entries) => {
                        all_entries.append(&mut entries);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse JSONL file {file_path:?}: {e}");
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
        // Check file size before reading
        let metadata = fs::metadata(file_path).await?;
        if metadata.len() > MAX_FILE_SIZE as u64 {
            return Err(anyhow!("File too large: {} bytes (max {} bytes)", metadata.len(), MAX_FILE_SIZE));
        }
        
        let content = fs::read_to_string(file_path).await?;
        let mut entries = Vec::new();
        
        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            
            // Check line size before parsing
            if line.len() > MAX_JSON_SIZE {
                log::warn!("Skipping oversized JSON line {} in {:?}: {} bytes (max {} bytes)", 
                          line_num + 1, file_path, line.len(), MAX_JSON_SIZE);
                continue;
            }
            
            match self.parse_json_with_depth_limit(line) {
                Ok(json) => {
                    match self.parse_usage_entry(json) {
                        Ok(entry) => {
                            entries.push(entry);
                        }
                        Err(e) => {
                            // Only log debug for unexpected errors, skip normal skippable entries
                            let error_msg = e.to_string();
                            if error_msg.contains("No usage data") || error_msg.contains("Skipping summary") {
                                log::trace!("Skipping entry at line {} in {:?}: {}", line_num + 1, file_path, error_msg);
                            } else {
                                log::debug!("Failed to parse usage entry at line {} in {:?}: {}", line_num + 1, file_path, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::debug!("Skipping invalid JSON line {} in {:?}: {}", line_num + 1, file_path, e);
                }
            }
        }
        
        Ok(entries)
    }
    
    /// Parse JSON with depth limit to prevent stack overflow attacks
    fn parse_json_with_depth_limit(&self, json_str: &str) -> Result<serde_json::Value> {
        // Basic depth check by counting brackets
        let mut depth = 0;
        let mut max_depth = 0;
        
        for ch in json_str.chars() {
            match ch {
                '{' | '[' => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                    if max_depth > MAX_JSON_DEPTH {
                        return Err(anyhow!("JSON nesting too deep: {} levels (max {})", max_depth, MAX_JSON_DEPTH));
                    }
                }
                '}' | ']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        
        // Parse the JSON using serde_json
        serde_json::from_str(json_str)
            .map_err(|e| anyhow!("JSON parsing error: {}", e))
    }

    /// Parse a JSON value into a UsageEntry
    fn parse_usage_entry(&self, json: serde_json::Value) -> Result<UsageEntry> {
        // Skip summary entries and other non-message entries
        if let Some(entry_type) = json.get("type").and_then(|v| v.as_str()) {
            if entry_type == "summary" {
                return Err(anyhow!("Skipping summary entry"));
            }
        }

        // Extract timestamp
        let timestamp = if let Some(ts_str) = json.get("timestamp").and_then(|v| v.as_str()) {
            DateTime::parse_from_rfc3339(ts_str)?.with_timezone(&Utc)
        } else {
            return Err(anyhow!("Missing or invalid timestamp"));
        };

        // Extract usage information from Claude Code JSONL format
        // Usage data is nested inside message.usage for assistant responses
        let usage = if let Some(message) = json.get("message") {
            if let Some(usage_obj) = message.get("usage") {
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
                // Skip entries without usage data (user messages, etc.)
                return Err(anyhow!("No usage data in message"));
            }
        } else {
            // Try fallback for direct usage format (in case format changes)
            if let Some(usage_obj) = json.get("usage") {
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
            }
        };

        // Extract model from message.model for Claude Code format
        let model = json.get("message")
            .and_then(|m| m.get("model"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| json.get("model").and_then(|v| v.as_str()).map(|s| s.to_string()));

        // Extract message ID from message.id for Claude Code format
        let message_id = json.get("message")
            .and_then(|m| m.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| json.get("message_id").and_then(|v| v.as_str()).map(|s| s.to_string()));

        // Extract request ID from requestId field in Claude Code format
        let request_id = json.get("requestId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| json.get("request_id").and_then(|v| v.as_str()).map(|s| s.to_string()));

        Ok(UsageEntry {
            timestamp,
            usage,
            model,
            message_id,
            request_id,
        })
    }

    /// Derive session information from JSONL entries (passive observation)
    pub fn derive_current_session(&self) -> Option<TokenSession> {
        if self.usage_entries.is_empty() {
            return None;
        }
        
        let now = Utc::now();
        let session_duration = chrono::Duration::hours(5);
        
        // Find the most recent entry to determine the current session
        let latest_entry = self.usage_entries.last()?;
        
        // Calculate session start time based on 5-hour windows
        let session_start = latest_entry.timestamp;
        let reset_time = session_start + session_duration;
        
        // Check if we're still within the session window
        let is_active = now <= reset_time;
        
        // Calculate total tokens used in this session
        let total_tokens_used: u32 = self.usage_entries
            .iter()
            .filter(|entry| entry.timestamp >= session_start && entry.timestamp <= now)
            .map(|entry| entry.usage.total_tokens())
            .sum();
        
        // Determine plan type based on usage patterns (best guess from observed data)
        let plan_type = if total_tokens_used > 20_000 {
            PlanType::Max20
        } else if total_tokens_used > 10_000 || self.usage_entries.len() > 20 {
            PlanType::Pro
        } else {
            PlanType::Max5
        };
        
        // Generate a session ID based on the session start time (deterministic)
        let session_id = format!("observed-{}", session_start.timestamp());
        
        Some(TokenSession {
            id: session_id,
            start_time: session_start,
            end_time: if is_active { None } else { Some(reset_time) },
            plan_type: plan_type.clone(),
            tokens_used: total_tokens_used,
            tokens_limit: plan_type.default_limit(),
            is_active,
            reset_time,
        })
    }
    
    /// Calculate current usage metrics from observed data (passive monitoring)
    pub fn calculate_metrics(&self) -> Option<UsageMetrics> {
        let current_session = self.derive_current_session()?;
        let now = Utc::now();
        let session_start = current_session.start_time;
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
            let expected_rate = current_session.tokens_limit as f64 / session_duration_minutes;
            let actual_rate = if usage_rate > 0.0 { usage_rate } else { 0.1 };
            (expected_rate / actual_rate).min(1.0).max(0.0)
        } else {
            1.0
        };
        
        // Calculate projected depletion
        let projected_depletion = if usage_rate > 0.0 {
            let remaining_tokens = current_session.tokens_limit.saturating_sub(total_tokens_used);
            let minutes_remaining = remaining_tokens as f64 / usage_rate;
            Some(now + chrono::Duration::minutes(minutes_remaining as i64))
        } else {
            None
        };
        
        // Update session with actual token count
        let mut updated_session = current_session;
        updated_session.tokens_used = total_tokens_used;
        
        // Generate time-series data points from session entries
        let usage_history = self.generate_time_series_data(&session_entries, &session_start);
        
        // Calculate enhanced analytics
        let (cache_hit_rate, cache_creation_rate, input_output_ratio) = self.calculate_enhanced_analytics(&session_entries, &recent_entries, session_duration_minutes);
        
        Some(UsageMetrics {
            current_session: updated_session,
            usage_rate,
            session_progress,
            efficiency_score,
            projected_depletion,
            usage_history,
            
            // Enhanced analytics
            cache_hit_rate,
            cache_creation_rate,
            token_consumption_rate: usage_rate,
            input_output_ratio,
        })
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

    /// Generate time-series data points for chart display
    fn generate_time_series_data(&self, session_entries: &[&UsageEntry], session_start: &DateTime<Utc>) -> Vec<TokenUsagePoint> {
        if session_entries.is_empty() {
            return Vec::new();
        }
        
        let mut time_series = Vec::new();
        let mut cumulative_tokens = 0u32;
        
        // Sort entries by timestamp to ensure proper ordering
        let mut sorted_entries = session_entries.to_vec();
        sorted_entries.sort_by_key(|entry| entry.timestamp);
        
        // Add starting point at session start with 0 tokens
        time_series.push(TokenUsagePoint {
            timestamp: *session_start,
            tokens_used: 0,
            session_id: "current".to_string(),
        });
        
        // Process each usage entry to create cumulative data points
        for entry in sorted_entries {
            cumulative_tokens += entry.usage.total_tokens();
            time_series.push(TokenUsagePoint {
                timestamp: entry.timestamp,
                tokens_used: cumulative_tokens,
                session_id: "current".to_string(),
            });
        }
        
        // If we have multiple points, ensure reasonable spacing for visualization
        if time_series.len() > 100 {
            // Sample down to ~50 points for better performance
            let step = time_series.len() / 50;
            time_series = time_series
                .into_iter()
                .enumerate()
                .filter(|(i, _)| i % step == 0)
                .map(|(_, point)| point)
                .collect();
            
            // Always include the last point
            if let Some(last) = session_entries.last() {
                time_series.push(TokenUsagePoint {
                    timestamp: last.timestamp,
                    tokens_used: cumulative_tokens,
                    session_id: "current".to_string(),
                });
            }
        }
        
        time_series
    }
    
    /// Calculate enhanced analytics for cache metrics and token ratios
    fn calculate_enhanced_analytics(&self, session_entries: &[&UsageEntry], _recent_entries: &[&UsageEntry], session_duration_minutes: f64) -> (f64, f64, f64) {
        if session_entries.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        
        // Calculate cache hit rate across all session entries
        let mut total_input_tokens = 0u32;
        let mut total_cache_read_tokens = 0u32;
        let mut total_cache_creation_tokens = 0u32;
        let mut total_output_tokens = 0u32;
        
        for entry in session_entries {
            total_input_tokens += entry.usage.input_tokens;
            total_cache_read_tokens += entry.usage.cache_read_tokens();
            total_cache_creation_tokens += entry.usage.cache_creation_tokens();
            total_output_tokens += entry.usage.output_tokens;
        }
        
        // Cache hit rate: cache read tokens / (input tokens + cache creation tokens)
        let total_effective_input = total_input_tokens + total_cache_creation_tokens;
        let cache_hit_rate = if total_effective_input > 0 {
            total_cache_read_tokens as f64 / total_effective_input as f64
        } else {
            0.0
        };
        
        // Cache creation rate: cache creation tokens per minute
        let cache_creation_rate = if session_duration_minutes > 0.0 {
            total_cache_creation_tokens as f64 / session_duration_minutes
        } else {
            0.0
        };
        
        // Input/Output ratio: total input tokens / total output tokens
        let input_output_ratio = if total_output_tokens > 0 {
            (total_input_tokens + total_cache_creation_tokens) as f64 / total_output_tokens as f64
        } else {
            0.0
        };
        
        (cache_hit_rate, cache_creation_rate, input_output_ratio)
    }
    
    /// Get file sources analysis with token counts
    pub fn get_file_sources_analysis(&self) -> Vec<(String, usize, u32)> {
        // Group entries by file path (approximated from data patterns)
        let mut file_analysis = Vec::new();
        
        // Since we don't track specific file paths, we'll analyze by patterns
        // This is a reasonable approximation based on typical usage
        if !self.usage_entries.is_empty() {
            let total_tokens: u32 = self.usage_entries.iter().map(|e| e.usage.total_tokens()).sum();
            let total_entries = self.usage_entries.len();
            
            // Group by time periods to simulate different sessions/files
            let mut current_group_start = self.usage_entries[0].timestamp;
            let mut current_group_tokens = 0u32;
            let mut current_group_entries = 0usize;
            let mut group_index = 1;
            
            for entry in &self.usage_entries {
                let time_diff = entry.timestamp.signed_duration_since(current_group_start);
                
                // If more than 1 hour gap, consider it a new "file" or session
                if time_diff > chrono::Duration::hours(1) {
                    if current_group_entries > 0 {
                        file_analysis.push((
                            format!("session-{group_index}.jsonl"),
                            current_group_entries,
                            current_group_tokens
                        ));
                    }
                    current_group_start = entry.timestamp;
                    current_group_tokens = 0;
                    current_group_entries = 0;
                    group_index += 1;
                }
                
                current_group_tokens += entry.usage.total_tokens();
                current_group_entries += 1;
            }
            
            // Add the final group
            if current_group_entries > 0 {
                file_analysis.push((
                    format!("session-{group_index}.jsonl"),
                    current_group_entries,
                    current_group_tokens
                ));
            }
            
            // If we have no groups (continuous usage), create a single entry
            if file_analysis.is_empty() {
                file_analysis.push((
                    "current-session.jsonl".to_string(),
                    total_entries,
                    total_tokens
                ));
            }
        }
        
        file_analysis
    }

    /// Get model usage breakdown
    pub fn get_model_usage_breakdown(&self) -> Vec<(String, u32, usize)> {
        use std::collections::HashMap;
        
        let mut model_usage: HashMap<String, (u32, usize)> = HashMap::new();
        
        for entry in &self.usage_entries {
            let model = entry.model.clone().unwrap_or_else(|| "unknown".to_string());
            let tokens = entry.usage.total_tokens();
            
            let (total_tokens, count) = model_usage.entry(model).or_insert((0, 0));
            *total_tokens += tokens;
            *count += 1;
        }
        
        let mut result: Vec<(String, u32, usize)> = model_usage
            .into_iter()
            .map(|(model, (tokens, count))| (model, tokens, count))
            .collect();
        
        result.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by tokens descending
        result
    }

    /// Get token type breakdown
    pub fn get_token_type_breakdown(&self) -> (u32, u32, u32, u32) {
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;
        let mut cache_creation_tokens = 0u32;
        let mut cache_read_tokens = 0u32;
        
        for entry in &self.usage_entries {
            input_tokens += entry.usage.input_tokens;
            output_tokens += entry.usage.output_tokens;
            cache_creation_tokens += entry.usage.cache_creation_input_tokens.unwrap_or(0);
            cache_read_tokens += entry.usage.cache_read_input_tokens.unwrap_or(0);
        }
        
        (input_tokens, output_tokens, cache_creation_tokens, cache_read_tokens)
    }

    /// Get monitored paths
    pub fn get_monitored_paths(&self) -> &[PathBuf] {
        &self.claude_data_paths
    }

    /// Start file system watcher for real-time updates
    pub fn start_file_watcher(&mut self) -> Result<mpsc::Receiver<notify::Result<Event>>> {
        let (tx, rx) = mpsc::channel();
        
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        
        // Watch all Claude data directories
        for path in &self.claude_data_paths {
            watcher.watch(path, RecursiveMode::Recursive)?;
            log::info!("Watching directory for changes: {path:?}");
        }
        
        // Store watcher in the struct to manage its lifetime properly
        self._watcher = Some(Arc::new(Mutex::new(watcher)));
        
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