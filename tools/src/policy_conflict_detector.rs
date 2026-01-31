//! Section 13.2: Cedar Policy Conflict Detection
//!
//! Detects conflicts between Cedar policies before deployment.

use std::collections::{HashMap, HashSet};

use cedar_policy::{Policy, PolicySet, Schema};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Conflict detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDetectionResult {
    pub valid: bool,
    pub conflicts: Vec<PolicyConflict>,
    pub warnings: Vec<PolicyWarning>,
    pub summary: ConflictSummary
}

/// Policy conflict details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConflict {
    pub conflict_type: ConflictType,
    pub policy_a_id: String,
    pub policy_b_id: Option<String>,
    pub action: String,
    pub resource: String,
    pub description: String,
    pub suggestion: String
}

/// Policy warning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyWarning {
    pub warning_type: WarningType,
    pub policy_id: String,
    pub description: String
}

/// Types of conflicts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictType {
    /// Both permit and forbid the same action/resource.
    ExplicitConflict,

    /// Policies have overlapping conditions but different effects.
    ImplicitConflict,

    /// Shadowing: one policy is never evaluated due to another.
    Shadowing,

    /// Redundant policies with identical rules.
    Redundancy
}

/// Types of warnings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WarningType {
    /// Policy has very broad scope.
    OverlyBroad,

    /// Policy has very narrow scope (may be unnecessary).
    OverlyNarrow,

    /// Missing documentation.
    MissingDocumentation
}

/// Conflict summary statistics.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConflictSummary {
    pub total_policies: usize,
    pub explicit_conflicts: usize,
    pub implicit_conflicts: usize,
    pub shadowing_issues: usize,
    pub redundancies: usize,
    pub warnings: usize
}

/// Policy conflict detector.
pub struct ConflictDetector {
    schema: Option<Schema>
}

impl ConflictDetector {
    /// Create a new conflict detector.
    pub fn new(schema: Option<Schema>) -> Self {
        Self { schema }
    }

    /// Detect conflicts in a policy set.
    pub fn detect_conflicts(
        &self,
        policies: &[ParsedPolicy]
    ) -> Result<ConflictDetectionResult, ConflictError> {
        let mut conflicts = Vec::new();
        let mut warnings = Vec::new();

        info!(
            "Starting conflict detection for {} policies",
            policies.len()
        );

        // Check for explicit conflicts (permit + forbid same action)
        self.check_explicit_conflicts(policies, &mut conflicts)?;

        // Check for implicit conflicts
        self.check_implicit_conflicts(policies, &mut conflicts)?;

        // Check for shadowing
        self.check_shadowing(policies, &mut conflicts)?;

        // Check for redundancies
        self.check_redundancies(policies, &mut conflicts)?;

        // Generate warnings
        self.generate_warnings(policies, &mut warnings)?;

        let summary = ConflictSummary {
            total_policies: policies.len(),
            explicit_conflicts: conflicts
                .iter()
                .filter(|c| c.conflict_type == ConflictType::ExplicitConflict)
                .count(),
            implicit_conflicts: conflicts
                .iter()
                .filter(|c| c.conflict_type == ConflictType::ImplicitConflict)
                .count(),
            shadowing_issues: conflicts
                .iter()
                .filter(|c| c.conflict_type == ConflictType::Shadowing)
                .count(),
            redundancies: conflicts
                .iter()
                .filter(|c| c.conflict_type == ConflictType::Redundancy)
                .count(),
            warnings: warnings.len()
        };

        let valid = conflicts.is_empty();

        if valid {
            info!("No conflicts detected in policy set");
        } else {
            warn!(
                "Detected {} conflicts: {} explicit, {} implicit, {} shadowing, {} redundant",
                conflicts.len(),
                summary.explicit_conflicts,
                summary.implicit_conflicts,
                summary.shadowing_issues,
                summary.redundancies
            );
        }

        Ok(ConflictDetectionResult {
            valid,
            conflicts,
            warnings,
            summary
        })
    }

