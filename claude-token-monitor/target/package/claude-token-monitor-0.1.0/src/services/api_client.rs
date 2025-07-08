use crate::models::{ApiConfig, ApiHeaders, ApiTokenUsage, ApiUsageResponse, ApiErrorResponse, CredentialManager, CredentialSource};
use anyhow::{anyhow, Result};
use reqwest::{Client, Response};
use std::time::Duration;
use tokio::time::sleep;

/// Claude API client for fetching token usage
#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    config: ApiConfig,
    headers: ApiHeaders,
}

impl ApiClient {
    /// Create new API client with configuration
    pub fn new(mut config: ApiConfig) -> Result<Self> {
        // Try to get API key using credential manager if not provided
        if config.api_key.is_empty() {
            config.api_key = CredentialManager::load_credentials(None)?;
        }

        let headers = ApiHeaders::new(&config.api_key);
        
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()?;

        Ok(Self {
            client,
            config,
            headers,
        })
    }

    /// Create API client from environment variables
    pub fn from_env() -> Result<Self> {
        let config = ApiConfig::default();
        Self::new(config)
    }

    /// Create API client with specific credential source
    pub fn with_credentials(credential_source: CredentialSource) -> Result<Self> {
        let api_key = CredentialManager::load_from_source(&credential_source)?;
        let mut config = ApiConfig::default();
        config.api_key = api_key;
        Self::new(config)
    }

    /// Create API client using Claude CLI credentials
    pub fn from_claude_cli() -> Result<Self> {
        let credential_source = CredentialSource::ClaudeCliFile;
        Self::with_credentials(credential_source)
    }

    /// Fetch current token usage from Claude API
    pub async fn fetch_token_usage(&self) -> Result<ApiTokenUsage> {
        let url = format!("{}/v1/usage", self.config.base_url);
        
        for attempt in 1..=self.config.retry_attempts {
            match self.try_fetch_usage(&url).await {
                Ok(usage) => return Ok(usage),
                Err(e) => {
                    log::warn!("API fetch attempt {}/{} failed: {}", attempt, self.config.retry_attempts, e);
                    
                    if attempt < self.config.retry_attempts {
                        let delay = Duration::from_secs(2_u64.pow(attempt - 1)); // Exponential backoff
                        sleep(delay).await;
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(anyhow!("All API fetch attempts failed"))
    }

    /// Single attempt to fetch usage data
    async fn try_fetch_usage(&self, url: &str) -> Result<ApiTokenUsage> {
        let response = self.client
            .get(url)
            .header("Authorization", &self.headers.authorization)
            .header("anthropic-version", &self.headers.anthropic_version)
            .header("Content-Type", &self.headers.content_type)
            .header("User-Agent", &self.headers.user_agent)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Handle API response and parse token usage data
    async fn handle_response(&self, response: Response) -> Result<ApiTokenUsage> {
        let status = response.status();
        let response_text = response.text().await?;

        if status.is_success() {
            // Parse the actual Claude API response format
            let usage_response: ApiUsageResponse = serde_json::from_str(&response_text)
                .map_err(|e| anyhow!("Failed to parse API response: {} - Response: {}", e, response_text))?;

            // Convert to our internal format
            let api_usage = ApiTokenUsage {
                current_usage: usage_response.tokens_used,
                daily_limit: usage_response.tokens_limit,
                session_limit: usage_response.tokens_limit,
                session_usage: usage_response.tokens_used,
                session_start: chrono::Utc::now() - chrono::Duration::hours(5), // Estimate
                session_reset: usage_response.reset_time,
                plan_name: usage_response.plan_type,
            };

            Ok(api_usage)
        } else {
            // Try to parse error response
            let error_response: Result<ApiErrorResponse, _> = serde_json::from_str(&response_text);
            
            match error_response {
                Ok(error) => Err(anyhow!("API Error {}: {} - {}", status, error.error, error.message)),
                Err(_) => Err(anyhow!("API Error {}: {}", status, response_text)),
            }
        }
    }

    /// Test API connection
    pub async fn test_connection(&self) -> Result<bool> {
        match self.fetch_token_usage().await {
            Ok(_) => {
                log::info!("API connection test successful");
                Ok(true)
            }
            Err(e) => {
                log::error!("API connection test failed: {}", e);
                Ok(false)
            }
        }
    }

    /// Get API configuration (without sensitive data)
    pub fn get_config_info(&self) -> String {
        let key_length = self.config.api_key.len();
        let key_info = if key_length > 0 {
            format!("Yes ({} chars)", key_length)
        } else {
            "No".to_string()
        };

        format!(
            "Base URL: {}, Timeout: {}s, Retry Attempts: {}, Has API Key: {}",
            self.config.base_url,
            self.config.timeout_seconds,
            self.config.retry_attempts,
            key_info
        )
    }

    /// Check available credential sources
    pub fn check_credential_sources() -> Vec<String> {
        let sources = CredentialManager::get_available_sources();
        sources.into_iter().map(|(source, available)| {
            let status = if available { "✅" } else { "❌" };
            match source {
                CredentialSource::ClaudeCliFile => format!("{} Claude CLI (~/.claude/.credentials.json)", status),
                CredentialSource::Environment(var) => format!("{} Environment Variable ({})", status, var),
                CredentialSource::Direct(_) => format!("{} Direct API Key", status),
                CredentialSource::CustomFile(path) => format!("{} Custom File ({})", status, path.display()),
            }
        }).collect()
    }
}

/// Mock API client for testing and development
pub struct MockApiClient {
    pub simulated_usage: u32,
    pub simulated_limit: u32,
    pub should_fail: bool,
}

impl MockApiClient {
    pub fn new() -> Self {
        Self {
            simulated_usage: 1500,
            simulated_limit: 40000,
            should_fail: false,
        }
    }

    pub fn with_usage(mut self, usage: u32, limit: u32) -> Self {
        self.simulated_usage = usage;
        self.simulated_limit = limit;
        self
    }

    pub fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }

    pub async fn fetch_token_usage(&mut self) -> Result<ApiTokenUsage> {
        if self.should_fail {
            return Err(anyhow!("Simulated API failure"));
        }

        // Simulate increasing usage over time
        self.simulated_usage += rand::random::<u32>() % 50;

        Ok(ApiTokenUsage {
            current_usage: self.simulated_usage,
            daily_limit: self.simulated_limit,
            session_limit: self.simulated_limit,
            session_usage: self.simulated_usage,
            session_start: chrono::Utc::now() - chrono::Duration::hours(2),
            session_reset: chrono::Utc::now() + chrono::Duration::hours(3),
            plan_name: "Pro".to_string(),
        })
    }
}

// Re-export for convenience
pub use MockApiClient as TestApiClient;