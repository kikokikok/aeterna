use storage::postgres::PostgresBackend;
use mk_core::types::{OrganizationalUnit, TenantContext, TenantId, UnitType, UserId};
use mk_core::traits::StorageBackend;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

#[tokio::test]
async fn test_recursive_hierarchy_queries() {
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

    let storage = PostgresBackend::new(&conn_str).await.unwrap();
    storage.initialize_schema().await.unwrap();

    let tenant_id = TenantId::new("t1".to_string()).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: UserId::new("u1".to_string()).unwrap(),
        agent_id: None,
    };

    // Create deep hierarchy
    // Company -> Org1 -> Team1 -> Project1
    //                  -> Team2 -> Project2
    
    let company = OrganizationalUnit {
        id: "comp".to_string(),
        name: "Company".to_string(),
        unit_type: UnitType::Company,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&company).await.unwrap();

    let org1 = OrganizationalUnit {
        id: "org1".to_string(),
        name: "Org 1".to_string(),
        unit_type: UnitType::Organization,
        tenant_id: tenant_id.clone(),
        parent_id: Some("comp".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&org1).await.unwrap();

    let team1 = OrganizationalUnit {
        id: "team1".to_string(),
        name: "Team 1".to_string(),
        unit_type: UnitType::Team,
        tenant_id: tenant_id.clone(),
        parent_id: Some("org1".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&team1).await.unwrap();

    let project1 = OrganizationalUnit {
        id: "proj1".to_string(),
        name: "Project 1".to_string(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: Some("team1".to_string()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000,
    };
    storage.create_unit(&project1).await.unwrap();

    // Test Descendants from Company
    let descendants = StorageBackend::get_descendants(&storage, ctx.clone(), "comp").await.unwrap();
    assert_eq!(descendants.len(), 3);
    let ids: Vec<String> = descendants.iter().map(|u| u.id.clone()).collect();
    assert!(ids.contains(&"org1".to_string()));
    assert!(ids.contains(&"team1".to_string()));
    assert!(ids.contains(&"proj1".to_string()));

    // Test Descendants from Org1
    let descendants = StorageBackend::get_descendants(&storage, ctx.clone(), "org1").await.unwrap();
    assert_eq!(descendants.len(), 2);

    // Test Ancestors from Project1
    let ancestors = StorageBackend::get_ancestors(&storage, ctx.clone(), "proj1").await.unwrap();
    assert_eq!(ancestors.len(), 3);
    let ids: Vec<String> = ancestors.iter().map(|u| u.id.clone()).collect();
    assert!(ids.contains(&"team1".to_string()));
    assert!(ids.contains(&"org1".to_string()));
    assert!(ids.contains(&"comp".to_string()));

    // Test Isolation
    let tenant2_id = TenantId::new("t2".to_string()).unwrap();
    let ctx2 = TenantContext {
        tenant_id: tenant2_id.clone(),
        user_id: UserId::new("u2".to_string()).unwrap(),
        agent_id: None,
    };

    let ancestors_t2 = storage.get_ancestors(&ctx2, "proj1").await.unwrap();
    assert_eq!(ancestors_t2.len(), 0);
}
