use adapters::auth::cedar::CedarAuthorizer;
use mk_core::traits::AuthorizationService;
use mk_core::types::{Role, TenantContext, TenantId, UserId};

const ROLE_SCHEMA: &str = r#"{
    "": {
        "entityTypes": {
            "User": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "role": { "type": "Long" }
                    }
                }
            },
            "Unit": {},
            "Memory": {},
            "Knowledge": {},
            "SyncState": {},
            "GovernanceEvent": {}
        },
        "actions": {
            "View": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit", "Memory", "Knowledge", "SyncState", "GovernanceEvent"]
                }
            },
            "Edit": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit", "Memory", "Knowledge", "SyncState"]
                }
            },
            "Delete": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit", "Memory", "Knowledge"]
                }
            },
            "Create": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit", "Memory", "Knowledge", "SyncState"]
                }
            },
            "Promote": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Memory", "Knowledge"]
                }
            },
            "Approve": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Knowledge", "GovernanceEvent"]
                }
            },
            "Admin": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit"]
                }
            },
            "ManageRoles": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["Unit"]
                }
            },
            "ActAs": {
                "appliesTo": {
                    "principalTypes": ["User"],
                    "resourceTypes": ["User"]
                }
            }
        }
    }
}"#;

fn role_precedence(role: &Role) -> u8 {
    match role {
        Role::Agent => 0,
        Role::Developer => 1,
        Role::TechLead => 2,
        Role::Architect => 3,
        Role::Admin => 4
    }
}

fn create_ctx(tenant: &str, user: &str) -> TenantContext {
    TenantContext::new(
        TenantId::new(tenant.into()).unwrap(),
        UserId::new(user.into()).unwrap()
    )
}

fn create_agent_ctx(tenant: &str, user: &str, agent: &str) -> TenantContext {
    let mut ctx = TenantContext::new(
        TenantId::new(tenant.into()).unwrap(),
        UserId::new(user.into()).unwrap()
    );
    ctx.agent_id = Some(agent.to_string());
    ctx
}

mod rbac_matrix {
    use super::*;

    const ROLE_BASED_POLICIES: &str = r#"
        // Agent (precedence 0) - minimal permissions
        permit(principal == User::"agent", action == Action::"View", resource);
        
        // Developer (precedence 1) - view + create + edit own
        permit(principal == User::"developer", action == Action::"View", resource);
        permit(principal == User::"developer", action == Action::"Create", resource == Memory::"user-layer");
        permit(principal == User::"developer", action == Action::"Edit", resource == Memory::"user-layer");
        
        // TechLead (precedence 2) - developer + team management
        permit(principal == User::"techlead", action == Action::"View", resource);
        permit(principal == User::"techlead", action == Action::"Create", resource);
        permit(principal == User::"techlead", action == Action::"Edit", resource);
        permit(principal == User::"techlead", action == Action::"Promote", resource == Memory::"team-layer");
        
        // Architect (precedence 3) - techlead + knowledge management
        permit(principal == User::"architect", action == Action::"View", resource);
        permit(principal == User::"architect", action == Action::"Create", resource);
        permit(principal == User::"architect", action == Action::"Edit", resource);
        permit(principal == User::"architect", action == Action::"Delete", resource == Knowledge::"draft");
        permit(principal == User::"architect", action == Action::"Promote", resource);
        permit(principal == User::"architect", action == Action::"Approve", resource == Knowledge::"pending");
        
        // Admin (precedence 4) - full access
        permit(principal == User::"admin", action, resource);
    "#;

