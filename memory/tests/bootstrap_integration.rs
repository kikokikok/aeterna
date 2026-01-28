use memory::rlm::bootstrap::{
    BootstrapTaskTemplate, BootstrapTrainer, ComplexityLevel, generate_bootstrap_tasks
};
use memory::rlm::trainer::RewardConfig;
use mk_core::types::{MemoryLayer, TenantContext, TenantId, UserId};
use std::str::FromStr;

fn test_tenant() -> TenantContext {
    TenantContext::new(
        TenantId::from_str("bootstrap-test-tenant").unwrap(),
        UserId::from_str("bootstrap-test-user").unwrap()
    )
}

#[test]
fn test_bootstrap_template_simple_creates_minimal_actions() {
    let template = BootstrapTaskTemplate::simple("test_simple", "find the config file");

    assert_eq!(template.name, "test_simple");
    assert_eq!(template.complexity_level, ComplexityLevel::Simple);
    assert_eq!(template.expected_actions.len(), 2);
}

#[test]
fn test_bootstrap_template_moderate_includes_multiple_layers() {
    let template = BootstrapTaskTemplate::moderate(
        "test_moderate",
        "find patterns",
        vec![MemoryLayer::Project, MemoryLayer::Team, MemoryLayer::Org]
    );

    assert_eq!(template.complexity_level, ComplexityLevel::Moderate);
    assert_eq!(template.expected_actions.len(), 4);
}

#[test]
fn test_bootstrap_template_complex_has_deep_actions() {
    let template =
        BootstrapTaskTemplate::complex("test_complex", "analyze and compare all patterns");

    assert_eq!(template.complexity_level, ComplexityLevel::Complex);
    assert!(template.expected_actions.len() >= 3);
}

#[test]
fn test_generate_bootstrap_tasks_creates_correct_count() {
    let templates = vec![
        BootstrapTaskTemplate::simple("t1", "query1"),
        BootstrapTaskTemplate::simple("t2", "query2"),
    ];

    let tasks = generate_bootstrap_tasks(&templates, &test_tenant(), 10);
    assert_eq!(tasks.len(), 20);
}

#[test]
fn test_generate_bootstrap_tasks_with_mixed_complexity() {
    let templates = vec![
        BootstrapTaskTemplate::simple("simple", "simple query"),
        BootstrapTaskTemplate::moderate(
            "moderate",
            "mod query",
            vec![MemoryLayer::Project, MemoryLayer::Team]
        ),
        BootstrapTaskTemplate::complex("complex", "complex query"),
    ];

    let tasks = generate_bootstrap_tasks(&templates, &test_tenant(), 5);
    assert_eq!(tasks.len(), 15);

    let simple_count = tasks.iter().filter(|t| t.actions.len() == 2).count();
    let moderate_count = tasks.iter().filter(|t| t.actions.len() == 3).count();
    let complex_count = tasks.iter().filter(|t| t.actions.len() >= 4).count();

    assert_eq!(
        simple_count, 5,
        "Should have 5 simple tasks (2 actions each)"
    );
    assert_eq!(
        moderate_count, 5,
        "Should have 5 moderate tasks (3 actions each)"
    );
    assert_eq!(
        complex_count, 5,
        "Should have 5 complex tasks (4+ actions each)"
    );
}

#[test]
fn test_trajectory_has_correct_tenant_context() {
    let tenant = test_tenant();
    let template = BootstrapTaskTemplate::simple("test", "query");
    let trajectory = template.to_trajectory(&tenant);

    assert_eq!(trajectory.tenant_context.tenant_id, tenant.tenant_id);
    assert_eq!(trajectory.tenant_context.user_id, tenant.user_id);
}

#[test]
fn test_trajectory_has_reward_assigned() {
    let template = BootstrapTaskTemplate::simple("test", "query");
    let trajectory = template.to_trajectory(&test_tenant());

    assert!(trajectory.reward.is_some());
    assert!(trajectory.reward.unwrap() > 0.0);
}

#[test]
fn test_trajectory_token_usage_varies_by_complexity() {
    let simple = BootstrapTaskTemplate::simple("s", "q").to_trajectory(&test_tenant());
    let moderate = BootstrapTaskTemplate::moderate("m", "q", vec![MemoryLayer::Project])
        .to_trajectory(&test_tenant());
    let complex = BootstrapTaskTemplate::complex("c", "q").to_trajectory(&test_tenant());

    assert!(simple.tokens_used < moderate.tokens_used);
    assert!(moderate.tokens_used < complex.tokens_used);
}

