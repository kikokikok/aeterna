//! Integration tests for organizational hierarchy in PostgreSQL storage backend
//!
//! These tests verify the strict hierarchy enforcement and recursive navigation
//! logic.

use mk_core::types::{OrganizationalUnit, TenantContext, TenantId, UnitType, UserId};
use std::collections::HashMap;
use storage::postgres::PostgresBackend;
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

fn create_test_unit(
    id: &str,
    name: &str,
    unit_type: UnitType,
    parent_id: Option<String>,
    tenant_id: &str
) -> OrganizationalUnit {
    OrganizationalUnit {
        id: id.to_string(),
        name: name.to_string(),
        unit_type,
        parent_id,
        tenant_id: TenantId::new(tenant_id.to_string()).unwrap(),
        metadata: HashMap::new(),
        created_at: chrono::Utc::now().timestamp(),
        updated_at: chrono::Utc::now().timestamp()
    }
}

#[tokio::test]
async fn test_hierarchy_strict_enforcement() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = "tenant-1";

            // 1. Root must be a Company
            let org_root =
                create_test_unit("org-1", "Org Root", UnitType::Organization, None, tenant_id);
            let result = backend.create_unit(&org_root).await;
            assert!(result.is_err(), "Root unit must be a Company");

            let company =
                create_test_unit("comp-1", "Comp Root", UnitType::Company, None, tenant_id);
            backend
                .create_unit(&company)
                .await
                .expect("Company can be root");

            // 2. Organization must be under Company
            let team_under_comp = create_test_unit(
                "team-1",
                "Team Under Comp",
                UnitType::Team,
                Some("comp-1".to_string()),
                tenant_id
            );
            let result = backend.create_unit(&team_under_comp).await;
            assert!(result.is_err(), "Team cannot be directly under Company");

            let org = create_test_unit(
                "org-1",
                "Org",
                UnitType::Organization,
                Some("comp-1".to_string()),
                tenant_id
            );
            backend
                .create_unit(&org)
                .await
                .expect("Organization can be under Company");

            // 3. Team must be under Organization
            let project_under_org = create_test_unit(
                "proj-1",
                "Proj Under Org",
                UnitType::Project,
                Some("org-1".to_string()),
                tenant_id
            );
            let result = backend.create_unit(&project_under_org).await;
            assert!(
                result.is_err(),
                "Project cannot be directly under Organization"
            );

            let team = create_test_unit(
                "team-1",
                "Team",
                UnitType::Team,
                Some("org-1".to_string()),
                tenant_id
            );
            backend
                .create_unit(&team)
                .await
                .expect("Team can be under Organization");

            // 4. Project must be under Team
            let project = create_test_unit(
                "proj-1",
                "Project",
                UnitType::Project,
                Some("team-1".to_string()),
                tenant_id
            );
            backend
                .create_unit(&project)
                .await
                .expect("Project can be under Team");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL hierarchy test: Docker not available");
        }
    }
}

#[tokio::test]
async fn test_recursive_hierarchy_navigation() {
    match setup_postgres_container().await {
        Ok((_container, connection_url)) => {
            let backend = PostgresBackend::new(&connection_url).await.unwrap();
            backend.initialize_schema().await.unwrap();

            let tenant_id = "tenant-1";
            let ctx = TenantContext::new(
                TenantId::new(tenant_id.to_string()).unwrap(),
                UserId::default()
            );

            // Build hierarchy: Comp -> Org -> Team -> Proj
            let company = create_test_unit("comp-1", "Comp", UnitType::Company, None, tenant_id);
            let org = create_test_unit(
                "org-1",
                "Org",
                UnitType::Organization,
                Some("comp-1".to_string()),
                tenant_id
            );
            let team = create_test_unit(
                "team-1",
                "Team",
                UnitType::Team,
                Some("org-1".to_string()),
                tenant_id
            );
            let project = create_test_unit(
                "proj-1",
                "Proj",
                UnitType::Project,
                Some("team-1".to_string()),
                tenant_id
            );

            backend.create_unit(&company).await.unwrap();
            backend.create_unit(&org).await.unwrap();
            backend.create_unit(&team).await.unwrap();
            backend.create_unit(&project).await.unwrap();

            // Test Ancestors of Project
            let ancestors = backend.get_unit_ancestors(&ctx, "proj-1").await.unwrap();
            assert_eq!(ancestors.len(), 3);
            assert_eq!(ancestors[0].id, "team-1");
            assert_eq!(ancestors[1].id, "org-1");
            assert_eq!(ancestors[2].id, "comp-1");

            // Test Descendants of Company
            let descendants = backend.get_unit_descendants(&ctx, "comp-1").await.unwrap();
            assert_eq!(descendants.len(), 3);
            assert_eq!(descendants[0].id, "org-1");
            assert_eq!(descendants[1].id, "team-1");
            assert_eq!(descendants[2].id, "proj-1");

            // Test Descendants of Organization
            let descendants_org = backend.get_unit_descendants(&ctx, "org-1").await.unwrap();
            assert_eq!(descendants_org.len(), 2);
            assert_eq!(descendants_org[0].id, "team-1");
            assert_eq!(descendants_org[1].id, "proj-1");
        }
        Err(_) => {
            eprintln!("Skipping PostgreSQL hierarchy test: Docker not available");
        }
    }
}
