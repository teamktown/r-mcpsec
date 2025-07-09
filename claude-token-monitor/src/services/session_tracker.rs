use super::SessionService;
use crate::models::*;
use crate::services::file_monitor::FileBasedTokenMonitor;
use anyhow::Result;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Session observation implementation (passive monitoring only)
pub struct SessionTracker {
    observed_sessions: HashMap<String, TokenSession>,
    data_path: PathBuf,
    file_monitor: FileBasedTokenMonitor,
}

impl SessionTracker {
    pub fn new(data_path: PathBuf) -> Result<Self> {
        let file_monitor = FileBasedTokenMonitor::new()?;
        Ok(Self {
            observed_sessions: HashMap::new(),
            data_path,
            file_monitor,
        })
    }

    /// Update observed sessions from JSONL file data
    pub async fn update_observed_sessions(&mut self) -> Result<()> {
        // Scan for new usage data
        self.file_monitor.scan_usage_files().await?;
        
        // Derive current session from observed data
        if let Some(current_session) = self.file_monitor.derive_current_session() {
            self.observed_sessions.insert(current_session.id.clone(), current_session);
        }
        
        // Save observed sessions for historical tracking
        self.save_observed_sessions().await?;
        
        Ok(())
    }

    pub async fn save_observed_sessions(&self) -> Result<()> {
        let sessions: Vec<&TokenSession> = self.observed_sessions.values().collect();
        let content = serde_json::to_string_pretty(&sessions)?;
        
        if let Some(parent) = self.data_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::write(&self.data_path, content).await?;
        Ok(())
    }

    /// Clean up old observed sessions
    pub async fn cleanup_expired_sessions(&mut self) -> Result<()> {
        let now = Utc::now();
        let session_duration = Duration::hours(5);
        
        self.observed_sessions.retain(|_, session| {
            if let Some(end_time) = session.end_time {
                now.signed_duration_since(end_time) < Duration::days(7)
            } else {
                now.signed_duration_since(session.start_time) < session_duration
            }
        });
        
        self.save_observed_sessions().await?;
        Ok(())
    }
}

impl SessionService for SessionTracker {
    fn get_active_session(&self) -> impl std::future::Future<Output = Result<Option<TokenSession>>> + Send {
        let active_session = self.observed_sessions.values()
            .find(|session| session.is_active && Utc::now() <= session.reset_time)
            .cloned();
        
        async move {
            Ok(active_session)
        }
    }

    fn get_session_history(&self, limit: usize) -> impl std::future::Future<Output = Result<Vec<TokenSession>>> + Send {
        let mut sessions: Vec<TokenSession> = self.observed_sessions.values().cloned().collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions.truncate(limit);
        
        async move {
            Ok(sessions)
        }
    }
}