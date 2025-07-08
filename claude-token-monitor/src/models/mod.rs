use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a Claude AI usage session with token tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSession {
    pub id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub plan_type: PlanType,
    pub tokens_used: u32,
    pub tokens_limit: u32,
    pub is_active: bool,
    pub reset_time: DateTime<Utc>,
}

/// Claude AI plan types with their respective limits
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlanType {
    Pro,
    Max5,
    Max20,
    Custom(u32),
}

impl PlanType {
    pub fn default_limit(&self) -> u32 {
        match self {
            PlanType::Pro => 40_000,
            PlanType::Max5 => 20_000,
            PlanType::Max20 => 100_000,
            PlanType::Custom(limit) => *limit,
        }
    }

    pub fn session_duration_hours(&self) -> u32 {
        5 // All plans use 5-hour sessions
    }
}

/// Real-time usage metrics and predictions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageMetrics {
    pub current_session: TokenSession,
    pub usage_rate: f64, // tokens per minute
    pub projected_depletion: Option<DateTime<Utc>>,
    pub efficiency_score: f64,
    pub session_progress: f64, // percentage of session time elapsed
    pub usage_history: Vec<TokenUsagePoint>,
}

/// Point-in-time token usage data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsagePoint {
    pub timestamp: DateTime<Utc>,
    pub tokens_used: u32,
    pub session_id: String,
}

/// User configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub default_plan: PlanType,
    pub timezone: String,
    pub update_interval_seconds: u64,
    pub warning_threshold: f64, // percentage at which to warn
    pub auto_switch_plans: bool,
    pub color_scheme: ColorScheme,
    pub custom_limits: HashMap<String, u32>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            default_plan: PlanType::Pro,
            timezone: "UTC".to_string(),
            update_interval_seconds: 3,
            warning_threshold: 0.85,
            auto_switch_plans: true,
            color_scheme: ColorScheme::default(),
            custom_limits: HashMap::new(),
        }
    }
}

/// Color scheme for terminal UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub progress_bar_full: String,
    pub progress_bar_empty: String,
    pub warning_color: String,
    pub success_color: String,
    pub error_color: String,
    pub info_color: String,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            progress_bar_full: "green".to_string(),
            progress_bar_empty: "gray".to_string(),
            warning_color: "yellow".to_string(),
            success_color: "green".to_string(),
            error_color: "red".to_string(),
            info_color: "blue".to_string(),
        }
    }
}

/// Application state and runtime data
#[derive(Debug, Clone)]
pub struct AppState {
    pub config: UserConfig,
    pub current_metrics: Option<UsageMetrics>,
    pub is_monitoring: bool,
    pub last_update: DateTime<Utc>,
    pub session_history: Vec<TokenSession>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            config: UserConfig::default(),
            current_metrics: None,
            is_monitoring: false,
            last_update: Utc::now(),
            session_history: Vec::new(),
        }
    }
}