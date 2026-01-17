use mk_core::traits::StorageBackend;
use mk_core::types::{OrganizationalUnit, TenantContext, TenantId, UnitType, UserId};
use std::sync::atomic::{AtomicU32, Ordering};
use storage::postgres::PostgresBackend;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::OnceCell;

struct PostgresFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    url: String
}

static POSTGRES: OnceCell<Option<PostgresFixture>> = OnceCell::const_new();
static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn get_postgres_fixture() -> Option<&'static PostgresFixture> {
    POSTGRES
        .get_or_init(|| async {
            match Postgres::default().start().await {
                Ok(container) => {
                    let host = container.get_host().await.ok()?;
                    let port = container.get_host_port_ipv4(5432).await.ok()?;
                    let url = format!(
                        "postgres://postgres:postgres@{}:{}/postgres?sslmode=disable",
                        host, port
                    );
                    Some(PostgresFixture { container, url })
                }
                Err(_) => None
            }
        })
        .await
        .as_ref()
}

async fn create_test_backend() -> Option<PostgresBackend> {
    let fixture = get_postgres_fixture().await?;
    let backend = PostgresBackend::new(&fixture.url).await.ok()?;
    backend.initialize_schema().await.ok()?;
    Some(backend)
}

fn unique_id(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}-{}", prefix, id)
}

#[tokio::test]
async fn test_recursive_hierarchy_queries() {
    let Some(storage) = create_test_backend().await else {
        eprintln!("Skipping Postgres test: Docker not available");
        return;
    };

    let tenant_id = TenantId::new(unique_id("t")).unwrap();
    let ctx = TenantContext {
        tenant_id: tenant_id.clone(),
        user_id: UserId::new("u1".to_string()).unwrap(),
        agent_id: None
    };

    let comp_id = unique_id("comp");
    let org_id = unique_id("org");
    let team_id = unique_id("team");
    let proj_id = unique_id("proj");

    let company = OrganizationalUnit {
        id: comp_id.clone(),
        name: "Company".to_string(),
        unit_type: UnitType::Company,
        tenant_id: tenant_id.clone(),
        parent_id: None,
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000
    };
    storage.create_unit(&company).await.unwrap();

    let org1 = OrganizationalUnit {
        id: org_id.clone(),
        name: "Org 1".to_string(),
        unit_type: UnitType::Organization,
        tenant_id: tenant_id.clone(),
        parent_id: Some(comp_id.clone()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000
    };
    storage.create_unit(&org1).await.unwrap();

    let team1 = OrganizationalUnit {
        id: team_id.clone(),
        name: "Team 1".to_string(),
        unit_type: UnitType::Team,
        tenant_id: tenant_id.clone(),
        parent_id: Some(org_id.clone()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000
    };
    storage.create_unit(&team1).await.unwrap();

    let project1 = OrganizationalUnit {
        id: proj_id.clone(),
        name: "Project 1".to_string(),
        unit_type: UnitType::Project,
        tenant_id: tenant_id.clone(),
        parent_id: Some(team_id.clone()),
        metadata: std::collections::HashMap::new(),
        created_at: 1000,
        updated_at: 1000
    };
    storage.create_unit(&project1).await.unwrap();

    let descendants = StorageBackend::get_descendants(&storage, ctx.clone(), &comp_id)
        .await
        .unwrap();
    assert_eq!(descendants.len(), 3);
    let ids: Vec<String> = descendants.iter().map(|u| u.id.clone()).collect();
    assert!(ids.contains(&org_id));
    assert!(ids.contains(&team_id));
    assert!(ids.contains(&proj_id));

    let descendants = StorageBackend::get_descendants(&storage, ctx.clone(), &org_id)
        .await
        .unwrap();
    assert_eq!(descendants.len(), 2);

    let ancestors = StorageBackend::get_ancestors(&storage, ctx.clone(), &proj_id)
        .await
        .unwrap();
    assert_eq!(ancestors.len(), 3);
    let ids: Vec<String> = ancestors.iter().map(|u| u.id.clone()).collect();
    assert!(ids.contains(&team_id));
    assert!(ids.contains(&org_id));
    assert!(ids.contains(&comp_id));

    let tenant2_id = TenantId::new(unique_id("t2")).unwrap();
    let ctx2 = TenantContext {
        tenant_id: tenant2_id.clone(),
        user_id: UserId::new("u2".to_string()).unwrap(),
        agent_id: None
    };

    let ancestors_t2 = storage.get_ancestors(&ctx2, &proj_id).await.unwrap();
    assert_eq!(ancestors_t2.len(), 0);
}