    #[tokio::test]
    async fn test_agent_positive_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "agent");

        assert!(
            authorizer
                .check_permission(&ctx, "View", "Memory::\"any\"")
                .await
                .unwrap(),
            "Agent SHOULD view memory"
        );
        assert!(
            authorizer
                .check_permission(&ctx, "View", "Knowledge::\"any\"")
                .await
                .unwrap(),
            "Agent SHOULD view knowledge"
        );
    }

    #[tokio::test]
    async fn test_agent_negative_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "agent");

        assert!(
            !authorizer
                .check_permission(&ctx, "Create", "Memory::\"user-layer\"")
                .await
                .unwrap(),
            "Agent should NOT create memory"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Edit", "Memory::\"any\"")
                .await
                .unwrap(),
            "Agent should NOT edit memory"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Delete", "Knowledge::\"any\"")
                .await
                .unwrap(),
            "Agent should NOT delete knowledge"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Agent should NOT admin units"
        );
    }

    #[tokio::test]
    async fn test_developer_positive_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            authorizer
                .check_permission(&ctx, "View", "Memory::\"any\"")
                .await
                .unwrap(),
            "Developer SHOULD view"
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Create", "Memory::\"user-layer\"")
                .await
                .unwrap(),
            "Developer SHOULD create user-layer memory"
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Edit", "Memory::\"user-layer\"")
                .await
                .unwrap(),
            "Developer SHOULD edit user-layer memory"
        );
    }

    #[tokio::test]
    async fn test_developer_negative_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&ctx, "Delete", "Memory::\"any\"")
                .await
                .unwrap(),
            "Developer should NOT delete"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Promote", "Memory::\"team-layer\"")
                .await
                .unwrap(),
            "Developer should NOT promote"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Developer should NOT admin"
        );
    }

    #[tokio::test]
    async fn test_techlead_positive_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "techlead");

        assert!(
            authorizer
                .check_permission(&ctx, "View", "Memory::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Create", "Memory::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Edit", "Memory::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Promote", "Memory::\"team-layer\"")
                .await
                .unwrap(),
            "TechLead SHOULD promote team memory"
        );
    }

    #[tokio::test]
    async fn test_techlead_negative_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "techlead");

        assert!(
            !authorizer
                .check_permission(&ctx, "Delete", "Knowledge::\"draft\"")
                .await
                .unwrap(),
            "TechLead should NOT delete knowledge"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Approve", "Knowledge::\"pending\"")
                .await
                .unwrap(),
            "TechLead should NOT approve knowledge"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "TechLead should NOT admin"
        );
    }

    #[tokio::test]
    async fn test_architect_positive_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "architect");

        assert!(
            authorizer
                .check_permission(&ctx, "View", "Knowledge::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Create", "Knowledge::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Edit", "Knowledge::\"any\"")
                .await
                .unwrap()
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Delete", "Knowledge::\"draft\"")
                .await
                .unwrap(),
            "Architect SHOULD delete draft knowledge"
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Approve", "Knowledge::\"pending\"")
                .await
                .unwrap(),
            "Architect SHOULD approve pending knowledge"
        );
        assert!(
            authorizer
                .check_permission(&ctx, "Promote", "Memory::\"any\"")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_architect_negative_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "architect");

        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Architect should NOT admin units"
        );
        assert!(
            !authorizer
                .check_permission(&ctx, "ManageRoles", "Unit::\"any\"")
                .await
                .unwrap(),
            "Architect should NOT manage roles"
        );
    }

    #[tokio::test]
    async fn test_admin_full_permissions() {
        let authorizer = CedarAuthorizer::new(ROLE_BASED_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "admin");

        let actions = [
            "View", "Create", "Edit", "Delete", "Promote", "Approve", "Admin"
        ];
        for action in actions {
            let result = authorizer
                .check_permission(&ctx, action, "Unit::\"any\"")
                .await
                .unwrap();
            assert!(result, "Admin SHOULD have {} permission", action);
        }
    }
}

mod privilege_escalation_prevention {
    use super::*;

    const ROLE_POLICIES: &str = r#"
        permit(principal == User::"developer", action == Action::"View", resource);
        permit(principal == User::"developer", action == Action::"Edit", resource == Memory::"user-layer");
        permit(principal == User::"admin", action, resource);
    "#;

    #[tokio::test]
    async fn test_developer_cannot_grant_admin_privileges() {
        let authorizer = CedarAuthorizer::new(ROLE_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&ctx, "ManageRoles", "Unit::\"any\"")
                .await
                .unwrap(),
            "Developer should NOT manage roles"
        );
    }

    #[tokio::test]
    async fn test_developer_cannot_access_admin_resources() {
        let authorizer = CedarAuthorizer::new(ROLE_POLICIES, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"admin-panel\"")
                .await
                .unwrap(),
            "Developer should NOT access admin panel"
        );
    }

    #[tokio::test]
    async fn test_agent_cannot_escalate_to_user_permissions() {
        let policies = r#"
            permit(principal == User::"agent-x", action == Action::"ActAs", resource == User::"developer");
            permit(principal == User::"developer", action == Action::"Edit", resource == Memory::"user-layer");
            permit(principal == User::"admin", action, resource);
        "#;

        let authorizer = CedarAuthorizer::new(policies, ROLE_SCHEMA).unwrap();
        let ctx = create_agent_ctx("tenant1", "developer", "agent-x");

        assert!(
            authorizer
                .check_permission(&ctx, "Edit", "Memory::\"user-layer\"")
                .await
                .unwrap(),
            "Agent with delegation SHOULD have delegated user's permissions"
        );

        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Agent should NOT escalate beyond delegated user"
        );
    }

    #[tokio::test]
    async fn test_cannot_bypass_via_resource_manipulation() {
        let policies = r#"
            permit(principal == User::"developer", action == Action::"Edit", resource == Memory::"user-layer");
        "#;

        let authorizer = CedarAuthorizer::new(policies, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&ctx, "Edit", "Memory::\"admin-layer\"")
                .await
                .unwrap(),
            "Developer should NOT edit admin-layer by spoofing resource"
        );
    }

    #[tokio::test]
    async fn test_forbid_blocks_escalation_attempt() {
        let policies = r#"
            permit(principal, action == Action::"View", resource);
            forbid(principal == User::"developer", action == Action::"Admin", resource);
        "#;

        let authorizer = CedarAuthorizer::new(policies, ROLE_SCHEMA).unwrap();
        let ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Forbid should block escalation even with general permit"
        );
    }
}

