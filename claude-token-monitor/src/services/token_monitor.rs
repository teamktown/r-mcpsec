use super::{TokenMonitorService, SessionService};
use crate::models::*;
use crate::services::file_monitor::FileBasedTokenMonitor;
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

/// Real-time token monitoring service using file-based approach
pub struct TokenMonitor<T: SessionService + Send + Sync + 'static> {
    session_service: Arc<RwLock<T>>,
    file_monitor: Arc<RwLock<FileBasedTokenMonitor>>,
    current_metrics: Arc<RwLock<Option<UsageMetrics>>>,
    is_running: Arc<RwLock<bool>>,
    update_interval: Duration,
    use_mock_data: bool,
}

impl<T: SessionService + Send + Sync + 'static> TokenMonitor<T> {
    pub fn new(
        session_service: Arc<RwLock<T>>,
        update_interval_seconds: u64,
    ) -> Result<Self> {
        let file_monitor = FileBasedTokenMonitor::new()?;
        
        Ok(Self {
            session_service,
            file_monitor: Arc::new(RwLock::new(file_monitor)),
            current_metrics: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            update_interval: Duration::from_secs(update_interval_seconds),
            use_mock_data: false,
        })
    }

    /// Create monitor with mock data for testing
    pub fn with_mock_data(
        session_service: Arc<RwLock<T>>,
        update_interval_seconds: u64,
    ) -> Result<Self> {
        let file_monitor = FileBasedTokenMonitor::new()?;
        
        Ok(Self {
            session_service,
            file_monitor: Arc::new(RwLock::new(file_monitor)),
            current_metrics: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
            update_interval: Duration::from_secs(update_interval_seconds),
            use_mock_data: true,
        })
    }

    /// Enable or disable mock data mode
    pub fn set_mock_mode(&mut self, use_mock: bool) {
        self.use_mock_data = use_mock;
    }

    /// Generate mock metrics for testing
    async fn generate_mock_metrics(&self) -> Result<UsageMetrics> {
        // Create a mock session
        let base_usage = 1500u32 + (rand::random::<u32>() % 500);
        let limit = 40000u32; // Pro plan limit
        
        let session = TokenSession {
            id: "mock-session".to_string(),
            start_time: Utc::now() - chrono::Duration::hours(2),
            end_time: None,
            plan_type: PlanType::Pro,
            tokens_used: base_usage,
            tokens_limit: limit,
            is_active: true,
            reset_time: Utc::now() + chrono::Duration::hours(3),
        };

        let elapsed_minutes = 120.0; // 2 hours
        let usage_rate = base_usage as f64 / elapsed_minutes;
        let session_progress = elapsed_minutes / (5.0 * 60.0); // 5 hour sessions
        let efficiency_score = 0.75 + (rand::random::<f64>() * 0.25); // 0.75 to 1.0
        
        let projected_depletion = if usage_rate > 0.0 {
            let remaining_tokens = limit.saturating_sub(base_usage);
            let minutes_remaining = remaining_tokens as f64 / usage_rate;
            Some(Utc::now() + chrono::Duration::minutes(minutes_remaining as i64))
        } else {
            None
        };

        Ok(UsageMetrics {
            current_session: session,
            usage_rate,
            projected_depletion,
            efficiency_score,
            session_progress,
            usage_history: Vec::new(),
            
            // Default values for enhanced analytics
            cache_hit_rate: 0.0,
            cache_creation_rate: 0.0,
            token_consumption_rate: usage_rate,
            input_output_ratio: 1.0,
        })
    }

    /// Background monitoring task
    async fn monitoring_loop(&self) -> Result<()> {
        let mut interval = tokio::time::interval(self.update_interval);
        
        loop {
            interval.tick().await;
            
            let is_running = *self.is_running.read().await;
            if !is_running {
                break;
            }

            if let Err(e) = self.update_usage_async().await {
                log::error!("Error updating usage: {}", e);
            }
        }
        
        Ok(())
    }

    /// Internal async update method for background task
    async fn update_usage_async(&self) -> Result<()> {
        let metrics = if self.use_mock_data {
            self.generate_mock_metrics().await?
        } else {
            // Scan for new usage files
            let mut file_monitor = self.file_monitor.write().await;
            file_monitor.scan_usage_files().await?;
            
            // Calculate metrics using file data (passive monitoring)
            file_monitor.calculate_metrics().unwrap_or_else(|| {
                // If no data available, create placeholder metrics using derived session if available
                let placeholder_session = file_monitor.derive_current_session().unwrap_or_else(|| {
                    // Create minimal session if no data exists
                    TokenSession {
                        id: "no-data".to_string(),
                        start_time: chrono::Utc::now(),
                        end_time: None,
                        plan_type: PlanType::Pro,
                        tokens_used: 0,
                        tokens_limit: 40000,
                        is_active: false,
                        reset_time: chrono::Utc::now() + chrono::Duration::hours(5),
                    }
                });
                
                UsageMetrics {
                    current_session: placeholder_session,
                    usage_rate: 0.0,
                    session_progress: 0.0,
                    efficiency_score: 1.0,
                    projected_depletion: None,
                    usage_history: Vec::new(),
                    
                    // Default values for enhanced analytics
                    cache_hit_rate: 0.0,
                    cache_creation_rate: 0.0,
                    token_consumption_rate: 0.0,
                    input_output_ratio: 1.0,
                }
            })
        };

        let mut current_metrics = self.current_metrics.write().await;
        *current_metrics = Some(metrics);

        Ok(())
    }
}

impl<T: SessionService + Send + Sync + 'static> TokenMonitorService for TokenMonitor<T> {
    fn start_monitoring(&mut self) -> Result<()> {
        let is_running = futures::executor::block_on(async {
            let mut is_running = self.is_running.write().await;
            if *is_running {
                return true;
            }
            *is_running = true;
            false
        });

        if is_running {
            return Ok(());
        }

        log::info!("Starting token monitoring with interval {:?}", self.update_interval);
        
        // Start background monitoring task
        let monitor = self.clone();
        tokio::spawn(async move {
            if let Err(e) = monitor.monitoring_loop().await {
                log::error!("Monitoring loop error: {}", e);
            }
        });

        Ok(())
    }

    fn stop_monitoring(&mut self) -> Result<()> {
        futures::executor::block_on(async {
            let mut is_running = self.is_running.write().await;
            *is_running = false;
        });
        log::info!("Stopped token monitoring");
        Ok(())
    }

    fn get_current_usage(&self) -> Result<UsageMetrics> {
        futures::executor::block_on(async {
            let metrics = self.current_metrics.read().await;
            metrics.clone().ok_or_else(|| anyhow!("No usage metrics available"))
        })
    }

    fn update_usage(&mut self) -> Result<()> {
        futures::executor::block_on(self.update_usage_async())
    }
}

impl<T: SessionService + Send + Sync + 'static> Clone for TokenMonitor<T> {
    fn clone(&self) -> Self {
        Self {
            session_service: Arc::clone(&self.session_service),
            file_monitor: Arc::clone(&self.file_monitor),
            current_metrics: Arc::clone(&self.current_metrics),
            is_running: Arc::clone(&self.is_running),
            update_interval: self.update_interval,
            use_mock_data: self.use_mock_data,
        }
    }
}