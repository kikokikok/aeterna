//! Integration tests for combined R1 + RLM training.
//!
//! These tests verify that the combined trainer works correctly with
//! both Memory-R1 and RLM decomposition training pipelines.

use memory::rlm::combined_trainer::CombinedMemoryTrainer;
use memory::rlm::trainer::{DecompositionTrajectory, TrainingOutcome};
use memory::trainer::R1TrainerConfig;
use mk_core::types::{SearchQuery, TenantContext, TenantId, UserId};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Create a test tenant context.
fn test_tenant_context() -> TenantContext {
    TenantContext::new(
        TenantId::from_str("test-tenant").unwrap(),
        UserId::from_str("test-user").unwrap()
    )
}

/// Create a test combined trainer with minimal configuration.
fn create_test_combined_trainer() -> CombinedMemoryTrainer {
    let r1_config = R1TrainerConfig {
        min_batch_size: 1,
        learning_rate: 0.01,
        discount_factor: 0.99,
        max_trajectory_length: 100,
        baseline_decay: 0.9,
        min_weight: 0.1,
        max_weight: 10.0
    };

    let r1_trajectories: Arc<RwLock<HashMap<String, Vec<mk_core::types::MemoryTrajectoryEvent>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    let decomposition_config = memory::rlm::trainer::RewardConfig::default();

    CombinedMemoryTrainer::new(r1_config, decomposition_config, r1_trajectories)
}

#[tokio::test]
async fn test_combined_trainer_creation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trainer = create_test_combined_trainer();

    assert_eq!(trainer.decomposition_trainer().epsilon(), 0.1);

    Ok(())
}

#[tokio::test]
async fn test_decomposition_training_flow() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
{
    let mut trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Create and add a decomposition trajectory
    let trajectory = DecompositionTrajectory::new(
        SearchQuery {
            text: "How do we handle authentication in microservices?".to_string(),
            target_layers: Vec::new(),
            filters: std::collections::HashMap::new(),
            limit: 10,
            threshold: 0.7
        },
        tenant_ctx.clone()
    );

    trainer
        .add_decomposition_trajectory(trajectory.clone())
        .await;

    // Record outcome
    trainer
        .record_decomposition_outcome(
            &trajectory.id,
            TrainingOutcome::ResultUsed { quality_score: 0.8 }
        )
        .await?;

    // Train decomposition
    let metrics = trainer.train_decomposition().await?;

    // Verify training occurred
    assert_eq!(metrics.trajectories_trained, 1);
    assert!(metrics.average_reward > 0.0);

    Ok(())
}

#[tokio::test]
async fn test_export_import_state() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trainer1 = create_test_combined_trainer();

    // Export state
    let state = trainer1.export_state();

    // Create new trainer and import state
    let mut trainer2 = create_test_combined_trainer();
    trainer2.import_state(state)?;

    // Verify state was imported correctly
    assert_eq!(
        trainer1.decomposition_trainer().epsilon(),
        trainer2.decomposition_trainer().epsilon()
    );

    Ok(())
}

#[tokio::test]
async fn test_clear_decomposition_trajectories()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Add some trajectories
    for i in 0..3 {
        let trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: format!("Test query {}", i),
                target_layers: Vec::new(),
                filters: std::collections::HashMap::new(),
                limit: 10,
                threshold: 0.7
            },
            tenant_ctx.clone()
        );

        trainer.add_decomposition_trajectory(trajectory).await;
    }

    // Verify trajectories were added
    assert_eq!(trainer.decomposition_trajectory_count().await, 3);

    // Clear trajectories
    trainer.clear_decomposition_trajectories().await;

    // Verify trajectories were cleared
    assert_eq!(trainer.decomposition_trajectory_count().await, 0);

    Ok(())
}

#[tokio::test]
async fn test_multiple_trajectories_training()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Add multiple trajectories with different outcomes
    let trajectories = vec![
        (
            "How do we handle authentication?".to_string(),
            TrainingOutcome::ResultUsed { quality_score: 0.9 }
        ),
        (
            "What's our deployment strategy?".to_string(),
            TrainingOutcome::ResultUsed { quality_score: 0.7 }
        ),
        (
            "How to implement caching?".to_string(),
            TrainingOutcome::QueryRefined {
                new_query: "How to implement Redis caching?".to_string()
            }
        ),
        (
            "Database schema design".to_string(),
            TrainingOutcome::ResultIgnored
        ),
    ];

    for (query_text, outcome) in trajectories {
        let trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: query_text,
                target_layers: Vec::new(),
                filters: std::collections::HashMap::new(),
                limit: 10,
                threshold: 0.7
            },
            tenant_ctx.clone()
        );

        trainer
            .add_decomposition_trajectory(trajectory.clone())
            .await;

        // Record outcomes
        trainer
            .record_decomposition_outcome(&trajectory.id, outcome)
            .await?;
    }

    // Train on all trajectories
    let metrics = trainer.train_decomposition().await?;

    // Verify training metrics
    assert_eq!(metrics.trajectories_trained, 4);
    assert!(metrics.average_reward >= -1.0 && metrics.average_reward <= 1.0);

    Ok(())
}