mod role_hierarchy_enforcement {
    use super::*;

    #[test]
    fn test_role_precedence_order() {
        assert!(role_precedence(&Role::Agent) < role_precedence(&Role::Developer));
        assert!(role_precedence(&Role::Developer) < role_precedence(&Role::TechLead));
        assert!(role_precedence(&Role::TechLead) < role_precedence(&Role::Architect));
        assert!(role_precedence(&Role::Architect) < role_precedence(&Role::Admin));
    }

    #[test]
    fn test_role_precedence_values() {
        assert_eq!(role_precedence(&Role::Agent), 0);
        assert_eq!(role_precedence(&Role::Developer), 1);
        assert_eq!(role_precedence(&Role::TechLead), 2);
        assert_eq!(role_precedence(&Role::Architect), 3);
        assert_eq!(role_precedence(&Role::Admin), 4);
    }

    #[tokio::test]
    async fn test_higher_role_has_superset_permissions() {
        let policies = r#"
            permit(principal == User::"developer", action == Action::"View", resource);
            permit(principal == User::"techlead", action == Action::"View", resource);
            permit(principal == User::"techlead", action == Action::"Edit", resource);
            permit(principal == User::"architect", action == Action::"View", resource);
            permit(principal == User::"architect", action == Action::"Edit", resource);
            permit(principal == User::"architect", action == Action::"Delete", resource);
            permit(principal == User::"admin", action, resource);
        "#;

        let authorizer = CedarAuthorizer::new(policies, ROLE_SCHEMA).unwrap();

        let dev_ctx = create_ctx("tenant1", "developer");
        let tl_ctx = create_ctx("tenant1", "techlead");
        let arch_ctx = create_ctx("tenant1", "architect");
        let admin_ctx = create_ctx("tenant1", "admin");

        let dev_can_view = authorizer
            .check_permission(&dev_ctx, "View", "Memory::\"data\"")
            .await
            .unwrap();
        let tl_can_view = authorizer
            .check_permission(&tl_ctx, "View", "Memory::\"data\"")
            .await
            .unwrap();
        let arch_can_view = authorizer
            .check_permission(&arch_ctx, "View", "Memory::\"data\"")
            .await
            .unwrap();
        let admin_can_view = authorizer
            .check_permission(&admin_ctx, "View", "Memory::\"data\"")
            .await
            .unwrap();

        assert!(dev_can_view);
        assert!(
            tl_can_view,
            "TechLead should have at least Developer permissions"
        );
        assert!(
            arch_can_view,
            "Architect should have at least TechLead permissions"
        );
        assert!(
            admin_can_view,
            "Admin should have at least Architect permissions"
        );
    }

    #[tokio::test]
    async fn test_lower_role_lacks_higher_permissions() {
        let policies = r#"
            permit(principal == User::"developer", action == Action::"View", resource);
            permit(principal == User::"admin", action == Action::"Admin", resource);
        "#;

        let authorizer = CedarAuthorizer::new(policies, ROLE_SCHEMA).unwrap();
        let dev_ctx = create_ctx("tenant1", "developer");

        assert!(
            !authorizer
                .check_permission(&dev_ctx, "Admin", "Unit::\"any\"")
                .await
                .unwrap(),
            "Lower role should NOT have higher role's permissions"
        );
    }
}

mod resource_action_matrix {
    use super::*;

    struct TestCase {
        role: &'static str,
        action: &'static str,
        resource: &'static str,
        expected: bool
    }

    const COMPREHENSIVE_POLICIES: &str = r#"
        // Memory resources
        permit(principal == User::"developer", action == Action::"View", resource == Memory::"user");
        permit(principal == User::"developer", action == Action::"Create", resource == Memory::"user");
        permit(principal == User::"developer", action == Action::"Edit", resource == Memory::"user");
        permit(principal == User::"techlead", action == Action::"View", resource == Memory::"team");
        permit(principal == User::"techlead", action == Action::"Create", resource == Memory::"team");
        permit(principal == User::"techlead", action == Action::"Edit", resource == Memory::"team");
        permit(principal == User::"techlead", action == Action::"Promote", resource == Memory::"team");
        
