//! Integration tests for the governance event system and multi-publisher.

use mk_core::traits::{AuthorizationService, EventPublisher, StorageBackend};
use mk_core::types::{GovernanceEvent, TenantContext, TenantId, UserId};
use std::sync::Arc;
use storage::events::RedisPublisher;
use storage::postgres::PostgresBackend;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

#[tokio::test]
#[ignore = "Flaky: Redis pub/sub timing sensitive with testcontainers"]
async fn test_governance_event_propagation() {
    let result: Result<
        (
            String,
            String,
            ContainerAsync<Postgres>,
            ContainerAsync<Redis>,
        ),
        Box<dyn std::error::Error>,
    > = async {
        let (_pg_container, pg_url) = setup_postgres_test().await?;
        let (_redis_container, redis_url) = setup_redis_test().await?;
        Ok((pg_url, redis_url, _pg_container, _redis_container))
    }
    .await;

    match result {
        Ok((pg_url, redis_url, _pg_container, _redis_container)) => {
            let pg_backend = Arc::new(PostgresBackend::new(&pg_url).await.unwrap());
            pg_backend.initialize_schema().await.unwrap();

            let redis_publisher = Arc::new(RedisPublisher::new(&redis_url, "gov_events").unwrap());

            let mut rx = redis_publisher.subscribe(&["gov_events"]).await.unwrap();

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let tenant_id = TenantId::new("tenant-1".to_string()).unwrap();
            let event = GovernanceEvent::DriftDetected {
                project_id: "project-1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.75,
                timestamp: chrono::Utc::now().timestamp(),
            };

            pg_backend.publish(event.clone()).await.unwrap();
            redis_publisher.publish(event.clone()).await.unwrap();

            let received = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv())
                .await
                .expect("Timeout waiting for event")
                .expect("Channel closed");

            if let GovernanceEvent::DriftDetected { drift_score, .. } = received {
                assert_eq!(drift_score, 0.75);
            } else {
                panic!("Wrong event type received");
            }
        }
        Err(_) => {
            eprintln!("Skipping governance integration test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_full_governance_workflow() {
    let result: Result<(String, ContainerAsync<Postgres>), Box<dyn std::error::Error>> = async {
        let (_pg_container, pg_url) = setup_postgres_test().await?;
        Ok((pg_url, _pg_container))
    }
    .await;

    match result {
        Ok((pg_url, _pg_container)) => {
            let pg_backend = Arc::new(PostgresBackend::new(&pg_url).await.unwrap());
            pg_backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("comp1".to_string()).unwrap();
            let user_id = UserId::new("user1".to_string()).unwrap();
            let agent_id = "agent1".to_string();

            let company = mk_core::types::OrganizationalUnit {
                id: "comp1".into(),
                name: "Company 1".into(),
                unit_type: mk_core::types::UnitType::Company,
                parent_id: None,
                tenant_id: tenant_id.clone(),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            pg_backend.create_unit(&company).await.unwrap();

            let org = mk_core::types::OrganizationalUnit {
                id: "org1".into(),
                name: "Organization 1".into(),
                unit_type: mk_core::types::UnitType::Organization,
                parent_id: Some("comp1".into()),
                tenant_id: tenant_id.clone(),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            pg_backend.create_unit(&org).await.unwrap();

            let team = mk_core::types::OrganizationalUnit {
                id: "team1".into(),
                name: "Team 1".into(),
                unit_type: mk_core::types::UnitType::Team,
                parent_id: Some("org1".into()),
                tenant_id: tenant_id.clone(),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            pg_backend.create_unit(&team).await.unwrap();

            let project = mk_core::types::OrganizationalUnit {
                id: "proj1".into(),
                name: "Project 1".into(),
                unit_type: mk_core::types::UnitType::Project,
                parent_id: Some("team1".into()),
                tenant_id: tenant_id.clone(),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            pg_backend.create_unit(&project).await.unwrap();

            let cedar_policies = r#"
                permit(principal == User::"agent1", action == Action::"ActAs", resource == User::"user1");
                permit(principal == User::"user1", action == Action::"Update", resource == Unit::"proj1");
            "#;
            let cedar_schema = "{}";
            let authorizer =
                adapters::auth::cedar::CedarAuthorizer::new(cedar_policies, cedar_schema).unwrap();

            let ctx = TenantContext::with_agent(tenant_id.clone(), user_id.clone(), agent_id);
            let allowed = authorizer
                .check_permission(&ctx, "Update", "Unit::\"proj1\"")
                .await
                .unwrap();
            assert!(allowed);

            pg_backend
                .record_job_status(
                    "drift_scan",
                    tenant_id.as_str(),
                    "completed",
                    None,
                    chrono::Utc::now().timestamp() - 100,
                    Some(chrono::Utc::now().timestamp()),
                )
                .await
                .unwrap();

            let drift = mk_core::types::DriftResult {
                project_id: "proj1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.2,
                violations: vec![],
                timestamp: chrono::Utc::now().timestamp(),
            };
            pg_backend.store_drift_result(drift).await.unwrap();

            let engine = Arc::new(
                knowledge::governance::GovernanceEngine::new().with_storage(pg_backend.clone()),
            );
            let deployment_config = config::DeploymentConfig::default();
            let api = Arc::new(knowledge::api::GovernanceDashboardApi::new(
                engine,
                pg_backend.clone(),
                deployment_config,
            ));

            let drift_status = knowledge::api::get_drift_status(api.clone(), &ctx, "proj1")
                .await
                .unwrap();
            assert!(drift_status.is_some());
            assert_eq!(drift_status.unwrap().drift_score, 0.2);

            let jobs = knowledge::api::get_job_status(api, &ctx, Some("drift_scan"))
                .await
                .unwrap();
            assert!(jobs.as_array().unwrap().len() >= 1);
        }
        Err(_) => {
            eprintln!("Skipping governance workflow test: Docker not available");
        }
    }
}

async fn setup_postgres_test()
-> Result<(ContainerAsync<Postgres>, String), Box<dyn std::error::Error>> {
    let container = Postgres::default()
        .with_db_name("govdb")
        .with_user("govuser")
        .with_password("govpass")
        .start()
        .await?;

    let url = format!(
        "postgres://govuser:govpass@localhost:{}/govdb",
        container.get_host_port_ipv4(5432).await?
    );
    Ok((container, url))
}

async fn setup_redis_test() -> Result<(ContainerAsync<Redis>, String), Box<dyn std::error::Error>> {
    let container = Redis::default().start().await?;
    let url = format!(
        "redis://localhost:{}",
        container.get_host_port_ipv4(6379).await?
    );
    Ok((container, url))
}
