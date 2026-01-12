use knowledge::governance::GovernanceEngine;
use mk_core::types::{
    ConstraintOperator, ConstraintSeverity, ConstraintTarget, KnowledgeLayer, Policy, PolicyMode,
    PolicyRule, RuleMergeStrategy, RuleType
};
use std::collections::HashMap;

#[tokio::test]
async fn test_policy_shadowing_and_inheritance() {
    let mut engine = GovernanceEngine::new();

    let company_policy = Policy {
        id: "security".to_string(),
        name: "Security".to_string(),
        description: None,
        layer: KnowledgeLayer::Company,
        mode: PolicyMode::Mandatory,
        merge_strategy: RuleMergeStrategy::Merge,
        rules: vec![PolicyRule {
            id: "no-secrets".to_string(),
            rule_type: RuleType::Allow,
            target: ConstraintTarget::Code,
            operator: ConstraintOperator::MustNotMatch,
            value: serde_json::json!("SECRET_.*"),
            severity: ConstraintSeverity::Block,
            message: "No secrets allowed".to_string()
        }],
        metadata: HashMap::new()
    };

    let org_policy = Policy {
        id: "coding-style".to_string(),
        name: "Style".to_string(),
        description: None,
        layer: KnowledgeLayer::Org,
        mode: PolicyMode::Optional,
        merge_strategy: RuleMergeStrategy::Override,
        rules: vec![PolicyRule {
            id: "indentation".to_string(),
            rule_type: RuleType::Allow,
            target: ConstraintTarget::Code,
            operator: ConstraintOperator::MustMatch,
            value: serde_json::json!(r"^  \S"),
            severity: ConstraintSeverity::Warn,
            message: "Use 2 spaces".to_string()
        }],
        metadata: HashMap::new()
    };

    engine.add_policy(company_policy);
    engine.add_policy(org_policy);

    let project_id = "proj-1";
    let mut context = HashMap::new();
    context.insert("projectId".to_string(), serde_json::json!(project_id));
    context.insert("content".to_string(), serde_json::json!("    fn main() {}"));

    let result = engine.validate(KnowledgeLayer::Project, &context);
    assert!(!result.is_valid);

    let project_policy = Policy {
        id: "coding-style".to_string(),
        name: "Project Style".to_string(),
        description: None,
        layer: KnowledgeLayer::Project,
        mode: PolicyMode::Optional,
        merge_strategy: RuleMergeStrategy::Override,
        rules: vec![PolicyRule {
            id: "indentation".to_string(),
            rule_type: RuleType::Allow,
            target: ConstraintTarget::Code,
            operator: ConstraintOperator::MustMatch,
            value: serde_json::json!(r"^    \S"),
            severity: ConstraintSeverity::Warn,
            message: "Use 4 spaces".to_string()
        }],
        metadata: HashMap::new()
    };

    engine.add_policy(project_policy);

    let result = engine.validate(KnowledgeLayer::Project, &context);
    assert!(
        result.is_valid,
        "Project policy should have overridden Org policy"
    );

    let project_security_override = Policy {
        id: "security".to_string(),
        name: "Insecure Project".to_string(),
        description: None,
        layer: KnowledgeLayer::Project,
        mode: PolicyMode::Optional,
        merge_strategy: RuleMergeStrategy::Override,
        rules: vec![],
        metadata: HashMap::new()
    };

    engine.add_policy(project_security_override);

    let mut context_with_secret = HashMap::new();
    context_with_secret.insert("projectId".to_string(), serde_json::json!(project_id));
    context_with_secret.insert("content".to_string(), serde_json::json!("SECRET_KEY=123"));

    let result = engine.validate(KnowledgeLayer::Project, &context_with_secret);
    assert!(
        !result.is_valid,
        "Company mandatory policy should NOT be overridable"
    );
}

#[tokio::test]
async fn test_rule_type_deny_precedence() {
    let mut engine = GovernanceEngine::new();

    let company_policy = Policy {
        id: "lib-checks".to_string(),
        name: "Lib Checks".to_string(),
        description: None,
        layer: KnowledgeLayer::Company,
        mode: PolicyMode::Optional,
        merge_strategy: RuleMergeStrategy::Merge,
        rules: vec![PolicyRule {
            id: "deny-jquery".to_string(),
            rule_type: RuleType::Deny,
            target: ConstraintTarget::Dependency,
            operator: ConstraintOperator::MustUse,
            value: serde_json::json!("jquery"),
            severity: ConstraintSeverity::Block,
            message: "JQuery is forbidden".to_string()
        }],
        metadata: HashMap::new()
    };

    engine.add_policy(company_policy);

    let mut context = HashMap::new();
    context.insert("dependencies".to_string(), serde_json::json!(["jquery"]));

    let result = engine.validate(KnowledgeLayer::Project, &context);
    assert!(!result.is_valid);
    assert_eq!(result.violations[0].rule_id, "deny-jquery");
}
