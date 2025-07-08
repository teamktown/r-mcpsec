use crate::models::{CredentialManager, CredentialSource};
use anyhow::Result;

/// Check authentication status and available credentials
pub async fn check_auth_status() -> Result<()> {
    println!("🔐 Claude Token Monitor - Authentication Status");
    println!("===============================================");
    println!();

    // Check Claude CLI credentials
    println!("📋 Claude CLI Credentials (~/.claude/.credentials.json):");
    match CredentialManager::check_claude_cli_credentials() {
        Ok(credentials) => {
            println!("✅ Status: Available and valid");
            println!("📊 Details: {}", credentials.get_info_for_logging());
            
            if credentials.is_expired() {
                println!("⚠️ Warning: Token may be expired");
                println!("💡 Tip: Run 'claude auth login' to refresh");
            }
        }
        Err(e) => {
            println!("❌ Status: Not available");
            println!("📝 Error: {}", e);
            println!("💡 Tip: Run 'claude auth login' to set up Claude CLI authentication");
        }
    }
    println!();

    // Check environment variables
    println!("🌍 Environment Variables:");
    for var_name in ["CLAUDE_API_KEY", "ANTHROPIC_API_KEY"] {
        match std::env::var(var_name) {
            Ok(value) => {
                println!("✅ {}: Set ({} characters)", var_name, value.len());
            }
            Err(_) => {
                println!("❌ {}: Not set", var_name);
            }
        }
    }
    println!();

    // Test credential loading
    println!("🔄 Testing Credential Loading:");
    match CredentialManager::load_credentials(None) {
        Ok(_) => {
            println!("✅ Successfully loaded credentials using automatic detection");
            println!("🚀 Ready to use Claude Token Monitor!");
        }
        Err(e) => {
            println!("❌ Failed to load credentials: {}", e);
            println!();
            println!("📝 Setup Instructions:");
            println!("1. Install Claude CLI: npm install -g @anthropic-ai/claude-cli");
            println!("2. Login with OAuth: claude auth login");
            println!("3. Or set environment variable: export CLAUDE_API_KEY=your_key_here");
        }
    }

    Ok(())
}

/// Display help for authentication setup
pub fn show_auth_help() {
    println!("🔐 Claude Token Monitor - Authentication Help");
    println!("=============================================");
    println!();
    println!("The Claude Token Monitor supports multiple authentication methods:");
    println!();
    println!("1️⃣ Claude CLI OAuth (Recommended):");
    println!("   • Install: npm install -g @anthropic-ai/claude-cli");
    println!("   • Login: claude auth login");
    println!("   • Credentials stored in: ~/.claude/.credentials.json");
    println!();
    println!("2️⃣ Environment Variables:");
    println!("   • CLAUDE_API_KEY=your_key_here");
    println!("   • ANTHROPIC_API_KEY=your_key_here");
    println!();
    println!("3️⃣ Command Line Flag:");
    println!("   • cargo run -- --api-key your_key_here monitor");
    println!();
    println!("🔍 Priority Order:");
    println!("   1. --api-key flag (highest priority)");
    println!("   2. ~/.claude/.credentials.json (OAuth)");
    println!("   3. CLAUDE_API_KEY environment variable");
    println!("   4. ANTHROPIC_API_KEY environment variable");
    println!();
    println!("💡 To check your current authentication status:");
    println!("   cargo run -- auth status");
}

/// Validate credentials and show detailed information
pub async fn validate_credentials() -> Result<()> {
    println!("🔍 Validating Claude Credentials...");
    println!();

    // Try to load and validate credentials
    let credential_source = match CredentialManager::check_claude_cli_credentials() {
        Ok(credentials) => {
            println!("✅ Found Claude CLI credentials");
            credentials.validate()?;
            CredentialSource::ClaudeCliFile
        }
        Err(_) => {
            println!("⚠️ Claude CLI credentials not available, trying environment variables...");
            
            if std::env::var("CLAUDE_API_KEY").is_ok() {
                CredentialSource::Environment("CLAUDE_API_KEY".to_string())
            } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                CredentialSource::Environment("ANTHROPIC_API_KEY".to_string())
            } else {
                return Err(anyhow::anyhow!("No valid credentials found"));
            }
        }
    };

    // Test API connection
    println!("🔗 Testing API connection...");
    let client = crate::services::api_client::ApiClient::with_credentials(credential_source)?;
    
    if client.test_connection().await? {
        println!("✅ API connection successful!");
        println!("📊 Configuration: {}", client.get_config_info());
    } else {
        println!("❌ API connection failed");
        return Err(anyhow::anyhow!("Failed to connect to Claude API"));
    }

    Ok(())
}