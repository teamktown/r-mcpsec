use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Claude OAuth credentials structure matching ~/.claude/.credentials.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCredentials {
    /// OAuth access token
    pub access_token: String,
    
    /// OAuth refresh token  
    pub refresh_token: Option<String>,
    
    /// Token expiry timestamp
    pub expires_at: Option<i64>,
    
    /// OAuth scope
    pub scope: Option<String>,
    
    /// Token type (usually "Bearer")
    pub token_type: Option<String>,
    
    /// User ID
    pub user_id: Option<String>,
    
    /// Organization ID
    pub organization_id: Option<String>,
}

/// Extended credentials structure with additional fields that might be present
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtendedClaudeCredentials {
    #[serde(flatten)]
    pub credentials: ClaudeCredentials,
    
    /// API base URL
    pub api_url: Option<String>,
    
    /// Client ID for OAuth
    pub client_id: Option<String>,
    
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

impl ClaudeCredentials {
    /// Load credentials from the default Claude CLI location
    pub fn load_from_default_path() -> Result<Self> {
        let credentials_path = Self::get_default_credentials_path()?;
        Self::load_from_path(&credentials_path)
    }

    /// Load credentials from a specific path
    pub fn load_from_path(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Err(anyhow!(
                "Credentials file not found at {}. Please run 'claude auth login' first.",
                path.display()
            ));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read credentials file: {}", e))?;

        // Try to parse as extended credentials first, then fall back to basic
        let credentials = serde_json::from_str::<ExtendedClaudeCredentials>(&content)
            .map(|ext| ext.credentials)
            .or_else(|_| serde_json::from_str::<ClaudeCredentials>(&content))
            .map_err(|e| anyhow!("Failed to parse credentials file: {}", e))?;

        // Validate that we have the essential access token
        if credentials.access_token.is_empty() {
            return Err(anyhow!("Invalid credentials: access_token is empty"));
        }

        log::info!("Successfully loaded Claude credentials from {}", path.display());
        Ok(credentials)
    }

    /// Get the default credentials file path (~/.claude/.credentials.json)
    pub fn get_default_credentials_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Unable to determine home directory"))?;
        
        Ok(home_dir.join(".claude").join(".credentials.json"))
    }

    /// Check if the token is expired (if expiry information is available)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = chrono::Utc::now().timestamp();
            return now >= expires_at;
        }
        false // If no expiry info, assume it's valid
    }

    /// Get the authorization header value
    pub fn get_auth_header(&self) -> String {
        let token_type = self.token_type.as_deref().unwrap_or("Bearer");
        format!("{} {}", token_type, self.access_token)
    }

    /// Validate credentials and check for common issues
    pub fn validate(&self) -> Result<()> {
        if self.access_token.is_empty() {
            return Err(anyhow!("Access token is empty"));
        }

        if self.access_token.len() < 10 {
            return Err(anyhow!("Access token appears to be invalid (too short)"));
        }

        if self.is_expired() {
            return Err(anyhow!("Access token has expired. Please run 'claude auth login' to refresh."));
        }

        log::debug!("Credentials validation passed");
        Ok(())
    }

    /// Get a sanitized version for logging (without sensitive data)
    pub fn get_info_for_logging(&self) -> String {
        format!(
            "Token Type: {}, User ID: {}, Org ID: {}, Expires: {}, Token Length: {} chars",
            self.token_type.as_deref().unwrap_or("Bearer"),
            self.user_id.as_deref().unwrap_or("unknown"),
            self.organization_id.as_deref().unwrap_or("unknown"),
            self.expires_at.map_or("never".to_string(), |exp| {
                chrono::DateTime::from_timestamp(exp, 0)
                    .map_or("invalid".to_string(), |dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            }),
            self.access_token.len()
        )
    }
}

/// Credential loading strategy
#[derive(Debug, Clone)]
pub enum CredentialSource {
    /// Load from ~/.claude/.credentials.json
    ClaudeCliFile,
    /// Load from environment variable
    Environment(String),
    /// Use provided string directly
    Direct(String),
    /// Load from custom file path
    CustomFile(PathBuf),
}

/// Credential manager for different sources
pub struct CredentialManager;

impl CredentialManager {
    /// Load credentials using the best available method
    pub fn load_credentials(preferred_source: Option<CredentialSource>) -> Result<String> {
        // Try preferred source first
        if let Some(source) = preferred_source {
            if let Ok(token) = Self::load_from_source(&source) {
                return Ok(token);
            }
        }

        // Fallback strategy: try in order of preference
        let fallback_sources = vec![
            CredentialSource::ClaudeCliFile,
            CredentialSource::Environment("CLAUDE_API_KEY".to_string()),
            CredentialSource::Environment("ANTHROPIC_API_KEY".to_string()),
        ];

        for source in fallback_sources {
            if let Ok(token) = Self::load_from_source(&source) {
                log::info!("Successfully loaded credentials from {:?}", source);
                return Ok(token);
            }
        }

        Err(anyhow!(
            "No valid credentials found. Please either:\n\
            1. Run 'claude auth login' to set up OAuth credentials\n\
            2. Set CLAUDE_API_KEY or ANTHROPIC_API_KEY environment variable\n\
            3. Use --api-key flag with your API key"
        ))
    }

    /// Load credentials from a specific source
    pub fn load_from_source(source: &CredentialSource) -> Result<String> {
        match source {
            CredentialSource::ClaudeCliFile => {
                let credentials = ClaudeCredentials::load_from_default_path()?;
                credentials.validate()?;
                Ok(credentials.access_token)
            }
            CredentialSource::Environment(var_name) => {
                std::env::var(var_name)
                    .map_err(|_| anyhow!("Environment variable {} not found", var_name))
            }
            CredentialSource::Direct(token) => {
                if token.is_empty() {
                    Err(anyhow!("Direct token is empty"))
                } else {
                    Ok(token.clone())
                }
            }
            CredentialSource::CustomFile(path) => {
                let credentials = ClaudeCredentials::load_from_path(path)?;
                credentials.validate()?;
                Ok(credentials.access_token)
            }
        }
    }

    /// Get information about available credential sources
    pub fn get_available_sources() -> Vec<(CredentialSource, bool)> {
        let sources = vec![
            (CredentialSource::ClaudeCliFile, ClaudeCredentials::get_default_credentials_path().map_or(false, |p| p.exists())),
            (CredentialSource::Environment("CLAUDE_API_KEY".to_string()), std::env::var("CLAUDE_API_KEY").is_ok()),
            (CredentialSource::Environment("ANTHROPIC_API_KEY".to_string()), std::env::var("ANTHROPIC_API_KEY").is_ok()),
        ];
        
        sources
    }

    /// Check if Claude CLI credentials are available and valid
    pub fn check_claude_cli_credentials() -> Result<ClaudeCredentials> {
        let credentials = ClaudeCredentials::load_from_default_path()?;
        credentials.validate()?;
        Ok(credentials)
    }
}