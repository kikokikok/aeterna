use crate::policy_translator::{
    PolicyAction, PolicySeverity, StructuredIntent, TargetType, TranslationExample
};
use mk_core::types::{
    ConstraintOperator, ConstraintSeverity, ConstraintTarget, PolicyRule, RuleType
};

pub fn all_examples() -> Vec<TranslationExample> {
    let mut examples = Vec::new();
    examples.extend(dependency_examples());
    examples.extend(file_examples());
    examples.extend(code_examples());
    examples.extend(import_examples());
    examples.extend(config_examples());
    examples
}

pub fn few_shot_examples(count: usize) -> Vec<TranslationExample> {
    all_examples().into_iter().take(count).collect()
}

fn dependency_examples() -> Vec<TranslationExample> {
    vec![
        TranslationExample {
            natural_language: "Block MySQL dependencies in all projects".to_string(),
            structured_intent: StructuredIntent {
                original: "Block MySQL dependencies in all projects".to_string(),
                interpreted: "Prohibit usage of mysql dependency".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Dependency,
                target_value: "mysql".to_string(),
                condition: None,
                severity: PolicySeverity::Block,
                confidence: 0.95
            },
            rules: vec![PolicyRule {
                id: "no-mysql".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Dependency,
                operator: ConstraintOperator::MustNotUse,
                value: serde_json::Value::String("mysql".to_string()),
                severity: ConstraintSeverity::Block,
                message: "MySQL is prohibited. Use PostgreSQL instead.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Require opentelemetry for all services".to_string(),
            structured_intent: StructuredIntent {
                original: "Require opentelemetry for all services".to_string(),
                interpreted: "Mandate opentelemetry dependency usage".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Dependency,
                target_value: "opentelemetry".to_string(),
                condition: None,
                severity: PolicySeverity::Warn,
                confidence: 0.92
            },
            rules: vec![PolicyRule {
                id: "require-otel".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Dependency,
                operator: ConstraintOperator::MustUse,
                value: serde_json::Value::String("opentelemetry".to_string()),
                severity: ConstraintSeverity::Warn,
                message: "All services must include opentelemetry for tracing.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Block lodash versions below 4.17.21 due to CVE".to_string(),
            structured_intent: StructuredIntent {
                original: "Block lodash versions below 4.17.21 due to CVE".to_string(),
                interpreted: "Prohibit vulnerable lodash versions".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Dependency,
                target_value: "lodash < 4.17.21".to_string(),
                condition: Some("version < 4.17.21".to_string()),
                severity: PolicySeverity::Block,
                confidence: 0.98
            },
            rules: vec![PolicyRule {
                id: "no-vulnerable-lodash".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Dependency,
                operator: ConstraintOperator::MustNotUse,
                value: serde_json::json!({"name": "lodash", "version": "< 4.17.21"}),
                severity: ConstraintSeverity::Block,
                message: "CVE-2021-23337: Prototype pollution in lodash < 4.17.21".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Warn about using moment.js, suggest date-fns instead".to_string(),
            structured_intent: StructuredIntent {
                original: "Warn about using moment.js, suggest date-fns instead".to_string(),
                interpreted: "Discourage moment.js dependency".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Dependency,
                target_value: "moment".to_string(),
                condition: None,
                severity: PolicySeverity::Warn,
                confidence: 0.88
            },
            rules: vec![PolicyRule {
                id: "prefer-date-fns".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Dependency,
                operator: ConstraintOperator::MustNotUse,
                value: serde_json::Value::String("moment".to_string()),
                severity: ConstraintSeverity::Warn,
                message: "moment.js is deprecated. Consider using date-fns instead.".to_string()
            }]
        },
    ]
}

fn file_examples() -> Vec<TranslationExample> {
    vec![
        TranslationExample {
            natural_language: "Every project must have a README.md file".to_string(),
            structured_intent: StructuredIntent {
                original: "Every project must have a README.md file".to_string(),
                interpreted: "Require README.md file existence".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::File,
                target_value: "README.md".to_string(),
                condition: None,
                severity: PolicySeverity::Block,
                confidence: 0.96
            },
            rules: vec![PolicyRule {
                id: "require-readme".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::File,
                operator: ConstraintOperator::MustExist,
                value: serde_json::Value::String("README.md".to_string()),
                severity: ConstraintSeverity::Block,
                message: "README.md is required for all projects.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Warn if SECURITY.md is missing".to_string(),
            structured_intent: StructuredIntent {
                original: "Warn if SECURITY.md is missing".to_string(),
                interpreted: "Recommend SECURITY.md file".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::File,
                target_value: "SECURITY.md".to_string(),
                condition: None,
                severity: PolicySeverity::Warn,
                confidence: 0.91
            },
            rules: vec![PolicyRule {
                id: "recommend-security-md".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::File,
                operator: ConstraintOperator::MustExist,
                value: serde_json::Value::String("SECURITY.md".to_string()),
                severity: ConstraintSeverity::Warn,
                message: "SECURITY.md is recommended for security documentation.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Block commits with .env files".to_string(),
            structured_intent: StructuredIntent {
                original: "Block commits with .env files".to_string(),
                interpreted: "Prohibit .env files in repository".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::File,
                target_value: ".env".to_string(),
                condition: None,
                severity: PolicySeverity::Block,
                confidence: 0.97
            },
            rules: vec![PolicyRule {
                id: "no-env-files".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::File,
                operator: ConstraintOperator::MustNotExist,
                value: serde_json::Value::String(".env".to_string()),
                severity: ConstraintSeverity::Block,
                message: ".env files must not be committed. Use .env.example instead.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Require LICENSE file in all public repositories".to_string(),
            structured_intent: StructuredIntent {
                original: "Require LICENSE file in all public repositories".to_string(),
                interpreted: "Mandate LICENSE file for public repos".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::File,
                target_value: "LICENSE".to_string(),
                condition: Some("public repository".to_string()),
                severity: PolicySeverity::Block,
                confidence: 0.94
            },
            rules: vec![PolicyRule {
                id: "require-license".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::File,
                operator: ConstraintOperator::MustExist,
                value: serde_json::Value::String("LICENSE".to_string()),
                severity: ConstraintSeverity::Block,
                message: "LICENSE file is required for public repositories.".to_string()
            }]
        },
    ]
}

fn code_examples() -> Vec<TranslationExample> {
    vec![
        TranslationExample {
            natural_language: "All functions must use Result types for error handling".to_string(),
            structured_intent: StructuredIntent {
                original: "All functions must use Result types for error handling".to_string(),
                interpreted: "Require Result<T, E> return types".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Code,
                target_value: r"Result<.*, .*>".to_string(),
                condition: None,
                severity: PolicySeverity::Warn,
                confidence: 0.89
            },
            rules: vec![PolicyRule {
                id: "use-result-types".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustMatch,
                value: serde_json::json!({"pattern": r"Result<.*, .*>", "context": "function return types"}),
                severity: ConstraintSeverity::Warn,
                message: "Use Result types for error handling instead of panics.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Block usage of unwrap() in production code".to_string(),
            structured_intent: StructuredIntent {
                original: "Block usage of unwrap() in production code".to_string(),
                interpreted: "Prohibit unwrap() calls".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Code,
                target_value: r"\.unwrap\(\)".to_string(),
                condition: Some("production code".to_string()),
                severity: PolicySeverity::Block,
                confidence: 0.93
            },
            rules: vec![PolicyRule {
                id: "no-unwrap".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({"pattern": r"\.unwrap\(\)", "exclude": ["tests", "examples"]}),
                severity: ConstraintSeverity::Block,
                message: "Do not use unwrap() in production code. Use expect() or proper error \
                          handling."
                    .to_string()
            }]
        },
        TranslationExample {
            natural_language: "Warn about TODO comments in code".to_string(),
            structured_intent: StructuredIntent {
                original: "Warn about TODO comments in code".to_string(),
                interpreted: "Flag TODO comments for review".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Code,
                target_value: r"// TODO".to_string(),
                condition: None,
                severity: PolicySeverity::Info,
                confidence: 0.85
            },
            rules: vec![PolicyRule {
                id: "track-todos".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({"pattern": r"// TODO|// FIXME|// HACK"}),
                severity: ConstraintSeverity::Info,
                message: "TODO/FIXME comments found. Consider tracking in issue tracker."
                    .to_string()
            }]
        },
        TranslationExample {
            natural_language: "Block console.log statements in TypeScript".to_string(),
            structured_intent: StructuredIntent {
                original: "Block console.log statements in TypeScript".to_string(),
                interpreted: "Prohibit console.log in production".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Code,
                target_value: r"console\.log".to_string(),
                condition: Some("TypeScript files".to_string()),
                severity: PolicySeverity::Warn,
                confidence: 0.90
            },
            rules: vec![PolicyRule {
                id: "no-console-log".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Code,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({"pattern": r"console\.log", "file_types": ["*.ts", "*.tsx"]}),
                severity: ConstraintSeverity::Warn,
                message: "Use a proper logging library instead of console.log.".to_string()
            }]
        },
    ]
}

fn import_examples() -> Vec<TranslationExample> {
    vec![
        TranslationExample {
            natural_language: "Block wildcard imports in Python".to_string(),
            structured_intent: StructuredIntent {
                original: "Block wildcard imports in Python".to_string(),
                interpreted: "Prohibit from x import * statements".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Import,
                target_value: "from * import *".to_string(),
                condition: Some("Python files".to_string()),
                severity: PolicySeverity::Block,
                confidence: 0.94
            },
            rules: vec![PolicyRule {
                id: "no-wildcard-imports".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Import,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({"pattern": r"from .+ import \*", "file_types": ["*.py"]}),
                severity: ConstraintSeverity::Block,
                message: "Wildcard imports are prohibited. Use explicit imports.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Require explicit imports for React hooks".to_string(),
            structured_intent: StructuredIntent {
                original: "Require explicit imports for React hooks".to_string(),
                interpreted: "Mandate named imports for React hooks".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Import,
                target_value: "react hooks".to_string(),
                condition: Some("React components".to_string()),
                severity: PolicySeverity::Warn,
                confidence: 0.87
            },
            rules: vec![PolicyRule {
                id: "explicit-react-imports".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Import,
                operator: ConstraintOperator::MustMatch,
                value: serde_json::json!({"pattern": r"import \{ .*(useState|useEffect|useCallback).* \} from 'react'"}),
                severity: ConstraintSeverity::Warn,
                message: "Import React hooks explicitly from 'react'.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Warn about importing internal modules from outside their package"
                .to_string(),
            structured_intent: StructuredIntent {
                original: "Warn about importing internal modules from outside their package"
                    .to_string(),
                interpreted: "Flag cross-package internal imports".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Import,
                target_value: "internal module".to_string(),
                condition: Some("cross-package".to_string()),
                severity: PolicySeverity::Warn,
                confidence: 0.82
            },
            rules: vec![PolicyRule {
                id: "no-internal-imports".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Import,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({"pattern": r"from '.*/_internal/.*'|from '.*\\.internal'"}),
                severity: ConstraintSeverity::Warn,
                message: "Do not import internal modules from outside their package.".to_string()
            }]
        },
    ]
}

fn config_examples() -> Vec<TranslationExample> {
    vec![
        TranslationExample {
            natural_language: "All API clients must specify timeouts".to_string(),
            structured_intent: StructuredIntent {
                original: "All API clients must specify timeouts".to_string(),
                interpreted: "Require timeout configuration".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Config,
                target_value: "timeout".to_string(),
                condition: Some("API client config".to_string()),
                severity: PolicySeverity::Warn,
                confidence: 0.91
            },
            rules: vec![PolicyRule {
                id: "require-timeout".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Config,
                operator: ConstraintOperator::MustMatch,
                value: serde_json::json!({"pattern": r#""timeout":\s*\d+"#, "file_types": ["*.json", "*.yaml", "*.toml"]}),
                severity: ConstraintSeverity::Warn,
                message: "API clients must specify timeout values.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Block hardcoded secrets in configuration files".to_string(),
            structured_intent: StructuredIntent {
                original: "Block hardcoded secrets in configuration files".to_string(),
                interpreted: "Prohibit secrets in config".to_string(),
                action: PolicyAction::Deny,
                target_type: TargetType::Config,
                target_value: "secret".to_string(),
                condition: None,
                severity: PolicySeverity::Block,
                confidence: 0.96
            },
            rules: vec![PolicyRule {
                id: "no-hardcoded-secrets".to_string(),
                rule_type: RuleType::Deny,
                target: ConstraintTarget::Config,
                operator: ConstraintOperator::MustNotMatch,
                value: serde_json::json!({
                    "pattern": r#"(password|secret|api_key|token)\s*[:=]\s*["'][^"']+["']"#,
                    "file_types": ["*.json", "*.yaml", "*.toml", "*.env"]
                }),
                severity: ConstraintSeverity::Block,
                message: "Do not hardcode secrets. Use environment variables or secret management."
                    .to_string()
            }]
        },
        TranslationExample {
            natural_language: "Require strict TypeScript configuration".to_string(),
            structured_intent: StructuredIntent {
                original: "Require strict TypeScript configuration".to_string(),
                interpreted: "Mandate strict mode in tsconfig".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Config,
                target_value: "strict: true".to_string(),
                condition: Some("tsconfig.json".to_string()),
                severity: PolicySeverity::Block,
                confidence: 0.93
            },
            rules: vec![PolicyRule {
                id: "strict-typescript".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Config,
                operator: ConstraintOperator::MustMatch,
                value: serde_json::json!({"pattern": r#""strict":\s*true"#, "file": "tsconfig.json"}),
                severity: ConstraintSeverity::Block,
                message: "TypeScript must be configured with strict mode enabled.".to_string()
            }]
        },
        TranslationExample {
            natural_language: "Suggest enabling ESLint rules for accessibility".to_string(),
            structured_intent: StructuredIntent {
                original: "Suggest enabling ESLint rules for accessibility".to_string(),
                interpreted: "Recommend a11y ESLint plugin".to_string(),
                action: PolicyAction::Allow,
                target_type: TargetType::Config,
                target_value: "eslint-plugin-jsx-a11y".to_string(),
                condition: Some("React projects".to_string()),
                severity: PolicySeverity::Info,
                confidence: 0.79
            },
            rules: vec![PolicyRule {
                id: "suggest-a11y-eslint".to_string(),
                rule_type: RuleType::Allow,
                target: ConstraintTarget::Config,
                operator: ConstraintOperator::MustMatch,
                value: serde_json::json!({"pattern": r#"eslint-plugin-jsx-a11y"#, "file_types": [".eslintrc*", "package.json"]}),
                severity: ConstraintSeverity::Info,
                message: "Consider enabling ESLint accessibility rules with \
                          eslint-plugin-jsx-a11y."
                    .to_string()
            }]
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_examples_not_empty() {
        let examples = all_examples();
        assert!(!examples.is_empty(), "Should have translation examples");
        assert_eq!(examples.len(), 19, "Should have 19 examples total");
    }

    #[test]
    fn test_few_shot_examples_limits_count() {
        let examples = few_shot_examples(5);
        assert_eq!(examples.len(), 5, "Should limit to requested count");

        let all = few_shot_examples(100);
        assert_eq!(
            all.len(),
            19,
            "Should return all when requested more than available"
        );
    }

    #[test]
    fn test_dependency_examples_have_valid_structure() {
        let examples = dependency_examples();
        assert_eq!(examples.len(), 4, "Should have 4 dependency examples");

        for example in examples {
            assert_eq!(
                example.structured_intent.target_type,
                TargetType::Dependency
            );
            assert!(
                !example.rules.is_empty(),
                "Each example should have at least one rule"
            );

            for rule in &example.rules {
                assert_eq!(rule.target, ConstraintTarget::Dependency);
                assert!(!rule.id.is_empty());
                assert!(!rule.message.is_empty());
            }
        }
    }

    #[test]
    fn test_file_examples_have_valid_structure() {
        let examples = file_examples();
        assert_eq!(examples.len(), 4, "Should have 4 file examples");

        for example in examples {
            assert_eq!(example.structured_intent.target_type, TargetType::File);
            assert!(
                !example.rules.is_empty(),
                "Each example should have at least one rule"
            );

            for rule in &example.rules {
                assert_eq!(rule.target, ConstraintTarget::File);
                assert!(
                    matches!(
                        rule.operator,
                        ConstraintOperator::MustExist | ConstraintOperator::MustNotExist
                    ),
                    "File rules should use MustExist or MustNotExist"
                );
            }
        }
    }

    #[test]
    fn test_code_examples_have_valid_structure() {
        let examples = code_examples();
        assert_eq!(examples.len(), 4, "Should have 4 code examples");

        for example in examples {
            assert_eq!(example.structured_intent.target_type, TargetType::Code);
            assert!(
                !example.rules.is_empty(),
                "Each example should have at least one rule"
            );

            for rule in &example.rules {
                assert_eq!(rule.target, ConstraintTarget::Code);
                assert!(
                    matches!(
                        rule.operator,
                        ConstraintOperator::MustMatch | ConstraintOperator::MustNotMatch
                    ),
                    "Code rules should use MustMatch or MustNotMatch"
                );
            }
        }
    }

    #[test]
    fn test_import_examples_have_valid_structure() {
        let examples = import_examples();
        assert_eq!(examples.len(), 3, "Should have 3 import examples");

        for example in examples {
            assert_eq!(example.structured_intent.target_type, TargetType::Import);
            assert!(
                !example.rules.is_empty(),
                "Each example should have at least one rule"
            );

            for rule in &example.rules {
                assert_eq!(rule.target, ConstraintTarget::Import);
            }
        }
    }

    #[test]
    fn test_config_examples_have_valid_structure() {
        let examples = config_examples();
        assert_eq!(examples.len(), 4, "Should have 4 config examples");

        for example in examples {
            assert_eq!(example.structured_intent.target_type, TargetType::Config);
            assert!(
                !example.rules.is_empty(),
                "Each example should have at least one rule"
            );

            for rule in &example.rules {
                assert_eq!(rule.target, ConstraintTarget::Config);
            }
        }
    }

    #[test]
    fn test_action_to_rule_type_mapping() {
        let examples = all_examples();

        for example in examples {
            let action = &example.structured_intent.action;
            for rule in &example.rules {
                match action {
                    PolicyAction::Allow => {
                        assert_eq!(
                            rule.rule_type,
                            RuleType::Allow,
                            "Allow action should map to Allow rule_type"
                        );
                    }
                    PolicyAction::Deny => {
                        assert_eq!(
                            rule.rule_type,
                            RuleType::Deny,
                            "Deny action should map to Deny rule_type"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_all_examples_have_non_empty_fields() {
        let examples = all_examples();

        for example in examples {
            assert!(!example.natural_language.is_empty());
            assert!(!example.structured_intent.original.is_empty());
            assert!(!example.structured_intent.interpreted.is_empty());
            assert!(!example.structured_intent.target_value.is_empty());
            assert!(example.structured_intent.confidence > 0.0);
            assert!(example.structured_intent.confidence <= 1.0);

            for rule in &example.rules {
                assert!(!rule.id.is_empty());
                assert!(!rule.message.is_empty());
            }
        }
    }
}
