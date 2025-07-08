use super::SessionService;
use crate::models::*;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

/// Session tracking implementation
pub struct SessionTracker {
    sessions: HashMap<String, TokenSession>,
    data_path: PathBuf,
}

impl SessionTracker {
    pub fn new(data_path: PathBuf) -> Self {
        Self {
            sessions: HashMap::new(),
            data_path,
        }
    }

    pub async fn load_sessions(&mut self) -> Result<()> {
        if !self.data_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.data_path).await?;
        let sessions: Vec<TokenSession> = serde_json::from_str(&content)?;
        
        for session in sessions {
            self.sessions.insert(session.id.clone(), session);
        }
        
        Ok(())
    }

    pub async fn save_sessions(&self) -> Result<()> {
        let sessions: Vec<&TokenSession> = self.sessions.values().collect();
        let content = serde_json::to_string_pretty(&sessions)?;
        
        if let Some(parent) = self.data_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        fs::write(&self.data_path, content).await?;
        Ok(())
    }

    /// Clean up old expired sessions
    pub async fn cleanup_expired_sessions(&mut self) -> Result<()> {
        let now = Utc::now();
        let session_duration = Duration::hours(5);
        
        self.sessions.retain(|_, session| {
            if let Some(end_time) = session.end_time {
                now.signed_duration_since(end_time) < Duration::days(7)
            } else {
                now.signed_duration_since(session.start_time) < session_duration
            }
        });
        
        self.save_sessions().await?;
        Ok(())
    }
}

impl SessionService for SessionTracker {
    fn create_session(&mut self, plan_type: PlanType) -> impl std::future::Future<Output = Result<TokenSession>> + Send {
        let session_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let reset_time = now + Duration::hours(plan_type.session_duration_hours() as i64);
        
        let session = TokenSession {
            id: session_id.clone(),
            start_time: now,
            end_time: None,
            plan_type: plan_type.clone(),
            tokens_used: 0,
            tokens_limit: plan_type.default_limit(),
            is_active: true,
            reset_time,
        };
        
        self.sessions.insert(session_id.clone(), session.clone());
        
        async move {
            log::info!("Created new session: {} with plan {:?}", session_id, plan_type);
            Ok(session)
        }
    }

    fn update_session(&mut self, session_id: &str, tokens_used: u32) -> impl std::future::Future<Output = Result<()>> + Send {
        let session_id = session_id.to_string();
        let mut updated_session = None;
        
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.tokens_used = tokens_used;
            
            // Check if session should be ended due to time limit
            if Utc::now() > session.reset_time {
                session.is_active = false;
                session.end_time = Some(session.reset_time);
            }
            
            updated_session = Some(session.clone());
        }
        
        async move {
            if updated_session.is_some() {
                Ok(())
            } else {
                Err(anyhow!("Session not found: {}", session_id))
            }
        }
    }

    fn end_session(&mut self, session_id: &str) -> impl std::future::Future<Output = Result<()>> + Send {
        let session_id = session_id.to_string();
        let mut session_ended = false;
        
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.is_active = false;
            session.end_time = Some(Utc::now());
            session_ended = true;
        }
        
        async move {
            if session_ended {
                log::info!("Ended session: {}", session_id);
                Ok(())
            } else {
                Err(anyhow!("Session not found: {}", session_id))
            }
        }
    }

    fn get_active_session(&self) -> impl std::future::Future<Output = Result<Option<TokenSession>>> + Send {
        let active_session = self.sessions.values()
            .find(|session| session.is_active && Utc::now() <= session.reset_time)
            .cloned();
        
        async move {
            Ok(active_session)
        }
    }

    fn get_session_history(&self, limit: usize) -> impl std::future::Future<Output = Result<Vec<TokenSession>>> + Send {
        let mut sessions: Vec<TokenSession> = self.sessions.values().cloned().collect();
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions.truncate(limit);
        
        async move {
            Ok(sessions)
        }
    }
}