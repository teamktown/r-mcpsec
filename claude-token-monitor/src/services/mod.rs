pub mod session_tracker;
pub mod token_monitor;
pub mod file_monitor;

use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Core service trait for token monitoring
pub trait TokenMonitorService {
    fn start_monitoring(&mut self) -> Result<()>;
    fn stop_monitoring(&mut self) -> Result<()>;
    fn get_current_usage(&self) -> Result<UsageMetrics>;
    fn update_usage(&mut self) -> Result<()>;
}

/// Service for managing user configuration
pub trait ConfigService {
    fn load_config(&self) -> Result<UserConfig>;
    fn save_config(&self, config: &UserConfig) -> Result<()>;
    fn get_config_path(&self) -> Result<std::path::PathBuf>;
}

/// Service for session observation (passive monitoring only)
pub trait SessionService: Send + Sync {
    fn get_active_session(&self) -> impl std::future::Future<Output = Result<Option<TokenSession>>> + Send;
    fn get_session_history(&self, limit: usize) -> impl std::future::Future<Output = Result<Vec<TokenSession>>> + Send;
}

/// Service for analytics and predictions
pub trait AnalyticsService {
    fn calculate_usage_rate(&self, history: &[TokenUsagePoint]) -> f64;
    fn predict_depletion(&self, current_usage: u32, limit: u32, usage_rate: f64) -> Option<DateTime<Utc>>;
    fn calculate_efficiency(&self, usage_rate: f64, session_progress: f64) -> f64;
    fn analyze_usage_patterns(&self, sessions: &[TokenSession]) -> Result<UsageAnalysis>;
}

/// Usage pattern analysis results
#[derive(Debug, Clone)]
pub struct UsageAnalysis {
    pub average_session_duration: f64,
    pub peak_usage_times: Vec<(u32, u32)>, // (hour, usage)
    pub efficiency_trend: f64,
    pub recommended_plan: PlanType,
}