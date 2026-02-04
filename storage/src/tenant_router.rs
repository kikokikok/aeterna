/// Tenant Router for Shard Management
/// 
/// Routes tenants to appropriate shards based on their size and usage patterns.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TenantSize {
    /// Small tenant: < 10k memories
    Small,
    
    /// Medium tenant: 10k-100k memories
    Medium,
    
    /// Large tenant: > 100k memories
    Large,
}

impl TenantSize {
    /// Determine tenant size from memory count
    pub fn from_memory_count(count: usize) -> Self {
        match count {
            0..=10_000 => Self::Small,
            10_001..=100_000 => Self::Medium,
            _ => Self::Large,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantShard {
    pub tenant_id: String,
    pub size: TenantSize,
    pub shard_id: String,
    pub collection_name: String,
    pub assigned_at: chrono::DateTime<chrono::Utc>,
}

/// Tenant router for managing shard assignments
pub struct TenantRouter {
    /// Map of tenant_id -> shard assignment
    assignments: Arc<RwLock<HashMap<String, TenantShard>>>,
}

impl TenantRouter {
    pub fn new() -> Self {
        Self {
            assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Assign a tenant to a shard based on size
    pub fn assign_shard(&self, tenant_id: &str, size: TenantSize) -> TenantShard {
        let shard_id = match size {
            TenantSize::Small | TenantSize::Medium => {
                // Shared shard for small/medium tenants
                "shared-shard-1".to_string()
            }
            TenantSize::Large => {
                // Dedicated shard for large tenants
                format!("dedicated-{}", tenant_id)
            }
        };
        
        let collection_name = match size {
            TenantSize::Small | TenantSize::Medium => {
                // Use shared collection with tenant filtering
                "memories-shared".to_string()
            }
            TenantSize::Large => {
                // Dedicated collection
                format!("memories-{}", tenant_id)
            }
        };
        
        let shard = TenantShard {
            tenant_id: tenant_id.to_string(),
            size,
            shard_id,
            collection_name,
            assigned_at: chrono::Utc::now(),
        };
        
        self.assignments.write().insert(tenant_id.to_string(), shard.clone());
        shard
    }
    
    /// Get shard for a tenant
    pub fn get_shard(&self, tenant_id: &str) -> Option<TenantShard> {
        self.assignments.read().get(tenant_id).cloned()
    }
    
    /// Get or assign shard for a tenant
    pub fn get_or_assign(&self, tenant_id: &str, size: TenantSize) -> TenantShard {
        if let Some(shard) = self.get_shard(tenant_id) {
            shard
        } else {
            self.assign_shard(tenant_id, size)
        }
    }
    
    /// List all tenant assignments
    pub fn list_assignments(&self) -> Vec<TenantShard> {
        self.assignments.read().values().cloned().collect()
    }
    
    /// Remove tenant assignment
    pub fn remove_assignment(&self, tenant_id: &str) -> Option<TenantShard> {
        self.assignments.write().remove(tenant_id)
    }
    
    /// Get tenants by shard
    pub fn get_tenants_by_shard(&self, shard_id: &str) -> Vec<TenantShard> {
        self.assignments.read()
            .values()
            .filter(|s| s.shard_id == shard_id)
            .cloned()
            .collect()
    }
    
    /// Check if tenant should be migrated
    pub fn should_migrate(&self, tenant_id: &str, current_memory_count: usize) -> Option<TenantSize> {
        let current_shard = self.get_shard(tenant_id)?;
        let new_size = TenantSize::from_memory_count(current_memory_count);
        
        if new_size != current_shard.size {
            Some(new_size)
        } else {
            None
        }
    }
}

impl Default for TenantRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tenant_size_from_count() {
        assert_eq!(TenantSize::from_memory_count(1000), TenantSize::Small);
        assert_eq!(TenantSize::from_memory_count(50_000), TenantSize::Medium);
        assert_eq!(TenantSize::from_memory_count(200_000), TenantSize::Large);
    }
    
    #[test]
    fn test_assign_shard() {
        let router = TenantRouter::new();
        
        // Small tenant gets shared shard
        let small = router.assign_shard("tenant-small", TenantSize::Small);
        assert_eq!(small.shard_id, "shared-shard-1");
        assert_eq!(small.collection_name, "memories-shared");
        
        // Large tenant gets dedicated shard
        let large = router.assign_shard("tenant-large", TenantSize::Large);
        assert_eq!(large.shard_id, "dedicated-tenant-large");
        assert_eq!(large.collection_name, "memories-tenant-large");
    }
    
    #[test]
    fn test_get_or_assign() {
        let router = TenantRouter::new();
        
        let shard1 = router.get_or_assign("tenant-1", TenantSize::Small);
        let shard2 = router.get_or_assign("tenant-1", TenantSize::Medium);
        
        // Should return same shard (already assigned)
        assert_eq!(shard1.tenant_id, shard2.tenant_id);
        assert_eq!(shard1.shard_id, shard2.shard_id);
    }
    
    #[test]
    fn test_should_migrate() {
        let router = TenantRouter::new();
        router.assign_shard("tenant-1", TenantSize::Small);
        
        // Should not migrate if size unchanged
        assert_eq!(router.should_migrate("tenant-1", 5_000), None);
        
        // Should migrate if crossed threshold
        assert_eq!(router.should_migrate("tenant-1", 50_000), Some(TenantSize::Medium));
        assert_eq!(router.should_migrate("tenant-1", 150_000), Some(TenantSize::Large));
    }
}
