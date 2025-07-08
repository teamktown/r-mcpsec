pub mod session_tracker;
pub mod token_monitor;
pub mod api_client;

use crate::models::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use async_trait::async_trait;

/// Core service trait for token monitoring
#[async_trait]
pub trait TokenMonitorService {
    async fn start_monitoring(&mut self) -> Result<()>;
    async fn stop_monitoring(&mut self) -> Result<()>;
    async fn get_current_usage(&self) -> Result<UsageMetrics>;
    async fn update_usage(&mut self) -> Result<()>;
}

/// Service for managing user configuration
pub trait ConfigService {
    fn load_config(&self) -> Result<UserConfig>;
    fn save_config(&self, config: &UserConfig) -> Result<()>;
    fn get_config_path(&self) -> Result<std::path::PathBuf>;
}

/// Service for session tracking and management
#[async_trait]
pub trait SessionService {
    async fn create_session(&mut self, plan_type: PlanType) -> Result<TokenSession>;
    async fn update_session(&mut self, session_id: &str, tokens_used: u32) -> Result<()>;
    async fn end_session(&mut self, session_id: &str) -> Result<()>;
    async fn get_active_session(&self) -> Result<Option<TokenSession>>;
    async fn get_session_history(&self, limit: usize) -> Result<Vec<TokenSession>>;
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