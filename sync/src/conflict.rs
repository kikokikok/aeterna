use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConflictError {
    #[error("No strategy available for conflict type: {0}")]
    NoStrategy(String),
    #[error("Transform failed: {0}")]
    TransformFailed(String),
    #[error("Conflict unresolvable: {0}")]
    Unresolvable(String),
}

pub type ConflictResult<T> = Result<T, ConflictError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ConflictStrategy {
    OperationalTransform,
    LastWriteWins,
}

impl std::fmt::Display for ConflictStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OperationalTransform => write!(f, "operational_transform"),
            Self::LastWriteWins => write!(f, "last_write_wins"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OperationType {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub op_type: OperationType,
    pub resource_id: String,
    pub field: Option<String>,
    pub value: Option<serde_json::Value>,
    pub client_id: String,
    pub timestamp: DateTime<Utc>,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub id: String,
    pub resource_id: String,
    pub operations: Vec<Operation>,
    pub detected_at: DateTime<Utc>,
    pub strategy_used: Option<ConflictStrategy>,
    pub resolved: bool,
    pub resolution: Option<Resolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub winning_operation: Operation,
    pub strategy: ConflictStrategy,
    pub resolved_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConflictNotification {
    #[serde(rename_all = "camelCase")]
    ConflictDetected {
        conflict_id: String,
        resource_id: String,
        involved_clients: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    #[serde(rename_all = "camelCase")]
    ConflictResolved {
        conflict_id: String,
        resource_id: String,
        strategy: ConflictStrategy,
        winner_client: String,
        timestamp: DateTime<Utc>,
    },
    #[serde(rename_all = "camelCase")]
    ConflictUnresolvable {
        conflict_id: String,
        resource_id: String,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

pub struct ConflictResolver {
    default_strategy: ConflictStrategy,
    resource_strategies: HashMap<String, ConflictStrategy>,
}

impl ConflictResolver {
    pub fn new(default_strategy: ConflictStrategy) -> Self {
        Self {
            default_strategy,
            resource_strategies: HashMap::new(),
        }
    }

    pub fn with_resource_strategy(
        mut self,
        resource_pattern: String,
        strategy: ConflictStrategy,
    ) -> Self {
        self.resource_strategies.insert(resource_pattern, strategy);
        self
    }

    pub fn set_resource_strategy(&mut self, resource_pattern: String, strategy: ConflictStrategy) {
        self.resource_strategies.insert(resource_pattern, strategy);
    }

    pub fn resolve(&self, conflict: &mut Conflict) -> ConflictResult<Resolution> {
        if conflict.operations.len() < 2 {
            return Err(ConflictError::Unresolvable(
                "Need at least 2 operations for a conflict".into(),
            ));
        }

        let strategy = self.strategy_for_resource(&conflict.resource_id);
        conflict.strategy_used = Some(strategy);

        let resolution = match strategy {
            ConflictStrategy::OperationalTransform => self.resolve_ot(conflict)?,
            ConflictStrategy::LastWriteWins => self.resolve_lww(conflict)?,
        };

        conflict.resolved = true;
        conflict.resolution = Some(resolution.clone());

        Ok(resolution)
    }

    fn strategy_for_resource(&self, resource_id: &str) -> ConflictStrategy {
        for (pattern, strategy) in &self.resource_strategies {
            if resource_id.starts_with(pattern) {
                return *strategy;
            }
        }
        self.default_strategy
    }

    fn resolve_ot(&self, conflict: &Conflict) -> ConflictResult<Resolution> {
        // Operational transform: merge non-overlapping field changes,
        // fall back to LWW for same-field conflicts.
        let ops = &conflict.operations;

        let mut field_ops: HashMap<Option<&str>, Vec<&Operation>> = HashMap::new();
        for op in ops {
            field_ops.entry(op.field.as_deref()).or_default().push(op);
        }

        // For each field group, if only one op, it wins. If multiple, LWW within that field.
        let mut winning_op: Option<&Operation> = None;
        let mut merged_fields: HashMap<String, serde_json::Value> = HashMap::new();

        for (field, field_operations) in &field_ops {
            let latest = field_operations
                .iter()
                .max_by_key(|op| op.timestamp)
                .expect("field_operations is non-empty");

            if let (Some(field_name), Some(value)) = (field, &latest.value) {
                merged_fields.insert(field_name.to_string(), value.clone());
            }

            match winning_op {
                Some(current) if latest.timestamp > current.timestamp => {
                    winning_op = Some(latest);
                }
                None => {
                    winning_op = Some(latest);
                }
                _ => {}
            }
        }

        let winner = winning_op
            .ok_or_else(|| ConflictError::TransformFailed("No operations to transform".into()))?;

        let mut metadata = HashMap::new();
        metadata.insert(
            "merged_fields".to_string(),
            serde_json::to_value(&merged_fields).unwrap_or(serde_json::Value::Null),
        );
        metadata.insert(
            "total_operations".to_string(),
            serde_json::Value::Number(ops.len().into()),
        );

        Ok(Resolution {
            winning_operation: winner.clone(),
            strategy: ConflictStrategy::OperationalTransform,
            resolved_at: Utc::now(),
            metadata,
        })
    }

    fn resolve_lww(&self, conflict: &Conflict) -> ConflictResult<Resolution> {
        let winner = conflict
            .operations
            .iter()
            .max_by_key(|op| op.timestamp)
            .ok_or_else(|| ConflictError::TransformFailed("No operations to compare".into()))?;

        let mut metadata = HashMap::new();
        metadata.insert(
            "total_operations".to_string(),
            serde_json::Value::Number(conflict.operations.len().into()),
        );

        Ok(Resolution {
            winning_operation: winner.clone(),
            strategy: ConflictStrategy::LastWriteWins,
            resolved_at: Utc::now(),
            metadata,
        })
    }

    pub fn detect_conflict(&self, ops: Vec<Operation>) -> Option<Conflict> {
        if ops.len() < 2 {
            return None;
        }

        let resource_id = &ops[0].resource_id;
        let all_same_resource = ops.iter().all(|op| op.resource_id == *resource_id);

        if !all_same_resource {
            return None;
        }

        let mut field_writers: HashMap<Option<&str>, Vec<&str>> = HashMap::new();
        for op in &ops {
            field_writers
                .entry(op.field.as_deref())
                .or_default()
                .push(&op.client_id);
        }

        let has_conflict = field_writers.values().any(|writers| {
            let unique: std::collections::HashSet<&&str> = writers.iter().collect();
            unique.len() > 1
        });

        if !has_conflict {
            return None;
        }

        Some(Conflict {
            id: uuid::Uuid::new_v4().to_string(),
            resource_id: resource_id.clone(),
            operations: ops,
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        })
    }

    pub fn create_notification(&self, conflict: &Conflict) -> ConflictNotification {
        if conflict.resolved {
            if let Some(ref resolution) = conflict.resolution {
                ConflictNotification::ConflictResolved {
                    conflict_id: conflict.id.clone(),
                    resource_id: conflict.resource_id.clone(),
                    strategy: resolution.strategy,
                    winner_client: resolution.winning_operation.client_id.clone(),
                    timestamp: Utc::now(),
                }
            } else {
                ConflictNotification::ConflictDetected {
                    conflict_id: conflict.id.clone(),
                    resource_id: conflict.resource_id.clone(),
                    involved_clients: conflict
                        .operations
                        .iter()
                        .map(|op| op.client_id.clone())
                        .collect(),
                    timestamp: Utc::now(),
                }
            }
        } else {
            ConflictNotification::ConflictDetected {
                conflict_id: conflict.id.clone(),
                resource_id: conflict.resource_id.clone(),
                involved_clients: conflict
                    .operations
                    .iter()
                    .map(|op| op.client_id.clone())
                    .collect(),
                timestamp: Utc::now(),
            }
        }
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ConflictStrategy::LastWriteWins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_operation(
        client_id: &str,
        resource_id: &str,
        field: Option<&str>,
        value: Option<serde_json::Value>,
        secs_offset: i64,
    ) -> Operation {
        Operation {
            op_type: OperationType::Update,
            resource_id: resource_id.to_string(),
            field: field.map(String::from),
            value,
            client_id: client_id.to_string(),
            timestamp: Utc::now() + chrono::Duration::seconds(secs_offset),
            version: 1,
        }
    }

    #[test]
    fn test_lww_resolution() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriteWins);

        let op1 = make_operation(
            "client-a",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("old")),
            0,
        );
        let op2 = make_operation(
            "client-b",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("new")),
            1,
        );

        let mut conflict = Conflict {
            id: "c-1".into(),
            resource_id: "mem-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let resolution = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(resolution.strategy, ConflictStrategy::LastWriteWins);
        assert_eq!(resolution.winning_operation.client_id, "client-b");
        assert!(conflict.resolved);
    }

    #[test]
    fn test_ot_resolution_different_fields() {
        let resolver = ConflictResolver::new(ConflictStrategy::OperationalTransform);

        let op1 = make_operation(
            "client-a",
            "mem-1",
            Some("title"),
            Some(serde_json::json!("Title A")),
            0,
        );
        let op2 = make_operation(
            "client-b",
            "mem-1",
            Some("body"),
            Some(serde_json::json!("Body B")),
            1,
        );

        let mut conflict = Conflict {
            id: "c-2".into(),
            resource_id: "mem-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let resolution = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(resolution.strategy, ConflictStrategy::OperationalTransform);

        let merged = resolution
            .metadata
            .get("merged_fields")
            .expect("should have merged_fields");
        let merged_obj = merged.as_object().expect("should be object");
        assert!(merged_obj.contains_key("title"));
        assert!(merged_obj.contains_key("body"));
    }

    #[test]
    fn test_ot_resolution_same_field_falls_back_to_lww() {
        let resolver = ConflictResolver::new(ConflictStrategy::OperationalTransform);

        let op1 = make_operation(
            "client-a",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("version A")),
            0,
        );
        let op2 = make_operation(
            "client-b",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("version B")),
            1,
        );

        let mut conflict = Conflict {
            id: "c-3".into(),
            resource_id: "mem-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let resolution = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(resolution.winning_operation.client_id, "client-b");
    }

    #[test]
    fn test_conflict_detection() {
        let resolver = ConflictResolver::default();

        let op1 = make_operation(
            "client-a",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("A")),
            0,
        );
        let op2 = make_operation(
            "client-b",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("B")),
            1,
        );

        let conflict = resolver.detect_conflict(vec![op1, op2]);
        assert!(conflict.is_some());
        let c = conflict.expect("should detect conflict");
        assert_eq!(c.resource_id, "mem-1");
        assert!(!c.resolved);
    }

    #[test]
    fn test_no_conflict_single_operation() {
        let resolver = ConflictResolver::default();
        let op1 = make_operation("client-a", "mem-1", Some("content"), None, 0);

        let conflict = resolver.detect_conflict(vec![op1]);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_no_conflict_different_resources() {
        let resolver = ConflictResolver::default();
        let op1 = make_operation("client-a", "mem-1", Some("content"), None, 0);
        let op2 = make_operation("client-b", "mem-2", Some("content"), None, 1);

        let conflict = resolver.detect_conflict(vec![op1, op2]);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_no_conflict_same_client_different_fields() {
        let resolver = ConflictResolver::default();
        let op1 = make_operation("client-a", "mem-1", Some("title"), None, 0);
        let op2 = make_operation("client-a", "mem-1", Some("body"), None, 1);

        let conflict = resolver.detect_conflict(vec![op1, op2]);
        assert!(conflict.is_none());
    }

    #[test]
    fn test_resource_specific_strategy() {
        let resolver = ConflictResolver::new(ConflictStrategy::LastWriteWins)
            .with_resource_strategy("policy:".into(), ConflictStrategy::OperationalTransform);

        let op1 = make_operation(
            "client-a",
            "policy:sec-1",
            Some("rules"),
            Some(serde_json::json!("A")),
            0,
        );
        let op2 = make_operation(
            "client-b",
            "policy:sec-1",
            Some("rules"),
            Some(serde_json::json!("B")),
            1,
        );

        let mut conflict = Conflict {
            id: "c-4".into(),
            resource_id: "policy:sec-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let resolution = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(resolution.strategy, ConflictStrategy::OperationalTransform);
    }

    #[test]
    fn test_resolve_insufficient_operations() {
        let resolver = ConflictResolver::default();
        let op = make_operation("client-a", "mem-1", Some("content"), None, 0);

        let mut conflict = Conflict {
            id: "c-5".into(),
            resource_id: "mem-1".into(),
            operations: vec![op],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let result = resolver.resolve(&mut conflict);
        assert!(result.is_err());
    }

    #[test]
    fn test_conflict_notification_detected() {
        let resolver = ConflictResolver::default();
        let conflict = Conflict {
            id: "c-6".into(),
            resource_id: "mem-1".into(),
            operations: vec![
                make_operation("client-a", "mem-1", Some("x"), None, 0),
                make_operation("client-b", "mem-1", Some("x"), None, 1),
            ],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let notification = resolver.create_notification(&conflict);
        match notification {
            ConflictNotification::ConflictDetected {
                involved_clients, ..
            } => {
                assert_eq!(involved_clients.len(), 2);
            }
            other => panic!("Expected ConflictDetected, got {other:?}"),
        }
    }

    #[test]
    fn test_conflict_notification_resolved() {
        let resolver = ConflictResolver::default();
        let winner_op = make_operation("client-b", "mem-1", Some("x"), None, 1);

        let conflict = Conflict {
            id: "c-7".into(),
            resource_id: "mem-1".into(),
            operations: vec![
                make_operation("client-a", "mem-1", Some("x"), None, 0),
                winner_op.clone(),
            ],
            detected_at: Utc::now(),
            strategy_used: Some(ConflictStrategy::LastWriteWins),
            resolved: true,
            resolution: Some(Resolution {
                winning_operation: winner_op,
                strategy: ConflictStrategy::LastWriteWins,
                resolved_at: Utc::now(),
                metadata: HashMap::new(),
            }),
        };

        let notification = resolver.create_notification(&conflict);
        match notification {
            ConflictNotification::ConflictResolved {
                winner_client,
                strategy,
                ..
            } => {
                assert_eq!(winner_client, "client-b");
                assert_eq!(strategy, ConflictStrategy::LastWriteWins);
            }
            other => panic!("Expected ConflictResolved, got {other:?}"),
        }
    }

    #[test]
    fn test_conflict_strategy_display() {
        assert_eq!(
            ConflictStrategy::OperationalTransform.to_string(),
            "operational_transform"
        );
        assert_eq!(
            ConflictStrategy::LastWriteWins.to_string(),
            "last_write_wins"
        );
    }

    #[test]
    fn test_conflict_serialization() {
        let op = make_operation(
            "client-a",
            "mem-1",
            Some("content"),
            Some(serde_json::json!("val")),
            0,
        );
        let conflict = Conflict {
            id: "c-8".into(),
            resource_id: "mem-1".into(),
            operations: vec![op],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let json = serde_json::to_string(&conflict).expect("serialize should succeed");
        let deserialized: Conflict =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(conflict.id, deserialized.id);
        assert_eq!(conflict.resource_id, deserialized.resource_id);
    }

    #[test]
    fn test_notification_serialization() {
        let notification = ConflictNotification::ConflictDetected {
            conflict_id: "c-1".into(),
            resource_id: "mem-1".into(),
            involved_clients: vec!["a".into(), "b".into()],
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&notification).expect("serialize should succeed");
        let deserialized: ConflictNotification =
            serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(notification, deserialized);
    }

    #[test]
    fn test_default_resolver() {
        let resolver = ConflictResolver::default();
        let op1 = make_operation("a", "r-1", Some("f"), Some(serde_json::json!(1)), 0);
        let op2 = make_operation("b", "r-1", Some("f"), Some(serde_json::json!(2)), 1);

        let mut conflict = Conflict {
            id: "c-9".into(),
            resource_id: "r-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let res = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(res.strategy, ConflictStrategy::LastWriteWins);
    }

    #[test]
    fn test_set_resource_strategy() {
        let mut resolver = ConflictResolver::default();
        resolver.set_resource_strategy("knowledge:".into(), ConflictStrategy::OperationalTransform);

        let op1 = make_operation(
            "a",
            "knowledge:entry-1",
            Some("content"),
            Some(serde_json::json!("A")),
            0,
        );
        let op2 = make_operation(
            "b",
            "knowledge:entry-1",
            Some("content"),
            Some(serde_json::json!("B")),
            1,
        );

        let mut conflict = Conflict {
            id: "c-10".into(),
            resource_id: "knowledge:entry-1".into(),
            operations: vec![op1, op2],
            detected_at: Utc::now(),
            strategy_used: None,
            resolved: false,
            resolution: None,
        };

        let res = resolver.resolve(&mut conflict).expect("should resolve");
        assert_eq!(res.strategy, ConflictStrategy::OperationalTransform);
    }

    #[test]
    fn test_conflict_error_display() {
        let err = ConflictError::NoStrategy("unknown".into());
        assert_eq!(
            err.to_string(),
            "No strategy available for conflict type: unknown"
        );

        let err = ConflictError::TransformFailed("bad input".into());
        assert_eq!(err.to_string(), "Transform failed: bad input");

        let err = ConflictError::Unresolvable("deadlock".into());
        assert_eq!(err.to_string(), "Conflict unresolvable: deadlock");
    }
}
