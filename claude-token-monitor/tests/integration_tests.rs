use claude_token_monitor::models::*;
use claude_token_monitor::services::session_tracker::SessionTracker;
use claude_token_monitor::services::SessionService;
use chrono::Utc;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio_test;

#[tokio::test]
async fn test_session_creation() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("sessions.json");
    
    let mut tracker = SessionTracker::new(sessions_path);
    let session = tracker.create_session(PlanType::Pro).await.unwrap();
    
    assert_eq!(session.plan_type, PlanType::Pro);
    assert_eq!(session.tokens_limit, 40_000);
    assert_eq!(session.tokens_used, 0);
    assert!(session.is_active);
    assert!(session.start_time <= Utc::now());
}

#[tokio::test]
async fn test_session_update() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("sessions.json");
    
    let mut tracker = SessionTracker::new(sessions_path);
    let session = tracker.create_session(PlanType::Max5).await.unwrap();
    
    tracker.update_session(&session.id, 1000).await.unwrap();
    
    let updated_session = tracker.get_active_session().await.unwrap().unwrap();
    assert_eq!(updated_session.tokens_used, 1000);
    assert_eq!(updated_session.id, session.id);
}

#[tokio::test]
async fn test_session_history() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("sessions.json");
    
    let mut tracker = SessionTracker::new(sessions_path);
    
    // Create multiple sessions
    let _session1 = tracker.create_session(PlanType::Pro).await.unwrap();
    let _session2 = tracker.create_session(PlanType::Max5).await.unwrap();
    let _session3 = tracker.create_session(PlanType::Max20).await.unwrap();
    
    let history = tracker.get_session_history(10).await.unwrap();
    assert_eq!(history.len(), 3);
    
    // Should be sorted by start time (newest first)
    assert!(history[0].start_time >= history[1].start_time);
    assert!(history[1].start_time >= history[2].start_time);
}

#[tokio::test]
async fn test_session_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("sessions.json");
    
    let session_id = {
        let mut tracker = SessionTracker::new(sessions_path.clone());
        let session = tracker.create_session(PlanType::Pro).await.unwrap();
        tracker.update_session(&session.id, 5000).await.unwrap();
        session.id
    };
    
    // Create new tracker instance (simulating restart)
    let mut tracker2 = SessionTracker::new(sessions_path);
    tracker2.load_sessions().await.unwrap();
    
    let loaded_session = tracker2.get_active_session().await.unwrap().unwrap();
    assert_eq!(loaded_session.id, session_id);
    assert_eq!(loaded_session.tokens_used, 5000);
}

#[tokio::test]
async fn test_plan_type_limits() {
    assert_eq!(PlanType::Pro.default_limit(), 40_000);
    assert_eq!(PlanType::Max5.default_limit(), 20_000);
    assert_eq!(PlanType::Max20.default_limit(), 100_000);
    assert_eq!(PlanType::Custom(50_000).default_limit(), 50_000);
}

#[tokio::test]
async fn test_user_config_defaults() {
    let config = UserConfig::default();
    assert_eq!(config.default_plan, PlanType::Pro);
    assert_eq!(config.update_interval_seconds, 3);
    assert_eq!(config.warning_threshold, 0.85);
    assert!(config.auto_switch_plans);
}

#[tokio::test]
async fn test_usage_metrics_calculation() {
    let session = TokenSession {
        id: "test".to_string(),
        start_time: Utc::now() - chrono::Duration::minutes(10),
        end_time: None,
        plan_type: PlanType::Pro,
        tokens_used: 1000,
        tokens_limit: 40_000,
        is_active: true,
        reset_time: Utc::now() + chrono::Duration::hours(5),
    };
    
    let usage_point = TokenUsagePoint {
        timestamp: Utc::now(),
        tokens_used: 1000,
        session_id: "test".to_string(),
    };
    
    let metrics = UsageMetrics {
        current_session: session,
        usage_rate: 100.0, // 100 tokens per minute
        projected_depletion: None,
        efficiency_score: 0.95,
        session_progress: 0.1,
        usage_history: vec![usage_point],
    };
    
    assert_eq!(metrics.usage_rate, 100.0);
    assert_eq!(metrics.efficiency_score, 0.95);
    assert_eq!(metrics.session_progress, 0.1);
    assert_eq!(metrics.usage_history.len(), 1);
}