/// Shard Manager for Managing Storage Shards
/// 
/// Manages the lifecycle of storage shards including provisioning, monitoring,
/// and load balancing across shards.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShardError {
    #[error("Shard not found: {0}")]
    NotFound(String),
    
    #[error("Shard at capacity: {0}")]
    AtCapacity(String),
    
    #[error("Failed to provision shard: {0}")]
    ProvisionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardInfo {
    pub shard_id: String,
    pub max_capacity: usize,
    pub current_tenants: usize,
    pub endpoint: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: ShardStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ShardStatus {
    Active,
    Provisioning,
    Draining,
    Offline,
}

impl ShardInfo {
    /// Check if shard has capacity
    pub fn has_capacity(&self) -> bool {
        self.current_tenants < self.max_capacity && self.status == ShardStatus::Active
    }
    
    /// Get utilization percentage
    pub fn utilization(&self) -> f64 {
        if self.max_capacity == 0 {
            return 0.0;
        }
        (self.current_tenants as f64 / self.max_capacity as f64) * 100.0
    }
}

/// Shard manager for managing storage shards
pub struct ShardManager {
    /// Available shards and their capacity
    shards: HashMap<String, ShardInfo>,
}

impl ShardManager {
    pub fn new() -> Self {
        let mut shards = HashMap::new();
        
        // Initialize shared shards
        shards.insert("shared-shard-1".to_string(), ShardInfo {
            shard_id: "shared-shard-1".to_string(),
            max_capacity: 100,
            current_tenants: 0,
            endpoint: "qdrant-shared-1:6333".to_string(),
            created_at: chrono::Utc::now(),
            status: ShardStatus::Active,
        });
        
        Self { shards }
    }
    
    /// Find best shard for a new tenant
    pub fn find_best_shard(&self) -> Option<String> {
        self.shards.values()
            .filter(|s| s.has_capacity())
            .max_by_key(|s| s.max_capacity - s.current_tenants)
            .map(|s| s.shard_id.clone())
    }
    
    /// Get shard info
    pub fn get_shard(&self, shard_id: &str) -> Option<&ShardInfo> {
        self.shards.get(shard_id)
    }
    
    /// Increment tenant count for a shard
    pub fn increment_tenant_count(&mut self, shard_id: &str) -> Result<(), ShardError> {
        let shard = self.shards.get_mut(shard_id)
            .ok_or_else(|| ShardError::NotFound(shard_id.to_string()))?;
        
        if !shard.has_capacity() {
            return Err(ShardError::AtCapacity(shard_id.to_string()));
        }
        
        shard.current_tenants += 1;
        Ok(())
    }
    
    /// Decrement tenant count for a shard
    pub fn decrement_tenant_count(&mut self, shard_id: &str) -> Result<(), ShardError> {
        let shard = self.shards.get_mut(shard_id)
            .ok_or_else(|| ShardError::NotFound(shard_id.to_string()))?;
        
        if shard.current_tenants > 0 {
            shard.current_tenants -= 1;
        }
        
        Ok(())
    }
    
    /// Create a dedicated shard for large tenant
    pub fn create_dedicated_shard(&mut self, tenant_id: &str) -> Result<String, ShardError> {
        let shard_id = format!("dedicated-{}", tenant_id);
        
        // Check if shard already exists
        if self.shards.contains_key(&shard_id) {
            return Ok(shard_id);
        }
        
        // In production, this would provision actual infrastructure
        // For now, we just create the metadata
        let shard_info = ShardInfo {
            shard_id: shard_id.clone(),
            max_capacity: 1,
            current_tenants: 1,
            endpoint: format!("qdrant-{}.svc.cluster.local:6333", shard_id),
            created_at: chrono::Utc::now(),
            status: ShardStatus::Provisioning,
        };
        
        self.shards.insert(shard_id.clone(), shard_info);
        Ok(shard_id)
    }
    
    /// Mark shard as active
    pub fn activate_shard(&mut self, shard_id: &str) -> Result<(), ShardError> {
        let shard = self.shards.get_mut(shard_id)
            .ok_or_else(|| ShardError::NotFound(shard_id.to_string()))?;
        
        shard.status = ShardStatus::Active;
        Ok(())
    }
    
    /// Start draining a shard (prepare for removal)
    pub fn drain_shard(&mut self, shard_id: &str) -> Result<(), ShardError> {
        let shard = self.shards.get_mut(shard_id)
            .ok_or_else(|| ShardError::NotFound(shard_id.to_string()))?;
        
        shard.status = ShardStatus::Draining;
        Ok(())
    }
    
    /// Remove a shard
    pub fn remove_shard(&mut self, shard_id: &str) -> Result<ShardInfo, ShardError> {
        let shard = self.shards.remove(shard_id)
            .ok_or_else(|| ShardError::NotFound(shard_id.to_string()))?;
        
        if shard.current_tenants > 0 {
            // Put it back and return error
            self.shards.insert(shard_id.to_string(), shard.clone());
            return Err(ShardError::ProvisionFailed(
                format!("Cannot remove shard {} with {} active tenants", shard_id, shard.current_tenants)
            ));
        }
        
        Ok(shard)
    }
    
    /// List all shards
    pub fn list_shards(&self) -> Vec<&ShardInfo> {
        self.shards.values().collect()
    }
    
    /// Get shard statistics
    pub fn get_statistics(&self) -> ShardStatistics {
        let total_shards = self.shards.len();
        let active_shards = self.shards.values()
            .filter(|s| s.status == ShardStatus::Active)
            .count();
        let total_capacity = self.shards.values()
            .map(|s| s.max_capacity)
            .sum();
        let total_tenants = self.shards.values()
            .map(|s| s.current_tenants)
            .sum();
        
        let avg_utilization = if total_capacity > 0 {
            (total_tenants as f64 / total_capacity as f64) * 100.0
        } else {
            0.0
        };
        
        ShardStatistics {
            total_shards,
            active_shards,
            total_capacity,
            total_tenants,
            avg_utilization,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStatistics {
    pub total_shards: usize,
    pub active_shards: usize,
    pub total_capacity: usize,
    pub total_tenants: usize,
    pub avg_utilization: f64,
}

impl Default for ShardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_shard_info_capacity() {
        let shard = ShardInfo {
            shard_id: "test".to_string(),
            max_capacity: 10,
            current_tenants: 5,
            endpoint: "localhost:6333".to_string(),
            created_at: chrono::Utc::now(),
            status: ShardStatus::Active,
        };
        
        assert!(shard.has_capacity());
        assert_eq!(shard.utilization(), 50.0);
    }
    
    #[test]
    fn test_find_best_shard() {
        let manager = ShardManager::new();
        let best = manager.find_best_shard();
        assert_eq!(best, Some("shared-shard-1".to_string()));
    }
    
    #[test]
    fn test_increment_decrement_tenant_count() {
        let mut manager = ShardManager::new();
        
        manager.increment_tenant_count("shared-shard-1").unwrap();
        assert_eq!(manager.get_shard("shared-shard-1").unwrap().current_tenants, 1);
        
        manager.decrement_tenant_count("shared-shard-1").unwrap();
        assert_eq!(manager.get_shard("shared-shard-1").unwrap().current_tenants, 0);
    }
    
    #[test]
    fn test_create_dedicated_shard() {
        let mut manager = ShardManager::new();
        let shard_id = manager.create_dedicated_shard("large-tenant").unwrap();
        
        assert_eq!(shard_id, "dedicated-large-tenant");
        assert_eq!(manager.get_shard(&shard_id).unwrap().max_capacity, 1);
        assert_eq!(manager.get_shard(&shard_id).unwrap().current_tenants, 1);
    }
    
    #[test]
    fn test_shard_lifecycle() {
        let mut manager = ShardManager::new();
        let shard_id = manager.create_dedicated_shard("test-tenant").unwrap();
        
        // Should start in provisioning state
        assert_eq!(manager.get_shard(&shard_id).unwrap().status, ShardStatus::Provisioning);
        
        // Activate it
        manager.activate_shard(&shard_id).unwrap();
        assert_eq!(manager.get_shard(&shard_id).unwrap().status, ShardStatus::Active);
        
        // Drain it
        manager.drain_shard(&shard_id).unwrap();
        assert_eq!(manager.get_shard(&shard_id).unwrap().status, ShardStatus::Draining);
    }
    
    #[test]
    fn test_statistics() {
        let manager = ShardManager::new();
        let stats = manager.get_statistics();
        
        assert_eq!(stats.total_shards, 1);
        assert_eq!(stats.active_shards, 1);
        assert_eq!(stats.total_capacity, 100);
        assert_eq!(stats.total_tenants, 0);
        assert_eq!(stats.avg_utilization, 0.0);
    }
}
