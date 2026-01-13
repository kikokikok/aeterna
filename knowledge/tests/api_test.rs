use knowledge::api::{
    GovernanceDashboardApi, approve_proposal, get_drift_status, get_job_status, get_org_report,
    reject_proposal, replay_events,
};
use knowledge::governance::GovernanceEngine;
use mk_core::traits::{EventPublisher, KnowledgeRepository, StorageBackend};
use mk_core::types::{
    GovernanceEvent, KnowledgeEntry, KnowledgeLayer, KnowledgeStatus, KnowledgeType,
    OrganizationalUnit, TenantContext, TenantId, UnitType,
};
use std::sync::Arc;
use storage::postgres::PostgresBackend;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn test_governance_dashboard_api_get_drift_status() {
    let container = match Postgres::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Postgres test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!(
        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
        host, port
    );

    let storage = Arc::new(PostgresBackend::new(&conn_str).await.unwrap());
    storage.initialize_schema().await.unwrap();

    let engine = Arc::new(GovernanceEngine::new());
    let deployment_config = config::config::DeploymentConfig {
        mode: "local".to_string(),
        remote_url: None,
        ..Default::default()
    };

    let api = Arc::new(GovernanceDashboardApi::new(
        engine.clone(),
        storage.clone(),
        deployment_config,
    ));

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("u1".to_string()).unwrap(),
        agent_id: Some("a1".to_string()),
    };

    let res = get_drift_status(api.clone(), &ctx, "p1").await.unwrap();
    assert!(res.is_none());

    let drift = mk_core::types::DriftResult {
        project_id: "p1".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.75,
        violations: vec![],
        timestamp: 123456789,
    };
    storage.store_drift_result(drift).await.unwrap();

    let res = get_drift_status(api.clone(), &ctx, "p1").await.unwrap();
    assert!(res.is_some());
    assert_eq!(res.unwrap().drift_score, 0.75);
}

#[tokio::test]
async fn test_governance_dashboard_api_get_org_report() {
    let container = match Postgres::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Postgres test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!(
        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
        host, port
    );

    let storage = Arc::new(PostgresBackend::new(&conn_str).await.unwrap());
    storage.initialize_schema().await.unwrap();

    let engine = Arc::new(GovernanceEngine::new());
    let deployment_config = config::config::DeploymentConfig {
        mode: "local".to_string(),
        remote_url: None,
        ..Default::default()
    };

    let api = Arc::new(GovernanceDashboardApi::new(
        engine.clone(),
        storage.clone(),
        deployment_config,
    ));

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("u1".to_string()).unwrap(),
        agent_id: Some("a1".to_string()),
    };

    let company_unit = OrganizationalUnit {
        id: "c1".to_string(),
        name: "Company 1".to_string(),
        unit_type: UnitType::Company,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&company_unit).await.unwrap();

    let org_unit = OrganizationalUnit {
        id: "org1".to_string(),
        name: "Org 1".to_string(),
        unit_type: UnitType::Organization,
        tenant_id: tenant_id.clone(),
        parent_id: Some("c1".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&org_unit).await.unwrap();

    let team_unit = OrganizationalUnit {
        id: "t1_unit".to_string(),
        name: "Team 1".to_string(),
        unit_type: UnitType::Team,
        tenant_id: tenant_id.clone(),
        parent_id: Some("org1".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&team_unit).await.unwrap();

    let project_unit = OrganizationalUnit {
        id: "p1".to_string(),
        name: "Project 1".to_string(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: Some("t1_unit".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&project_unit).await.unwrap();

    let drift = mk_core::types::DriftResult {
        project_id: "p1".to_string(),
        tenant_id: tenant_id.clone(),
        drift_score: 0.5,
        violations: vec![],
        timestamp: 123456789,
    };
    storage.store_drift_result(drift).await.unwrap();

    let report = get_org_report(api.clone(), &ctx, "c1").await.unwrap();
    assert_eq!(report["orgId"], "c1");
    assert_eq!(report["averageDrift"], 0.5);
    assert_eq!(report["projectCount"], 1);
}

#[tokio::test]
async fn test_governance_dashboard_api_proposals() {
    let container = match Postgres::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Postgres test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!(
        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
        host, port
    );

    let storage = Arc::new(PostgresBackend::new(&conn_str).await.unwrap());
    storage.initialize_schema().await.unwrap();

    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        knowledge::repository::GitRepository::new(temp_dir.path().to_str().unwrap()).unwrap(),
    );

    let engine = Arc::new(GovernanceEngine::new().with_repository(repo.clone()));
    let deployment_config = config::config::DeploymentConfig {
        mode: "local".to_string(),
        remote_url: None,
        ..Default::default()
    };

    let api = Arc::new(GovernanceDashboardApi::new(
        engine.clone(),
        storage.clone(),
        deployment_config,
    ));

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("u1".to_string()).unwrap(),
        agent_id: Some("a1".to_string()),
    };

    let entry = KnowledgeEntry {
        path: "prop1".to_string(),
        content: "Content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Adr,
        status: KnowledgeStatus::Proposed,
        metadata: std::collections::HashMap::new(),
        commit_hash: None,
        author: Some("u1".to_string()),
        updated_at: 1000,
    };
    repo.store(ctx.clone(), entry, "Initial proposal")
        .await
        .unwrap();

    let proposals = api.list_proposals(&ctx, None).await.unwrap();
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].path, "prop1");

    approve_proposal(api.clone(), &ctx, "prop1").await.unwrap();
    let approved = repo
        .get(ctx.clone(), KnowledgeLayer::Project, "prop1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(approved.status, KnowledgeStatus::Accepted);

    let entry2 = KnowledgeEntry {
        path: "prop2".to_string(),
        content: "Content".to_string(),
        layer: KnowledgeLayer::Project,
        kind: KnowledgeType::Adr,
        status: KnowledgeStatus::Proposed,
        metadata: std::collections::HashMap::new(),
        commit_hash: None,
        author: Some("u1".to_string()),
        updated_at: 1000,
    };
    repo.store(ctx.clone(), entry2, "Initial proposal")
        .await
        .unwrap();

    reject_proposal(api.clone(), &ctx, "prop2", "Too complex")
        .await
        .unwrap();
    let rejected = repo
        .get(ctx.clone(), KnowledgeLayer::Project, "prop2")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rejected.status, KnowledgeStatus::Draft);
    assert_eq!(
        rejected.metadata.get("rejection_reason").unwrap(),
        &serde_json::json!("Too complex")
    );
}