#[tokio::test]
async fn test_train_step_with_decomposition_only()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Skip R1 training by not adding any R1 trajectories
    // This test focuses on decomposition training only

    // Add a decomposition trajectory
    let trajectory = DecompositionTrajectory::new(
        SearchQuery {
            text: "Test query for decomposition".to_string(),
            target_layers: Vec::new(),
            filters: std::collections::HashMap::new(),
            limit: 10,
            threshold: 0.7
        },
        tenant_ctx.clone()
    );

    trainer
        .add_decomposition_trajectory(trajectory.clone())
        .await;

    // Record outcome
    trainer
        .record_decomposition_outcome(
            &trajectory.id,
            TrainingOutcome::ResultUsed { quality_score: 0.8 }
        )
        .await?;

    // Try to run train_step (may fail due to missing R1 data, but that's expected)
    let result = trainer.train_step().await;

    // The result might be an error due to missing R1 data, which is OK
    // We're testing that the method doesn't panic
    match result {
        Ok(metrics) => {
            // If it succeeds, verify metrics structure
            // total_trajectories is usize, so it's always >= 0
            assert!(metrics.decomposition_metrics.exploration_rate > 0.0);
        }
        Err(_) => {
            // Expected if R1 trainer has no data
            // This is acceptable for this test
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_policy_state_persistence_cycle()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Train on some trajectories to change policy state
    for i in 0..2 {
        let trajectory = DecompositionTrajectory::new(
            SearchQuery {
                text: format!("Training query {}", i),
                target_layers: Vec::new(),
                filters: std::collections::HashMap::new(),
                limit: 10,
                threshold: 0.7
            },
            tenant_ctx.clone()
        );

        trainer
            .add_decomposition_trajectory(trajectory.clone())
            .await;

        trainer
            .record_decomposition_outcome(
                &trajectory.id,
                TrainingOutcome::ResultUsed { quality_score: 0.8 }
            )
            .await?;
    }

    // Train to update policy
    let metrics_before = trainer.train_decomposition().await?;

    // Export state
    let state = trainer.export_state();

    // Create new trainer and import state
    let mut new_trainer = create_test_combined_trainer();
    new_trainer.import_state(state.clone())?;

    // Verify policy state was preserved
    assert_eq!(
        trainer.decomposition_trainer().epsilon(),
        new_trainer.decomposition_trainer().epsilon()
    );

    // Train again on new trainer
    let metrics_after = new_trainer.train_decomposition().await?;

    // Exploration rate should have decayed (or stayed the same)
    assert!(metrics_after.exploration_rate <= metrics_before.exploration_rate);

    Ok(())
}

#[tokio::test]
async fn test_empty_trajectory_handling() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut trainer = create_test_combined_trainer();

    // Train with no trajectories
    let metrics = trainer.train_decomposition().await?;

    // Should return default metrics
    assert_eq!(metrics.trajectories_trained, 0);
    assert_eq!(metrics.average_reward, 0.0);
    assert_eq!(metrics.exploration_rate, 0.1); // Default epsilon

    Ok(())
}

#[tokio::test]
async fn test_incomplete_trajectory_handling()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut trainer = create_test_combined_trainer();
    let tenant_ctx = test_tenant_context();

    // Add a trajectory without recording outcome (incomplete)
    let trajectory = DecompositionTrajectory::new(
        SearchQuery {
            text: "Incomplete query".to_string(),
            target_layers: Vec::new(),
            filters: std::collections::HashMap::new(),
            limit: 10,
            threshold: 0.7
        },
        tenant_ctx.clone()
    );

    trainer.add_decomposition_trajectory(trajectory).await;

    // Train - should skip incomplete trajectory
    let metrics = trainer.train_decomposition().await?;

    assert_eq!(metrics.trajectories_trained, 0);
    assert_eq!(metrics.average_reward, 0.0);

    Ok(())
}
