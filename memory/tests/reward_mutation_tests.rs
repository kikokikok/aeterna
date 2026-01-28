use memory::rlm::trainer::{DecompositionTrajectory, RewardConfig, TrainingOutcome};
use mk_core::types::{SearchQuery, TenantContext, TenantId, UserId};
use std::str::FromStr;

fn test_tenant() -> TenantContext {
    TenantContext::new(
        TenantId::from_str("test-tenant").unwrap(),
        UserId::from_str("test-user").unwrap()
    )
}

fn create_trajectory(tokens_used: usize) -> DecompositionTrajectory {
    let mut traj = DecompositionTrajectory::new(
        SearchQuery {
            text: "test".to_string(),
            ..Default::default()
        },
        test_tenant()
    );
    traj.tokens_used = tokens_used;
    traj
}

#[test]
fn reward_result_used_quality_affects_outcome() {
    let config = RewardConfig::default();

    let mut low_quality = create_trajectory(10_000);
    low_quality.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.2 }, &config);

    let mut high_quality = create_trajectory(10_000);
    high_quality.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.9 }, &config);

    assert!(
        high_quality.reward.unwrap() > low_quality.reward.unwrap(),
        "Higher quality should yield higher reward: {} vs {}",
        high_quality.reward.unwrap(),
        low_quality.reward.unwrap()
    );
}

#[test]
fn reward_result_ignored_is_negative() {
    let config = RewardConfig::default();

    let mut traj = create_trajectory(10_000);
    traj.record_outcome(TrainingOutcome::ResultIgnored, &config);

    assert!(
        traj.reward.unwrap() < 0.0,
        "ResultIgnored should yield negative reward: {}",
        traj.reward.unwrap()
    );
}

#[test]
fn reward_query_refined_is_positive_but_lower_than_used() {
    let config = RewardConfig::default();

    let mut refined = create_trajectory(10_000);
    refined.record_outcome(
        TrainingOutcome::QueryRefined {
            new_query: "new query".to_string()
        },
        &config
    );

    let mut used = create_trajectory(10_000);
    used.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

    assert!(
        refined.reward.unwrap() > 0.0,
        "QueryRefined should yield positive reward"
    );
    assert!(
        used.reward.unwrap() > refined.reward.unwrap(),
        "ResultUsed should yield higher reward than QueryRefined"
    );
}

#[test]
fn reward_no_signal_is_neutral() {
    let config = RewardConfig::default();

    let mut traj = create_trajectory(50_000);
    traj.record_outcome(TrainingOutcome::NoSignal, &config);

    let reward = traj.reward.unwrap();
    assert!(
        reward.abs() < 0.5,
        "NoSignal should yield near-neutral reward: {}",
        reward
    );
}

#[test]
fn reward_efficiency_penalizes_high_token_usage() {
    let config = RewardConfig::default();

    let mut low_tokens = create_trajectory(5_000);
    low_tokens.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

    let mut high_tokens = create_trajectory(80_000);
    high_tokens.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

    assert!(
        low_tokens.reward.unwrap() > high_tokens.reward.unwrap(),
        "Lower token usage should yield higher reward: {} vs {}",
        low_tokens.reward.unwrap(),
        high_tokens.reward.unwrap()
    );
}

#[test]
fn reward_efficiency_caps_at_100k_tokens() {
    let config = RewardConfig::default();

    let mut at_cap = create_trajectory(100_000);
    at_cap.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

    let mut over_cap = create_trajectory(200_000);
    over_cap.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.8 }, &config);

    assert_eq!(
        at_cap.reward.unwrap(),
        over_cap.reward.unwrap(),
        "Tokens over 100k should not further decrease reward"
    );
}

#[test]
fn reward_weights_affect_computation() {
    let success_heavy = RewardConfig {
        success_weight: 2.0,
        efficiency_weight: 0.1
    };

    let efficiency_heavy = RewardConfig {
        success_weight: 0.5,
        efficiency_weight: 1.0
    };

    let mut traj1 = create_trajectory(50_000);
    traj1.record_outcome(
        TrainingOutcome::ResultUsed { quality_score: 0.8 },
        &success_heavy
    );

    let mut traj2 = create_trajectory(50_000);
    traj2.record_outcome(
        TrainingOutcome::ResultUsed { quality_score: 0.8 },
        &efficiency_heavy
    );

    assert_ne!(
        traj1.reward.unwrap(),
        traj2.reward.unwrap(),
        "Different weights should produce different rewards"
    );
}

