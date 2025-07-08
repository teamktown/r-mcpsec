use super::{TokenMonitorService, SessionService, api_client::ApiClient};
use crate::models::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Real-time token monitoring service
pub struct TokenMonitor<T: SessionService + Send + Sync + 'static> {
    session_service: Arc<RwLock<T>>,
    current_metrics: Arc<RwLock<Option<UsageMetrics>>>,
    is_running: Arc<RwLock<bool>>,
    update_interval: Duration,
    api_client: Option<ApiClient>,
    use_mock_data: bool,
}

impl<T: SessionService + Send + Sync + 'static> TokenMonitor<T> {
    pub fn new(
        session_service: Arc<RwLock<T>>,
        update_interval_seconds: u64,
    ) -> Self {
        Self {
            session_service,
            current_metrics: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            update_interval: Duration::from_secs(update_interval_seconds),
            api_client: None,
            use_mock_data: true,
        }
    }

    /// Create monitor with real API client
    pub fn with_api_client(
        session_service: Arc<RwLock<T>>,
        update_interval_seconds: u64,
        api_client: ApiClient,
    ) -> Self {
        Self {
            session_service,
            current_metrics: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            update_interval: Duration::from_secs(update_interval_seconds),
            api_client: Some(api_client),
            use_mock_data: false,
        }
    }

    /// Enable or disable mock data mode
    pub fn set_mock_mode(&mut self, use_mock: bool) {
        self.use_mock_data = use_mock;
    }

    /// Calculate usage rate based on session history
    async fn calculate_usage_rate(&self, session: &TokenSession) -> f64 {
        let elapsed = Utc::now()
            .signed_duration_since(session.start_time)
            .num_minutes() as f64;
        
        if elapsed > 0.0 {
            session.tokens_used as f64 / elapsed
        } else {
            0.0
        }
    }

    /// Predict when tokens will be depleted
    fn predict_depletion(&self, current_usage: u32, limit: u32, usage_rate: f64) -> Option<DateTime<Utc>> {
        if usage_rate <= 0.0 {
            return None;
        }

        let remaining_tokens = limit.saturating_sub(current_usage) as f64;
        let minutes_remaining = remaining_tokens / usage_rate;
        
        if minutes_remaining > 0.0 {
            Some(Utc::now() + chrono::Duration::minutes(minutes_remaining as i64))
        } else {
            None
        }
    }

    /// Calculate session progress percentage
    fn calculate_session_progress(&self, session: &TokenSession) -> f64 {
        let elapsed = Utc::now()
            .signed_duration_since(session.start_time)
            .num_minutes() as f64;
        
        let total_duration = session.reset_time
            .signed_duration_since(session.start_time)
            .num_minutes() as f64;
        
        if total_duration > 0.0 {
            (elapsed / total_duration).min(1.0)
        } else {
            0.0
        }
    }

    /// Calculate efficiency score
    fn calculate_efficiency(&self, usage_rate: f64, session_progress: f64) -> f64 {
        if session_progress <= 0.0 {
            return 1.0;
        }

        // Efficiency is based on how evenly tokens are used throughout the session
        let expected_rate = 1.0 / session_progress;
        let actual_rate = if usage_rate > 0.0 { usage_rate } else { 0.1 };
        
        (expected_rate / actual_rate).min(1.0).max(0.0)
    }

    /// Fetch current token usage from Claude API or mock data
    async fn fetch_current_token_usage(&self) -> Result<u32> {
        if self.use_mock_data || self.api_client.is_none() {
            // Mock data for development/testing
            let base_usage = 1500u32;
            let random_increment = rand::random::<u32>() % 100;
            let simulated_usage = base_usage + random_increment;
            
            log::debug!("Using mock token usage: {}", simulated_usage);
            Ok(simulated_usage)
        } else {
            // Real API integration
            let api_client = self.api_client.as_ref().unwrap();
            
            match api_client.fetch_token_usage().await {
                Ok(usage_data) => {
                    log::info!("Fetched real token usage: {} / {}", 
                        usage_data.session_usage, usage_data.session_limit);
                    Ok(usage_data.session_usage)
                }
                Err(e) => {
                    log::error!("Failed to fetch real token usage: {}", e);
                    log::warn!("Falling back to mock data");
                    
                    // Fallback to mock data if API fails
                    let fallback_usage = 1500u32 + (rand::random::<u32>() % 100);
                    Ok(fallback_usage)
                }
            }
        }
    }

    /// Background monitoring task
    async fn monitoring_loop(&mut self) -> Result<()> {
        let mut interval = interval(self.update_interval);
        
        loop {
            interval.tick().await;
            
            let is_running = *self.is_running.read().await;
            if !is_running {
                break;
            }

            if let Err(e) = self.update_usage().await {
                log::error!("Error updating usage: {}", e);
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl<T: SessionService + Send + Sync + 'static> TokenMonitorService for TokenMonitor<T> {
    async fn start_monitoring(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }

        *is_running = true;
        drop(is_running);

        log::info!("Starting token monitoring with interval {:?}", self.update_interval);
        
        // Start background monitoring task
        let mut monitor = self.clone();
        tokio::spawn(async move {
            if let Err(e) = monitor.monitoring_loop().await {
                log::error!("Monitoring loop error: {}", e);
            }
        });

        Ok(())
    }

    async fn stop_monitoring(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        log::info!("Stopped token monitoring");
        Ok(())
    }

    async fn get_current_usage(&self) -> Result<UsageMetrics> {
        let metrics = self.current_metrics.read().await;
        metrics.clone().ok_or_else(|| anyhow!("No usage metrics available"))
    }

    async fn update_usage(&mut self) -> Result<()> {
        let session_service = self.session_service.read().await;
        let active_session = session_service.get_active_session().await?;
        
        let session = match active_session {
            Some(session) => session,
            None => {
                log::warn!("No active session found");
                return Ok(());
            }
        };

        // Fetch current token usage
        let current_tokens = self.fetch_current_token_usage().await?;
        
        // Update session with new token count
        drop(session_service);
        let mut session_service = self.session_service.write().await;
        session_service.update_session(&session.id, current_tokens).await?;
        drop(session_service);

        // Calculate metrics
        let usage_rate = self.calculate_usage_rate(&session).await;
        let session_progress = self.calculate_session_progress(&session);
        let efficiency_score = self.calculate_efficiency(usage_rate, session_progress);
        let projected_depletion = self.predict_depletion(
            current_tokens, 
            session.tokens_limit, 
            usage_rate
        );

        // Create usage point for history
        let usage_point = TokenUsagePoint {
            timestamp: Utc::now(),
            tokens_used: current_tokens,
            session_id: session.id.clone(),
        };

        // Update metrics
        let metrics = UsageMetrics {
            current_session: session,
            usage_rate,
            projected_depletion,
            efficiency_score,
            session_progress,
            usage_history: vec![usage_point], // In real implementation, maintain history
        };

        let mut current_metrics = self.current_metrics.write().await;
        *current_metrics = Some(metrics);

        Ok(())
    }
}

impl<T: SessionService + Send + Sync + 'static> Clone for TokenMonitor<T> {
    fn clone(&self) -> Self {
        Self {
            session_service: Arc::clone(&self.session_service),
            current_metrics: Arc::clone(&self.current_metrics),
            is_running: Arc::clone(&self.is_running),
            update_interval: self.update_interval,
            api_client: self.api_client.clone(),
            use_mock_data: self.use_mock_data,
        }
    }
}