    /// Check for explicit permit/forbid conflicts.
    fn check_explicit_conflicts(
        &self,
        policies: &[ParsedPolicy],
        conflicts: &mut Vec<PolicyConflict>
    ) -> Result<(), ConflictError> {
        // Group policies by (action, resource) pairs
        let mut action_resource_groups: HashMap<(String, String), Vec<&ParsedPolicy>> =
            HashMap::new();

        for policy in policies {
            for action in &policy.actions {
                for resource in &policy.resources {
                    let key = (action.clone(), resource.clone());
                    action_resource_groups.entry(key).or_default().push(policy);
                }
            }
        }

        // Check for conflicts within each group
        for ((action, resource), group) in action_resource_groups {
            let permits: Vec<_> = group
                .iter()
                .filter(|p| p.effect == Effect::Permit)
                .collect();
            let forbids: Vec<_> = group
                .iter()
                .filter(|p| p.effect == Effect::Forbid)
                .collect();

            if !permits.is_empty() && !forbids.is_empty() {
                // Found explicit conflict
                for permit in &permits {
                    for forbid in &forbids {
                        // Check if conditions overlap
                        if self.conditions_overlap(permit, forbid) {
                            conflicts.push(PolicyConflict {
                                conflict_type: ConflictType::ExplicitConflict,
                                policy_a_id: permit.id.clone(),
                                policy_b_id: Some(forbid.id.clone()),
                                action: action.clone(),
                                resource: resource.clone(),
                                description: format!(
                                    "Policy '{}' permits while '{}' forbids the same \
                                     action/resource",
                                    permit.id, forbid.id
                                ),
                                suggestion: format!(
                                    "Review conditions on both policies. If '{}' should take \
                                     precedence, add an 'unless' condition to '{}'",
                                    forbid.id, permit.id
                                )
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check for implicit conflicts (overlapping conditions, different
    /// effects).
    fn check_implicit_conflicts(
        &self,
        policies: &[ParsedPolicy],
        conflicts: &mut Vec<PolicyConflict>
    ) -> Result<(), ConflictError> {
        for (i, policy_a) in policies.iter().enumerate() {
            for policy_b in policies.iter().skip(i + 1) {
                if policy_a.effect == policy_b.effect {
                    continue; // Same effect can't conflict implicitly
                }

                // Check for overlapping conditions
                if self.policies_overlap(policy_a, policy_b) {
                    conflicts.push(PolicyConflict {
                        conflict_type: ConflictType::ImplicitConflict,
                        policy_a_id: policy_a.id.clone(),
                        policy_b_id: Some(policy_b.id.clone()),
                        action: policy_a.actions.join(", "),
                        resource: policy_a.resources.join(", "),
                        description: format!(
                            "Policies '{}' and '{}' have overlapping conditions but different \
                             effects",
                            policy_a.id, policy_b.id
                        ),
                        suggestion: "Review condition specificity. More specific conditions \
                                     should come first."
                            .to_string()
                    });
                }
            }
        }

        Ok(())
    }

    /// Check for shadowing (one policy makes another unreachable).
    fn check_shadowing(
        &self,
        policies: &[ParsedPolicy],
        conflicts: &mut Vec<PolicyConflict>
    ) -> Result<(), ConflictError> {
        // Sort by specificity (most specific first)
        let mut sorted: Vec<_> = policies.iter().collect();
        sorted.sort_by(|a, b| b.specificity.cmp(&a.specificity));

        for (i, policy_a) in sorted.iter().enumerate() {
            for policy_b in sorted.iter().skip(i + 1) {
                if policy_a.effect == policy_b.effect {
                    continue;
                }

                // Check if policy_a shadows policy_b
                if self.policy_shadows(policy_a, policy_b) {
                    conflicts.push(PolicyConflict {
                        conflict_type: ConflictType::Shadowing,
                        policy_a_id: policy_a.id.clone(),
                        policy_b_id: Some(policy_b.id.clone()),
                        action: policy_b.actions.join(", "),
                        resource: policy_b.resources.join(", "),
                        description: format!(
                            "Policy '{}' shadows '{}': '{}' will never be evaluated",
                            policy_a.id, policy_b.id, policy_b.id
                        ),
                        suggestion: format!(
                            "Either remove '{}' or make '{}' more specific",
                            policy_b.id, policy_b.id
                        )
                    });
                }
            }
        }

        Ok(())
    }

    /// Check for redundant policies.
    fn check_redundancies(
        &self,
        policies: &[ParsedPolicy],
        conflicts: &mut Vec<PolicyConflict>
    ) -> Result<(), ConflictError> {
        for (i, policy_a) in policies.iter().enumerate() {
            for policy_b in policies.iter().skip(i + 1) {
                if self.policies_are_equivalent(policy_a, policy_b) {
                    conflicts.push(PolicyConflict {
                        conflict_type: ConflictType::Redundancy,
                        policy_a_id: policy_a.id.clone(),
                        policy_b_id: Some(policy_b.id.clone()),
                        action: policy_a.actions.join(", "),
                        resource: policy_a.resources.join(", "),
                        description: format!(
                            "Policies '{}' and '{}' are functionally equivalent",
                            policy_a.id, policy_b.id
                        ),
                        suggestion: format!(
                            "Remove one of the policies or merge them into '{}'",
                            policy_a.id
                        )
                    });
                }
            }
        }

        Ok(())
    }

    /// Generate warnings for policies.
    fn generate_warnings(
        &self,
        policies: &[ParsedPolicy],
        warnings: &mut Vec<PolicyWarning>
    ) -> Result<(), ConflictError> {
        for policy in policies {
            // Check for overly broad policies
            if policy.actions.contains(&"*".to_string())
                || policy.resources.contains(&"*".to_string())
            {
                warnings.push(PolicyWarning {
                    warning_type: WarningType::OverlyBroad,
                    policy_id: policy.id.clone(),
                    description: "Policy uses wildcard (*) for actions or resources".to_string()
                });
            }

            // Check for overly narrow policies
            if policy.actions.len() == 1
                && policy.resources.len() == 1
                && policy.conditions.len() > 3
            {
                warnings.push(PolicyWarning {
                    warning_type: WarningType::OverlyNarrow,
                    policy_id: policy.id.clone(),
                    description: "Policy has very narrow scope with many conditions".to_string()
                });
            }

            // Check for missing documentation
            if policy.description.is_empty() || policy.description.len() < 10 {
                warnings.push(PolicyWarning {
                    warning_type: WarningType::MissingDocumentation,
                    policy_id: policy.id.clone(),
                    description: "Policy has insufficient documentation".to_string()
                });
            }
        }

        Ok(())
    }

    /// Check if two policy conditions overlap.
    fn conditions_overlap(&self, a: &ParsedPolicy, b: &ParsedPolicy) -> bool {
        // Simplified overlap detection
        // In real implementation, this would analyze Cedar conditions
        !a.conditions.is_empty() && !b.conditions.is_empty()
    }

    /// Check if two policies overlap in scope.
    fn policies_overlap(&self, a: &ParsedPolicy, b: &ParsedPolicy) -> bool {
        let actions_overlap: HashSet<_> = a.actions.iter().collect();
        let b_actions: HashSet<_> = b.actions.iter().collect();

        let resources_overlap: HashSet<_> = a.resources.iter().collect();
        let b_resources: HashSet<_> = b.resources.iter().collect();

        !actions_overlap.is_disjoint(&b_actions) && !resources_overlap.is_disjoint(&b_resources)
    }

    /// Check if policy_a shadows policy_b.
    fn policy_shadows(&self, a: &ParsedPolicy, b: &ParsedPolicy) -> bool {
        // Simplified shadowing detection
        // Policy A shadows B if A is more specific and comes first
        a.specificity > b.specificity && self.policies_overlap(a, b) && a.effect != b.effect
    }

    /// Check if two policies are equivalent.
    fn policies_are_equivalent(&self, a: &ParsedPolicy, b: &ParsedPolicy) -> bool {
        a.effect == b.effect
            && a.actions == b.actions
            && a.resources == b.resources
            && a.conditions.len() == b.conditions.len()
    }
}

/// Parsed policy representation for analysis.
#[derive(Debug, Clone)]
pub struct ParsedPolicy {
    pub id: String,
    pub effect: Effect,
    pub actions: Vec<String>,
    pub resources: Vec<String>,
    pub conditions: Vec<Condition>,
    pub specificity: u32, // Higher = more specific
    pub description: String
}

/// Policy effect.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Permit,
    Forbid
}

/// Condition representation.
#[derive(Debug, Clone)]
pub struct Condition {
    pub attribute: String,
    pub operator: String,
    pub value: String
}

/// Conflict detection error.
#[derive(Debug, Error)]
pub enum ConflictError {
    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_conflict_detection() {
        let policies = vec![
            ParsedPolicy {
                id: "policy-1".to_string(),
                effect: Effect::Permit,
                actions: vec!["read".to_string()],
                resources: vec!["document".to_string()],
                conditions: vec![],
                specificity: 1,
                description: "Allow read".to_string()
            },
            ParsedPolicy {
                id: "policy-2".to_string(),
                effect: Effect::Forbid,
                actions: vec!["read".to_string()],
                resources: vec!["document".to_string()],
                conditions: vec![],
                specificity: 1,
                description: "Deny read".to_string()
            },
        ];

        let detector = ConflictDetector::new(None);
        let result = detector.detect_conflicts(&policies).unwrap();

        assert!(!result.valid);
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(
            result.conflicts[0].conflict_type,
            ConflictType::ExplicitConflict
        );
    }

    #[test]
    fn test_redundancy_detection() {
        let policies = vec![
            ParsedPolicy {
                id: "policy-1".to_string(),
                effect: Effect::Permit,
                actions: vec!["read".to_string()],
                resources: vec!["document".to_string()],
                conditions: vec![],
                specificity: 1,
                description: "Allow read".to_string()
            },
            ParsedPolicy {
                id: "policy-2".to_string(),
                effect: Effect::Permit,
                actions: vec!["read".to_string()],
                resources: vec!["document".to_string()],
                conditions: vec![],
                specificity: 1,
                description: "Also allow read".to_string()
            },
        ];

        let detector = ConflictDetector::new(None);
        let result = detector.detect_conflicts(&policies).unwrap();

        assert!(!result.valid);
        assert!(
            result
                .conflicts
                .iter()
                .any(|c| c.conflict_type == ConflictType::Redundancy)
        );
    }
}
