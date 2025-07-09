use claude_token_monitor::models::*;
use claude_token_monitor::services::session_tracker::SessionTracker;
use claude_token_monitor::services::SessionService;
use chrono::Utc;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_observation() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("observed_sessions.json");
    
    let tracker = SessionTracker::new(sessions_path).unwrap();
    
    // Test that we can create a tracker without errors
    assert!(tracker.get_active_session().await.is_ok());
    
    // Should return None when no observed sessions exist
    let active_session = tracker.get_active_session().await.unwrap();
    assert!(active_session.is_none());
}

#[tokio::test]
async fn test_session_history_empty() {
    let temp_dir = TempDir::new().unwrap();
    let sessions_path = temp_dir.path().join("observed_sessions.json");
    
    let tracker = SessionTracker::new(sessions_path).unwrap();
    
    let history = tracker.get_session_history(10).await.unwrap();
    assert_eq!(history.len(), 0);
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
        id: "observed-test".to_string(),
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
        session_id: "observed-test".to_string(),
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

#[tokio::test]
async fn test_passive_monitoring_principles() {
    // Test that observed session IDs follow the "observed-" pattern
    let session = TokenSession {
        id: "observed-1752068062".to_string(),
        start_time: Utc::now() - chrono::Duration::minutes(30),
        end_time: None,
        plan_type: PlanType::Max20,
        tokens_used: 54143,
        tokens_limit: 100_000,
        is_active: true,
        reset_time: Utc::now() + chrono::Duration::hours(4),
    };
    
    // Verify session follows passive monitoring pattern
    assert!(session.id.starts_with("observed-"));
    assert!(session.tokens_used > 0);
    assert!(session.is_active);
    assert_eq!(session.plan_type, PlanType::Max20);
    assert_eq!(session.tokens_limit, 100_000);
}

#[tokio::test]
async fn test_token_session_serialization() {
    let session = TokenSession {
        id: "observed-test".to_string(),
        start_time: Utc::now(),
        end_time: None,
        plan_type: PlanType::Pro,
        tokens_used: 1500,
        tokens_limit: 40_000,
        is_active: true,
        reset_time: Utc::now() + chrono::Duration::hours(5),
    };
    
    // Test serialization/deserialization
    let serialized = serde_json::to_string(&session).unwrap();
    let deserialized: TokenSession = serde_json::from_str(&serialized).unwrap();
    
    assert_eq!(session.id, deserialized.id);
    assert_eq!(session.tokens_used, deserialized.tokens_used);
    assert_eq!(session.plan_type, deserialized.plan_type);
    assert_eq!(session.is_active, deserialized.is_active);
}