#[tokio::test]
async fn test_governance_dashboard_api_get_job_status() {
    let container = match Postgres::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Postgres test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!(
        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
        host, port
    );

    let storage = Arc::new(PostgresBackend::new(&conn_str).await.unwrap());
    storage.initialize_schema().await.unwrap();

    let engine = Arc::new(GovernanceEngine::new());
    let api = Arc::new(GovernanceDashboardApi::new(
        engine,
        storage.clone(),
        config::config::DeploymentConfig::default(),
    ));

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("u1".to_string()).unwrap(),
        agent_id: Some("a1".to_string()),
    };

    storage
        .record_job_status(
            "test-job",
            "t1",
            "completed",
            Some("Done"),
            1000,
            Some(1001),
        )
        .await
        .unwrap();
    storage
        .record_job_status("other-job", "t1", "running", None, 1002, None)
        .await
        .unwrap();
    storage
        .record_job_status("global-job", "all", "completed", None, 1003, Some(1004))
        .await
        .unwrap();

    let all_jobs = get_job_status(api.clone(), &ctx, None).await.unwrap();
    assert_eq!(all_jobs.as_array().unwrap().len(), 3);

    let filtered_jobs = get_job_status(api.clone(), &ctx, Some("test-job"))
        .await
        .unwrap();
    assert_eq!(filtered_jobs.as_array().unwrap().len(), 1);
    assert_eq!(filtered_jobs[0]["jobName"], "test-job");
    assert_eq!(filtered_jobs[0]["status"], "completed");
}

#[tokio::test]
async fn test_governance_dashboard_api_replay_events() {
    let container = match Postgres::default().start().await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping Postgres test: Docker not available");
            return;
        }
    };

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let conn_str = format!(
        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
        host, port
    );

    let storage = Arc::new(PostgresBackend::new(&conn_str).await.unwrap());
    storage.initialize_schema().await.unwrap();

    let engine = Arc::new(GovernanceEngine::new());
    let api = Arc::new(GovernanceDashboardApi::new(
        engine,
        storage.clone(),
        config::config::DeploymentConfig::default(),
    ));

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: mk_core::types::UserId::new("u1".to_string()).unwrap(),
        agent_id: Some("a1".to_string()),
    };

    let event1 = GovernanceEvent::UnitCreated {
        unit_id: "u1".into(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        timestamp: 1000,
    };
    let event2 = GovernanceEvent::UnitCreated {
        unit_id: "u2".into(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        timestamp: 2000,
    };

    storage.publish(event1).await.unwrap();
    storage.publish(event2).await.unwrap();

    let all_replayed = replay_events(api.clone(), &ctx, 500, 10).await.unwrap();
    assert_eq!(all_replayed.len(), 2);

    let partial_replayed = replay_events(api.clone(), &ctx, 1500, 10).await.unwrap();
    assert_eq!(partial_replayed.len(), 1);
    if let GovernanceEvent::UnitCreated { unit_id, .. } = &partial_replayed[0] {
        assert_eq!(unit_id, "u2");
    } else {
        panic!("Expected UnitCreated event");
    }
}