        // Knowledge resources
        permit(principal == User::"developer", action == Action::"View", resource == Knowledge::"spec");
        permit(principal == User::"architect", action == Action::"View", resource == Knowledge::"spec");
        permit(principal == User::"architect", action == Action::"Create", resource == Knowledge::"spec");
        permit(principal == User::"architect", action == Action::"Edit", resource == Knowledge::"spec");
        permit(principal == User::"architect", action == Action::"Approve", resource == Knowledge::"spec");
        
        // Governance
        permit(principal == User::"admin", action == Action::"View", resource == GovernanceEvent::"event");
        permit(principal == User::"admin", action == Action::"Approve", resource == GovernanceEvent::"event");
        
        // Unit admin
        permit(principal == User::"admin", action == Action::"Admin", resource == Unit::"org");
        permit(principal == User::"admin", action == Action::"ManageRoles", resource == Unit::"org");
    "#;

    #[tokio::test]
    async fn test_full_permission_matrix() {
        let test_cases = vec![
            // Developer + Memory
            TestCase {
                role: "developer",
                action: "View",
                resource: "Memory::\"user\"",
                expected: true
            },
            TestCase {
                role: "developer",
                action: "Create",
                resource: "Memory::\"user\"",
                expected: true
            },
            TestCase {
                role: "developer",
                action: "Edit",
                resource: "Memory::\"user\"",
                expected: true
            },
            TestCase {
                role: "developer",
                action: "Delete",
                resource: "Memory::\"user\"",
                expected: false
            },
            TestCase {
                role: "developer",
                action: "Promote",
                resource: "Memory::\"user\"",
                expected: false
            },
            // Developer + Knowledge
            TestCase {
                role: "developer",
                action: "View",
                resource: "Knowledge::\"spec\"",
                expected: true
            },
            TestCase {
                role: "developer",
                action: "Create",
                resource: "Knowledge::\"spec\"",
                expected: false
            },
            TestCase {
                role: "developer",
                action: "Edit",
                resource: "Knowledge::\"spec\"",
                expected: false
            },
            TestCase {
                role: "developer",
                action: "Approve",
                resource: "Knowledge::\"spec\"",
                expected: false
            },
            // TechLead + Memory
            TestCase {
                role: "techlead",
                action: "View",
                resource: "Memory::\"team\"",
                expected: true
            },
            TestCase {
                role: "techlead",
                action: "Create",
                resource: "Memory::\"team\"",
                expected: true
            },
            TestCase {
                role: "techlead",
                action: "Edit",
                resource: "Memory::\"team\"",
                expected: true
            },
            TestCase {
                role: "techlead",
                action: "Promote",
                resource: "Memory::\"team\"",
                expected: true
            },
            TestCase {
                role: "techlead",
                action: "Delete",
                resource: "Memory::\"team\"",
                expected: false
            },
            // Architect + Knowledge
            TestCase {
                role: "architect",
                action: "View",
                resource: "Knowledge::\"spec\"",
                expected: true
            },
            TestCase {
                role: "architect",
                action: "Create",
                resource: "Knowledge::\"spec\"",
                expected: true
            },
            TestCase {
                role: "architect",
                action: "Edit",
                resource: "Knowledge::\"spec\"",
                expected: true
            },
            TestCase {
                role: "architect",
                action: "Approve",
                resource: "Knowledge::\"spec\"",
                expected: true
            },
            TestCase {
                role: "architect",
                action: "Admin",
                resource: "Unit::\"org\"",
                expected: false
            },
            // Admin + Governance
            TestCase {
                role: "admin",
                action: "View",
                resource: "GovernanceEvent::\"event\"",
                expected: true
            },
            TestCase {
                role: "admin",
                action: "Approve",
                resource: "GovernanceEvent::\"event\"",
                expected: true
            },
            TestCase {
                role: "admin",
                action: "Admin",
                resource: "Unit::\"org\"",
                expected: true
            },
            TestCase {
                role: "admin",
                action: "ManageRoles",
                resource: "Unit::\"org\"",
                expected: true
            },
        ];

        let authorizer = CedarAuthorizer::new(COMPREHENSIVE_POLICIES, ROLE_SCHEMA).unwrap();

        for tc in test_cases {
            let ctx = create_ctx("tenant1", tc.role);
            let result = authorizer
                .check_permission(&ctx, tc.action, tc.resource)
                .await
                .unwrap();
            assert_eq!(
                result, tc.expected,
                "Role '{}' + Action '{}' + Resource '{}' => expected {}, got {}",
                tc.role, tc.action, tc.resource, tc.expected, result
            );
        }
    }
}
