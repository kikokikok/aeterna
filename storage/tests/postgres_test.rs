//! Integration tests for PostgreSQL storage backend
//!
//! These tests use testcontainers to spin up a PostgreSQL instance.

use mk_core::traits::StorageBackend;
use mk_core::types::{TenantContext, TenantId, UserId};
use storage::postgres::{PostgresBackend, PostgresError};
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

async fn setup_postgres_container()
-> Result<(ContainerAsync<Postgres>, String), Box<dyn std::error::Error>> {
    let container = Postgres::default()
        .with_db_name("testdb")
        .with_user("testuser")
        .with_password("testpass")
        .start()
        .await?;

    let connection_url = format!(
        "postgres://testuser:testpass@localhost:{}/testdb",
        container.get_host_port_ipv4(5432).await?
    );

    Ok((container, connection_url))
}

#[tokio::test]
async fn test_postgres_backend_new() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await;
            assert!(backend.is_ok(), "Should connect to PostgreSQL");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_initialize_schema() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            let result = backend.initialize_schema().await;
            assert!(result.is_ok(), "Should initialize schema");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_store_and_retrieve() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "test_key";
            let value = b"{\"test\": \"data\"}";
            let store_result = backend.store(ctx.clone(), key, value).await;
            assert!(store_result.is_ok(), "Should store data");

            let retrieve_result = backend.retrieve(ctx, key).await;
            assert!(retrieve_result.is_ok(), "Should retrieve data");
            let retrieved = retrieve_result.unwrap();
            assert!(retrieved.is_some(), "Should have retrieved data");
            let retrieved_json: serde_json::Value =
                serde_json::from_slice(&retrieved.unwrap()).unwrap();
            let expected_json: serde_json::Value = serde_json::from_slice(value).unwrap();
            assert_eq!(
                retrieved_json, expected_json,
                "Retrieved JSON should match semantically"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_store_update() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "update_key";
            let value1 = b"{\"version\": 1}";
            backend.store(ctx.clone(), key, value1).await.unwrap();

            let value2 = b"{\"version\": 2}";
            backend.store(ctx.clone(), key, value2).await.unwrap();

            let retrieved = backend.retrieve(ctx, key).await.unwrap().unwrap();
            let retrieved_json: serde_json::Value = serde_json::from_slice(&retrieved).unwrap();
            let expected_json: serde_json::Value = serde_json::from_slice(value2).unwrap();
            assert_eq!(
                retrieved_json, expected_json,
                "Should retrieve updated value"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_delete() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "delete_key";
            let value = b"{\"to_delete\": true}";
            backend.store(ctx.clone(), key, value).await.unwrap();

            let exists_before = backend.exists(ctx.clone(), key).await.unwrap();
            assert!(exists_before, "Key should exist before delete");

            let delete_result = backend.delete(ctx.clone(), key).await;
            assert!(delete_result.is_ok(), "Should delete data");

            let exists_after = backend.exists(ctx.clone(), key).await.unwrap();
            assert!(!exists_after, "Key should not exist after delete");

            let retrieved = backend.retrieve(ctx, key).await.unwrap();
            assert!(retrieved.is_none(), "Should return None for deleted key");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_exists() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let exists = backend.exists(ctx.clone(), "nonexistent").await.unwrap();
            assert!(!exists, "Nonexistent key should not exist");

            let key = "exists_key";
            let value = b"{\"exists\": true}";
            backend.store(ctx.clone(), key, value).await.unwrap();

            let exists = backend.exists(ctx, key).await.unwrap();
            assert!(exists, "Stored key should exist");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_retrieve_nonexistent() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let result = backend.retrieve(ctx, "nonexistent_key").await;
            assert!(result.is_ok(), "Should handle nonexistent key");
            assert!(
                result.unwrap().is_none(),
                "Should return None for nonexistent key"
            );
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_invalid_json() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx = TenantContext::new(
                TenantId::new("test-tenant".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "invalid_json_key";
            let invalid_json = b"not valid json";
            let result = backend.store(ctx.clone(), key, invalid_json).await;
            assert!(result.is_ok(), "Should handle invalid JSON gracefully");

            let retrieved = backend.retrieve(ctx, key).await.unwrap();
            assert!(retrieved.is_some(), "Should retrieve something");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_tenant_isolation() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let ctx1 = TenantContext::new(
                TenantId::new("tenant-1".to_string()).unwrap(),
                UserId::default(),
            );
            let ctx2 = TenantContext::new(
                TenantId::new("tenant-2".to_string()).unwrap(),
                UserId::default(),
            );
            let key = "shared-key";
            let val1 = b"{\"tenant\": 1}";
            let val2 = b"{\"tenant\": 2}";

            // Tenant 1 stores data
            backend.store(ctx1.clone(), key, val1).await.unwrap();

            // Tenant 2 should NOT see it
            let res2 = backend.retrieve(ctx2.clone(), key).await.unwrap();
            assert!(res2.is_none(), "Tenant 2 should not see Tenant 1 data");

            // Tenant 2 stores different data for same key
            backend.store(ctx2.clone(), key, val2).await.unwrap();

            // Both should now see their own data
            let res1 = backend.retrieve(ctx1.clone(), key).await.unwrap();
            let res1_json: serde_json::Value = serde_json::from_slice(&res1.unwrap()).unwrap();
            let val1_json: serde_json::Value = serde_json::from_slice(val1).unwrap();
            assert_eq!(res1_json, val1_json);

            let res2 = backend.retrieve(ctx2.clone(), key).await.unwrap();
            let res2_json: serde_json::Value = serde_json::from_slice(&res2.unwrap()).unwrap();
            let val2_json: serde_json::Value = serde_json::from_slice(val2).unwrap();
            assert_eq!(res2_json, val2_json);

            // Deleting from Tenant 1 should NOT affect Tenant 2
            backend.delete(ctx1.clone(), key).await.unwrap();
            assert!(!backend.exists(ctx1, key).await.unwrap());
            assert!(backend.exists(ctx2, key).await.unwrap());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_connection_error() {
    let result = PostgresBackend::new("postgres://invalid:5432/invalid").await;
    assert!(result.is_err(), "Should fail with invalid connection");

    match result {
        Err(PostgresError::Database(_)) => {}
        _ => {
            panic!("Expected PostgresError::Database");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_drift_result() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let drift = mk_core::types::DriftResult {
                project_id: "proj-1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.5,
                violations: vec![],
                timestamp: chrono::Utc::now().timestamp(),
            };

            backend.store_drift_result(drift).await.unwrap();

            let result = backend
                .get_latest_drift_result(ctx.clone(), "proj-1")
                .await
                .unwrap();
            assert!(result.is_some());
            assert_eq!(result.unwrap().drift_score, 0.5);

            let none_result = backend
                .get_latest_drift_result(ctx.clone(), "nonexistent")
                .await
                .unwrap();
            assert!(none_result.is_none());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_role_management() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let user_id = UserId::new("user-1".to_string()).unwrap();

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            backend
                .assign_role(&user_id, &tenant_id, "comp-1", mk_core::types::Role::Admin)
                .await
                .unwrap();

            let roles = backend.get_user_roles(&user_id, &tenant_id).await.unwrap();
            assert_eq!(roles.len(), 1);
            assert_eq!(roles[0].0, "comp-1");
            assert_eq!(roles[0].1, mk_core::types::Role::Admin);

            backend
                .remove_role(&user_id, &tenant_id, "comp-1", mk_core::types::Role::Admin)
                .await
                .unwrap();

            let roles_after = backend.get_user_roles(&user_id, &tenant_id).await.unwrap();
            assert_eq!(roles_after.len(), 0);
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_unit_policy() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            let policy = mk_core::types::Policy {
                id: "policy-1".to_string(),
                name: "Test Policy".to_string(),
                description: None,
                layer: mk_core::types::KnowledgeLayer::Company,
                rules: vec![],
                metadata: std::collections::HashMap::new(),
                mode: mk_core::types::PolicyMode::Optional,
                merge_strategy: mk_core::types::RuleMergeStrategy::Merge,
            };

            backend
                .add_unit_policy(&ctx, "comp-1", &policy)
                .await
                .unwrap();

            let policies = backend.get_unit_policies(&ctx, "comp-1").await.unwrap();
            assert_eq!(policies.len(), 1);
            assert_eq!(policies[0].id, "policy-1");

            let nonexistent_result = backend.add_unit_policy(&ctx, "nonexistent", &policy).await;
            assert!(matches!(
                nonexistent_result,
                Err(PostgresError::NotFound(_))
            ));
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_governance_events() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let event = mk_core::types::GovernanceEvent::DriftDetected {
                project_id: "proj-1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.5,
                timestamp: chrono::Utc::now().timestamp(),
            };

            backend.log_event(&event).await.unwrap();

            let events = backend.get_governance_events(ctx, 0, 10).await.unwrap();
            assert_eq!(events.len(), 1);

            if let mk_core::types::GovernanceEvent::DriftDetected { drift_score, .. } = &events[0] {
                assert_eq!(*drift_score, 0.5);
            } else {
                panic!("Wrong event type");
            }
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_governance_event_types() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let events = vec![
                mk_core::types::GovernanceEvent::UnitCreated {
                    unit_id: "unit-1".to_string(),
                    unit_type: mk_core::types::UnitType::Company,
                    tenant_id: tenant_id.clone(),
                    parent_id: None,
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::UnitUpdated {
                    unit_id: "unit-1".to_string(),
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::UnitDeleted {
                    unit_id: "unit-1".to_string(),
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::RoleAssigned {
                    user_id: mk_core::types::UserId::new("user-1".to_string()).unwrap(),
                    unit_id: "unit-1".to_string(),
                    role: mk_core::types::Role::Admin,
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::RoleRemoved {
                    user_id: mk_core::types::UserId::new("user-1".to_string()).unwrap(),
                    unit_id: "unit-1".to_string(),
                    role: mk_core::types::Role::Admin,
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::PolicyUpdated {
                    policy_id: "policy-1".to_string(),
                    layer: mk_core::types::KnowledgeLayer::Company,
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
                mk_core::types::GovernanceEvent::PolicyDeleted {
                    policy_id: "policy-1".to_string(),
                    tenant_id: tenant_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                },
            ];

            for event in events {
                backend.log_event(&event).await.unwrap();
            }

            let retrieved_events = backend.get_governance_events(ctx, 0, 10).await.unwrap();
            assert_eq!(retrieved_events.len(), 7);
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_unit_operations() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            let retrieved = backend.get_unit(&ctx, "comp-1").await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().name, "Company 1");

            let mut updated_company = company.clone();
            updated_company.name = "Updated Company".to_string();
            updated_company.updated_at = chrono::Utc::now().timestamp();
            backend.update_unit(&ctx, &updated_company).await.unwrap();

            let updated = backend.get_unit(&ctx, "comp-1").await.unwrap();
            assert_eq!(updated.unwrap().name, "Updated Company");

            backend.delete_unit(&ctx, "comp-1").await.unwrap();
            let deleted = backend.get_unit(&ctx, "comp-1").await.unwrap();
            assert!(deleted.is_none());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_list_children() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();
            let ctx = TenantContext::new(tenant_id.clone(), UserId::default());

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            let org1 = mk_core::types::OrganizationalUnit {
                id: "org-1".to_string(),
                name: "Org 1".to_string(),
                unit_type: mk_core::types::UnitType::Organization,
                tenant_id: tenant_id.clone(),
                parent_id: Some("comp-1".to_string()),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&org1).await.unwrap();

            let org2 = mk_core::types::OrganizationalUnit {
                id: "org-2".to_string(),
                name: "Org 2".to_string(),
                unit_type: mk_core::types::UnitType::Organization,
                tenant_id: tenant_id.clone(),
                parent_id: Some("comp-1".to_string()),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&org2).await.unwrap();

            let children = backend.list_children(&ctx, "comp-1").await.unwrap();
            assert_eq!(children.len(), 2);
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_list_all_units() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            let org = mk_core::types::OrganizationalUnit {
                id: "org-1".to_string(),
                name: "Org 1".to_string(),
                unit_type: mk_core::types::UnitType::Organization,
                tenant_id: tenant_id.clone(),
                parent_id: Some("comp-1".to_string()),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&org).await.unwrap();

            let all_units = backend.list_all_units().await.unwrap();
            assert_eq!(all_units.len(), 2);
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_job_status() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let start_time = chrono::Utc::now().timestamp();
            let end_time = start_time + 100;

            backend
                .record_job_status("test_job", "test-tenant", "running", None, start_time, None)
                .await
                .unwrap();

            backend
                .record_job_status(
                    "test_job",
                    "test-tenant",
                    "completed",
                    None,
                    start_time,
                    Some(end_time),
                )
                .await
                .unwrap();

            backend
                .record_job_status(
                    "failed_job",
                    "test-tenant",
                    "failed",
                    Some("Error message"),
                    start_time,
                    Some(end_time),
                )
                .await
                .unwrap();
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_hierarchy_validation() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = TenantId::new("test-tenant".to_string()).unwrap();

            let company = mk_core::types::OrganizationalUnit {
                id: "comp-1".to_string(),
                name: "Company 1".to_string(),
                unit_type: mk_core::types::UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            backend.create_unit(&company).await.unwrap();

            let invalid_project = mk_core::types::OrganizationalUnit {
                id: "proj-1".to_string(),
                name: "Invalid Project".to_string(),
                unit_type: mk_core::types::UnitType::Project,
                tenant_id: tenant_id.clone(),
                parent_id: Some("comp-1".to_string()),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            let result = backend.create_unit(&invalid_project).await;
            assert!(result.is_err());

            let no_parent_org = mk_core::types::OrganizationalUnit {
                id: "org-no-parent".to_string(),
                name: "No Parent Org".to_string(),
                unit_type: mk_core::types::UnitType::Organization,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            let result = backend.create_unit(&no_parent_org).await;
            assert!(result.is_err());

            let invalid_parent = mk_core::types::OrganizationalUnit {
                id: "org-invalid".to_string(),
                name: "Invalid Parent Org".to_string(),
                unit_type: mk_core::types::UnitType::Organization,
                tenant_id: tenant_id.clone(),
                parent_id: Some("nonexistent".to_string()),
                metadata: std::collections::HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            };
            let result = backend.create_unit(&invalid_parent).await;
            assert!(result.is_err());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_postgres_backend_event_publisher_subscribe() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            use mk_core::traits::EventPublisher;
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let result = backend.subscribe(&["test-channel"]).await;
            assert!(result.is_err());
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL test: Docker not available");
        }
    }
}