#[test]
fn reward_clamped_to_minus_one_to_one() {
    let extreme_config = RewardConfig {
        success_weight: 10.0,
        efficiency_weight: 10.0
    };

    let mut high_reward = create_trajectory(0);
    high_reward.record_outcome(
        TrainingOutcome::ResultUsed { quality_score: 1.0 },
        &extreme_config
    );

    let mut low_reward = create_trajectory(100_000);
    low_reward.record_outcome(TrainingOutcome::ResultIgnored, &extreme_config);

    assert!(
        high_reward.reward.unwrap() <= 1.0,
        "Reward should be clamped to max 1.0: {}",
        high_reward.reward.unwrap()
    );
    assert!(
        low_reward.reward.unwrap() >= -1.0,
        "Reward should be clamped to min -1.0: {}",
        low_reward.reward.unwrap()
    );
}

#[test]
fn reward_zero_tokens_gives_full_efficiency_bonus() {
    let config = RewardConfig {
        success_weight: 0.0,
        efficiency_weight: 1.0
    };

    let mut traj = create_trajectory(0);
    traj.record_outcome(TrainingOutcome::NoSignal, &config);

    assert_eq!(
        traj.reward.unwrap(),
        1.0,
        "Zero tokens with efficiency_weight=1.0 should give full bonus"
    );
}

#[test]
fn reward_boundary_quality_scores() {
    let config = RewardConfig::default();

    let mut zero_quality = create_trajectory(10_000);
    zero_quality.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.0 }, &config);

    let mut one_quality = create_trajectory(10_000);
    one_quality.record_outcome(TrainingOutcome::ResultUsed { quality_score: 1.0 }, &config);

    assert!(
        zero_quality.reward.unwrap() < one_quality.reward.unwrap(),
        "Quality 0.0 should yield lower reward than 1.0"
    );

    assert!(
        zero_quality.reward.unwrap() >= -1.0 && zero_quality.reward.unwrap() <= 1.0,
        "Reward should be in valid range"
    );
    assert!(
        one_quality.reward.unwrap() >= -1.0 && one_quality.reward.unwrap() <= 1.0,
        "Reward should be in valid range"
    );
}

#[test]
fn reward_different_outcomes_ordering() {
    let config = RewardConfig::default();
    let tokens = 10_000;

    let mut used_high = create_trajectory(tokens);
    used_high.record_outcome(TrainingOutcome::ResultUsed { quality_score: 1.0 }, &config);

    let mut used_medium = create_trajectory(tokens);
    used_medium.record_outcome(TrainingOutcome::ResultUsed { quality_score: 0.5 }, &config);

    let mut refined = create_trajectory(tokens);
    refined.record_outcome(
        TrainingOutcome::QueryRefined {
            new_query: "q".to_string()
        },
        &config
    );

    let mut no_signal = create_trajectory(tokens);
    no_signal.record_outcome(TrainingOutcome::NoSignal, &config);

    let mut ignored = create_trajectory(tokens);
    ignored.record_outcome(TrainingOutcome::ResultIgnored, &config);

    assert!(used_high.reward.unwrap() > used_medium.reward.unwrap());
    assert!(used_medium.reward.unwrap() > refined.reward.unwrap());
    assert!(refined.reward.unwrap() > no_signal.reward.unwrap());
    assert!(no_signal.reward.unwrap() > ignored.reward.unwrap());
}

#[test]
fn reward_with_zero_weights_is_zero() {
    let config = RewardConfig {
        success_weight: 0.0,
        efficiency_weight: 0.0
    };

    let mut traj = create_trajectory(50_000);
    traj.record_outcome(TrainingOutcome::ResultUsed { quality_score: 1.0 }, &config);

    assert_eq!(
        traj.reward.unwrap(),
        0.0,
        "Zero weights should yield zero reward"
    );
}

#[test]
fn reward_exact_threshold_token_count() {
    let config = RewardConfig {
        success_weight: 0.0,
        efficiency_weight: 1.0
    };

    let mut at_threshold = create_trajectory(50_000);
    at_threshold.record_outcome(TrainingOutcome::NoSignal, &config);

    let expected_efficiency = 1.0 - (50_000.0 / 100_000.0);
    assert!(
        (at_threshold.reward.unwrap() - expected_efficiency).abs() < 0.001,
        "50k tokens should give 0.5 efficiency: {}",
        at_threshold.reward.unwrap()
    );
}