#[tokio::test]
async fn test_bootstrap_trainer_initialization() {
    let trainer = BootstrapTrainer::new(RewardConfig::default());

    assert_eq!(trainer.trained_count(), 0);
    assert!(trainer.trainer().epsilon() > 0.0);
}

#[tokio::test]
async fn test_bootstrap_trainer_with_custom_templates() {
    let custom_templates = vec![
        BootstrapTaskTemplate::simple("custom1", "custom query 1"),
        BootstrapTaskTemplate::simple("custom2", "custom query 2"),
    ];

    let mut trainer =
        BootstrapTrainer::new(RewardConfig::default()).with_templates(custom_templates);

    let result = trainer.bootstrap(&test_tenant(), 5).await.unwrap();

    assert_eq!(result.tasks_trained, 10);
}

#[tokio::test]
async fn test_bootstrap_trainer_multiple_iterations() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default())
        .with_templates(vec![BootstrapTaskTemplate::simple("test", "query")]);

    trainer.bootstrap(&test_tenant(), 50).await.unwrap();

    let result = trainer.bootstrap(&test_tenant(), 50).await.unwrap();

    assert_eq!(trainer.trained_count(), 100);
    assert!(result.average_reward > 0.0);
}

#[tokio::test]
async fn test_bootstrap_trainer_result_contains_metrics() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default());

    let result = trainer.bootstrap(&test_tenant(), 10).await.unwrap();

    assert!(result.tasks_trained > 0);
    assert!(result.final_epsilon > 0.0);
    assert!(result.final_epsilon <= 0.1);
}

#[tokio::test]
async fn test_bootstrap_trainer_into_decomposition_trainer() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default())
        .with_templates(vec![BootstrapTaskTemplate::simple("test", "query")]);

    trainer.bootstrap(&test_tenant(), 100).await.unwrap();

    let decomposition_trainer = trainer.into_trainer();
    let state = decomposition_trainer.export_state().unwrap();

    assert!(state.step_count >= 100);
}

#[tokio::test]
async fn test_bootstrap_with_default_templates() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default());

    let result = trainer.bootstrap(&test_tenant(), 2).await.unwrap();

    assert!(result.tasks_trained >= 18);
}

#[tokio::test]
async fn test_bootstrap_empty_templates() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default()).with_templates(vec![]);

    let result = trainer.bootstrap(&test_tenant(), 10).await.unwrap();

    assert_eq!(result.tasks_trained, 0);
    assert_eq!(result.average_reward, 0.0);
}

#[tokio::test]
async fn test_bootstrap_single_iteration() {
    let mut trainer = BootstrapTrainer::new(RewardConfig::default())
        .with_templates(vec![BootstrapTaskTemplate::simple("single", "query")]);

    let result = trainer.bootstrap(&test_tenant(), 1).await.unwrap();

    assert_eq!(result.tasks_trained, 1);
    assert_eq!(trainer.trained_count(), 1);
}

#[test]
fn test_reward_config_affects_trajectory_reward() {
    let high_success = RewardConfig {
        success_weight: 2.0,
        efficiency_weight: 0.1
    };

    let high_efficiency = RewardConfig {
        success_weight: 0.5,
        efficiency_weight: 1.0
    };

    let template = BootstrapTaskTemplate::simple("test", "query");
    let trajectory = template.to_trajectory(&test_tenant());

    let reward_high_success = high_success.compute(&trajectory);
    let reward_high_efficiency = high_efficiency.compute(&trajectory);

    assert!(reward_high_success > 0.0);
    assert!(reward_high_efficiency > 0.0);
}

#[test]
fn test_complexity_level_serialization() {
    use serde_json;

    let level = ComplexityLevel::Complex;
    let json = serde_json::to_string(&level).unwrap();
    let deserialized: ComplexityLevel = serde_json::from_str(&json).unwrap();

    assert_eq!(level, deserialized);
}

#[test]
fn test_template_serialization_roundtrip() {
    use serde_json;

    let template = BootstrapTaskTemplate::moderate(
        "serialization_test",
        "test query",
        vec![MemoryLayer::Project, MemoryLayer::Team]
    );

    let json = serde_json::to_string(&template).unwrap();
    let deserialized: BootstrapTaskTemplate = serde_json::from_str(&json).unwrap();

    assert_eq!(template.name, deserialized.name);
    assert_eq!(template.query_pattern, deserialized.query_pattern);
    assert_eq!(template.complexity_level, deserialized.complexity_level);
    assert_eq!(
        template.expected_actions.len(),
        deserialized.expected_actions.len()
    );
}
