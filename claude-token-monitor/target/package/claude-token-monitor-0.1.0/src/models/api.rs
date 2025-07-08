use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Claude API usage response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUsageResponse {
    pub tokens_used: u32,
    pub tokens_limit: u32,
    pub reset_time: DateTime<Utc>,
    pub session_id: Option<String>,
    pub plan_type: String,
}

/// Claude API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    pub error: String,
    pub message: String,
    pub code: Option<u32>,
}

/// API client configuration
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            timeout_seconds: 30,
            retry_attempts: 3,
        }
    }
}

/// Token usage data point from API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTokenUsage {
    pub current_usage: u32,
    pub daily_limit: u32,
    pub session_limit: u32,
    pub session_usage: u32,
    pub session_start: DateTime<Utc>,
    pub session_reset: DateTime<Utc>,
    pub plan_name: String,
}

/// API request headers
#[derive(Debug, Clone)]
pub struct ApiHeaders {
    pub authorization: String,
    pub anthropic_version: String,
    pub content_type: String,
    pub user_agent: String,
}

impl ApiHeaders {
    pub fn new(api_key: &str) -> Self {
        Self {
            authorization: format!("Bearer {}", api_key),
            anthropic_version: "2023-06-01".to_string(),
            content_type: "application/json".to_string(),
            user_agent: "claude-token-monitor-rust/0.1.0".to_string(),
        }
    }
}