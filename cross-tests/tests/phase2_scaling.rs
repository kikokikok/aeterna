use storage::shard_manager::{ShardManager, ShardStatus};
use storage::tenant_router::{TenantRouter, TenantSize};

#[test]
fn shard_manager_and_router_assign_small_tenant_to_shared_shard() {
    let mut manager = ShardManager::new();
    let router = TenantRouter::new();

    let shard = router.assign_shard("tenant-small", TenantSize::Small);
    assert_eq!(shard.shard_id, "shared-shard-1");

    manager
        .increment_tenant_count("shared-shard-1")
        .expect("increment should succeed");

    let info = manager
        .get_shard("shared-shard-1")
        .expect("shard must exist");
    assert_eq!(info.current_tenants, 1);
    assert!(info.has_capacity());
}

#[test]
fn shard_manager_and_router_large_tenant_gets_dedicated_shard() {
    let mut manager = ShardManager::new();
    let router = TenantRouter::new();

    let shard = router.assign_shard("enterprise-co", TenantSize::Large);
    assert_eq!(shard.shard_id, "dedicated-enterprise-co");
    assert_eq!(shard.collection_name, "memories-enterprise-co");

    let dedicated_id = manager
        .create_dedicated_shard("enterprise-co")
        .expect("create dedicated shard should succeed");
    assert_eq!(dedicated_id, "dedicated-enterprise-co");

    manager
        .activate_shard(&dedicated_id)
        .expect("activate should succeed");

    let info = manager.get_shard(&dedicated_id).expect("shard must exist");
    assert_eq!(info.status, ShardStatus::Active);
}

#[test]
fn tenant_migration_detection_when_size_changes() {
    let router = TenantRouter::new();

    router.assign_shard("growing-tenant", TenantSize::Small);

    // Still small — no migration needed
    assert_eq!(router.should_migrate("growing-tenant", 5_000), None);

    // Crossed to medium
    assert_eq!(
        router.should_migrate("growing-tenant", 50_000),
        Some(TenantSize::Medium)
    );

    // Crossed to large
    assert_eq!(
        router.should_migrate("growing-tenant", 200_000),
        Some(TenantSize::Large)
    );
}

#[test]
fn shard_lifecycle_provision_activate_drain_remove() {
    let mut manager = ShardManager::new();

    let id = manager
        .create_dedicated_shard("lifecycle-tenant")
        .expect("provision should succeed");

    // Starts in Provisioning
    assert_eq!(
        manager.get_shard(&id).unwrap().status,
        ShardStatus::Provisioning
    );

    // Activate
    manager.activate_shard(&id).unwrap();
    assert_eq!(manager.get_shard(&id).unwrap().status, ShardStatus::Active);

    // Drain
    manager.drain_shard(&id).unwrap();
    assert_eq!(
        manager.get_shard(&id).unwrap().status,
        ShardStatus::Draining
    );

    // Cannot remove with active tenants (dedicated shard starts with current_tenants=1)
    let remove_result = manager.remove_shard(&id);
    assert!(
        remove_result.is_err(),
        "should not remove shard with active tenants"
    );

    // Decrement tenant count then remove
    manager.decrement_tenant_count(&id).unwrap();
    let removed = manager
        .remove_shard(&id)
        .expect("should remove empty shard");
    assert_eq!(removed.shard_id, id);
}

#[test]
fn shard_statistics_reflect_cluster_state() {
    let mut manager = ShardManager::new();

    let stats_initial = manager.get_statistics();
    assert_eq!(stats_initial.total_shards, 1);
    assert_eq!(stats_initial.active_shards, 1);
    assert_eq!(stats_initial.total_tenants, 0);
    assert_eq!(stats_initial.avg_utilization, 0.0);

    manager.increment_tenant_count("shared-shard-1").unwrap();
    manager.create_dedicated_shard("big-co").unwrap();
    manager.activate_shard("dedicated-big-co").unwrap();

    let stats = manager.get_statistics();
    assert_eq!(stats.total_shards, 2);
    assert_eq!(stats.active_shards, 2);
    assert_eq!(stats.total_tenants, 2); // 1 shared + 1 dedicated
}

#[test]
fn router_get_tenants_by_shard_returns_correct_set() {
    let router = TenantRouter::new();

    router.assign_shard("a", TenantSize::Small);
    router.assign_shard("b", TenantSize::Small);
    router.assign_shard("c", TenantSize::Large);

    let shared_tenants = router.get_tenants_by_shard("shared-shard-1");
    assert_eq!(shared_tenants.len(), 2);

    let dedicated_tenants = router.get_tenants_by_shard("dedicated-c");
    assert_eq!(dedicated_tenants.len(), 1);
    assert_eq!(dedicated_tenants[0].tenant_id, "c");
}

#[test]
fn router_get_or_assign_is_idempotent() {
    let router = TenantRouter::new();

    let first = router.get_or_assign("tenant-x", TenantSize::Small);
    let second = router.get_or_assign("tenant-x", TenantSize::Large);

    // Second call should return same assignment (idempotent)
    assert_eq!(first.shard_id, second.shard_id);
    assert_eq!(first.size, second.size);
}

#[test]
fn router_remove_assignment_then_reassign() {
    let router = TenantRouter::new();

    router.assign_shard("ephemeral", TenantSize::Small);
    assert!(router.get_shard("ephemeral").is_some());

    router.remove_assignment("ephemeral");
    assert!(router.get_shard("ephemeral").is_none());

    // Re-assign as large
    let new = router.assign_shard("ephemeral", TenantSize::Large);
    assert_eq!(new.shard_id, "dedicated-ephemeral");
}
