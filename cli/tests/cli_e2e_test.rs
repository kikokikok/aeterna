use assert_cmd::{Command, cargo_bin_cmd};

fn aeterna() -> Command {
    cargo_bin_cmd!("aeterna")
}

mod help_and_version {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_help_flag() {
        aeterna()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"))
            .stdout(predicate::str::contains("Commands:"));
    }

    #[test]
    fn test_short_help_flag() {
        aeterna()
            .arg("-h")
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:"));
    }

    #[test]
    fn test_version_flag() {
        aeterna()
            .arg("--version")
            .assert()
            .success()
            .stdout(predicate::str::contains("aeterna"));
    }

    #[test]
    fn test_short_version_flag() {
        aeterna()
            .arg("-V")
            .assert()
            .success()
            .stdout(predicate::str::contains("aeterna"));
    }

    #[test]
    fn test_no_args_shows_help() {
        aeterna()
            .assert()
            .failure()
            .stderr(predicate::str::contains("Usage:"));
    }
}

mod memory_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_memory_help() {
        aeterna()
            .args(["memory", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("memor"))
            .stdout(predicate::str::contains("add"))
            .stdout(predicate::str::contains("search"))
            .stdout(predicate::str::contains("list"));
    }

    #[test]
    fn test_memory_add_help() {
        aeterna()
            .args(["memory", "add", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Add a new memory"))
            .stdout(predicate::str::contains("--layer"));
    }

    #[test]
    fn test_memory_add_dry_run() {
        aeterna()
            .args(["memory", "add", "Test memory content", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_memory_add_dry_run_json() {
        aeterna()
            .args([
                "memory",
                "add",
                "Test memory content",
                "--dry-run",
                "--json"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"dryRun\""))
            .stdout(predicate::str::contains("\"content\""));
    }

    #[test]
    fn test_memory_add_with_layer() {
        aeterna()
            .args([
                "memory",
                "add",
                "Project specific memory",
                "--layer",
                "project",
                "--dry-run"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("project"));
    }

    #[test]
    fn test_memory_add_invalid_layer() {
        aeterna()
            .args(["memory", "add", "Test", "--layer", "invalid-layer"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("Invalid")));
    }

    #[test]
    fn test_memory_search_help() {
        aeterna()
            .args(["memory", "search", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Search memories"))
            .stdout(predicate::str::contains("--limit"));
    }

    #[test]
    fn test_memory_search_dry_run() {
        aeterna()
            .args(["memory", "search", "test query", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_memory_list_help() {
        aeterna()
            .args(["memory", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List memories"));
    }

    #[test]
    fn test_memory_delete_help() {
        aeterna()
            .args(["memory", "delete", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Delete"));
    }

    #[test]
    fn test_memory_feedback_help() {
        aeterna()
            .args(["memory", "feedback", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("feedback"));
    }

    #[test]
    fn test_memory_promote_help() {
        aeterna()
            .args(["memory", "promote", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Promote"));
    }
}

mod knowledge_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_knowledge_help() {
        aeterna()
            .args(["knowledge", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("knowledge"))
            .stdout(predicate::str::contains("search"))
            .stdout(predicate::str::contains("get"));
    }

    #[test]
    fn test_knowledge_search_help() {
        aeterna()
            .args(["knowledge", "search", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Search"));
    }

    #[test]
    fn test_knowledge_search_dry_run() {
        aeterna()
            .args(["knowledge", "search", "architecture decisions", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_knowledge_get_help() {
        aeterna()
            .args(["knowledge", "get", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Get"));
    }

    #[test]
    fn test_knowledge_check_help() {
        aeterna()
            .args(["knowledge", "check", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Check"));
    }

    #[test]
    fn test_knowledge_propose_help() {
        aeterna()
            .args(["knowledge", "propose", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Propose"));
    }
}

mod user_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_user_help() {
        aeterna()
            .args(["user", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("user"))
            .stdout(predicate::str::contains("register"))
            .stdout(predicate::str::contains("whoami"));
    }

    #[test]
    fn test_user_whoami() {
        aeterna()
            .args(["user", "whoami"])
            .assert()
            .success()
            .stdout(predicate::str::contains("User"));
    }

    #[test]
    fn test_user_whoami_json() {
        aeterna()
            .args(["user", "whoami", "--json"])
            .assert()
            .success()
            .stdout(predicate::str::contains("\"user\""))
            .stdout(predicate::str::contains("\"tenant\""));
    }

    #[test]
    fn test_user_register_help() {
        aeterna()
            .args(["user", "register", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Register"));
    }

    #[test]
    fn test_user_register_dry_run() {
        aeterna()
            .args(["user", "register", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_user_list_help() {
        aeterna()
            .args(["user", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List"));
    }

    #[test]
    fn test_user_roles_help() {
        aeterna()
            .args(["user", "roles", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("role"));
    }

    #[test]
    fn test_user_invite_help() {
        aeterna()
            .args(["user", "invite", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Invite"));
    }

    #[test]
    fn test_user_invite_invalid_email() {
        aeterna()
            .args([
                "user",
                "invite",
                "not-an-email",
                "--org",
                "test-org",
                "--yes"
            ])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("Invalid email").or(predicate::str::contains("invalid"))
            );
    }

    #[test]
    fn test_user_invite_invalid_role() {
        aeterna()
            .args([
                "user",
                "invite",
                "test@example.com",
                "--org",
                "test-org",
                "--role",
                "superuser",
                "--yes"
            ])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("Invalid role").or(predicate::str::contains("invalid"))
            );
    }
}

mod admin_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_admin_help() {
        aeterna()
            .args(["admin", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("admin"))
            .stdout(predicate::str::contains("health"))
            .stdout(predicate::str::contains("validate"))
            .stdout(predicate::str::contains("migrate"));
    }

    #[test]
    fn test_admin_health_help() {
        aeterna()
            .args(["admin", "health", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("health"))
            .stdout(predicate::str::contains("--component"));
    }

    #[test]
    fn test_admin_validate_help() {
        aeterna()
            .args(["admin", "validate", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Validate"))
            .stdout(predicate::str::contains("--strict"));
    }

    #[test]
    fn test_admin_migrate_help() {
        aeterna()
            .args(["admin", "migrate", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("migration"))
            .stdout(predicate::str::contains("--dry-run"));
    }

    #[test]
    fn test_admin_migrate_status() {
        aeterna()
            .args(["admin", "migrate", "status"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Migration"));
    }

    #[test]
    fn test_admin_drift_help() {
        aeterna()
            .args(["admin", "drift", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("drift"))
            .stdout(predicate::str::contains("--fix"));
    }

    #[test]
    fn test_admin_export_help() {
        aeterna()
            .args(["admin", "export", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Export"))
            .stdout(predicate::str::contains("--format"));
    }

    #[test]
    fn test_admin_import_help() {
        aeterna()
            .args(["admin", "import", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Import"))
            .stdout(predicate::str::contains("--mode"));
    }
}

mod policy_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_policy_help() {
        aeterna()
            .args(["policy", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("policy"))
            .stdout(predicate::str::contains("create"))
            .stdout(predicate::str::contains("list"))
            .stdout(predicate::str::contains("explain"));
    }

    #[test]
    fn test_policy_create_help() {
        aeterna()
            .args(["policy", "create", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Create"))
            .stdout(predicate::str::contains("--description"))
            .stdout(predicate::str::contains("--template"));
    }

    #[test]
    fn test_policy_create_dry_run_with_description() {
        aeterna()
            .args([
                "policy",
                "create",
                "--description",
                "Block critical CVEs",
                "--dry-run"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_policy_create_dry_run_with_template() {
        aeterna()
            .args([
                "policy",
                "create",
                "--template",
                "security-baseline",
                "--dry-run"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"))
            .stdout(predicate::str::contains("security-baseline"));
    }

    #[test]
    fn test_policy_create_missing_description_or_template() {
        aeterna()
            .args(["policy", "create"])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("description").or(predicate::str::contains("template"))
            );
    }

    #[test]
    fn test_policy_create_invalid_layer() {
        aeterna()
            .args([
                "policy",
                "create",
                "--description",
                "Test",
                "--layer",
                "invalid"
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("Invalid")));
    }

    #[test]
    fn test_policy_create_invalid_mode() {
        aeterna()
            .args([
                "policy",
                "create",
                "--description",
                "Test",
                "--mode",
                "required"
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("Invalid")));
    }

    #[test]
    fn test_policy_create_invalid_severity() {
        aeterna()
            .args([
                "policy",
                "create",
                "--description",
                "Test",
                "--severity",
                "critical"
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("Invalid")));
    }

    #[test]
    fn test_policy_list_help() {
        aeterna()
            .args(["policy", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List"))
            .stdout(predicate::str::contains("--layer"));
    }

    #[test]
    fn test_policy_explain_help() {
        aeterna()
            .args(["policy", "explain", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Explain"))
            .stdout(predicate::str::contains("--verbose"));
    }

    #[test]
    fn test_policy_simulate_help() {
        aeterna()
            .args(["policy", "simulate", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Simulate"))
            .stdout(predicate::str::contains("--scenario"));
    }

    #[test]
    fn test_policy_validate_help() {
        aeterna()
            .args(["policy", "validate", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Validate"))
            .stdout(predicate::str::contains("--strict"));
    }

    #[test]
    fn test_policy_draft_help() {
        aeterna()
            .args(["policy", "draft", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("draft"))
            .stdout(predicate::str::contains("--list"));
    }
}

mod govern_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_govern_help() {
        aeterna()
            .args(["govern", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("govern"))
            .stdout(predicate::str::contains("status"))
            .stdout(predicate::str::contains("pending"))
            .stdout(predicate::str::contains("approve"));
    }

    #[test]
    fn test_govern_status_help() {
        aeterna()
            .args(["govern", "status", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("status"));
    }

    #[test]
    fn test_govern_pending_help() {
        aeterna()
            .args(["govern", "pending", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("pending"))
            .stdout(predicate::str::contains("request-type").or(predicate::str::contains("-t")));
    }

    #[test]
    fn test_govern_approve_help() {
        aeterna()
            .args(["govern", "approve", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Approve"));
    }

    #[test]
    fn test_govern_reject_help() {
        aeterna()
            .args(["govern", "reject", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Reject"));
    }

    #[test]
    fn test_govern_configure_help() {
        aeterna()
            .args(["govern", "configure", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Configure"))
            .stdout(predicate::str::contains("--template"));
    }

    #[test]
    fn test_govern_roles_help() {
        aeterna()
            .args(["govern", "roles", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("role"));
    }

    #[test]
    fn test_govern_audit_help() {
        aeterna()
            .args(["govern", "audit", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("audit"))
            .stdout(predicate::str::contains("--export"));
    }

    #[test]
    fn test_govern_status() {
        aeterna()
            .args(["govern", "status"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Status"));
    }

    #[test]
    fn test_govern_status_json() {
        let output = aeterna()
            .args(["govern", "status", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("config").is_some());
        assert!(json.get("metrics").is_some());
    }

    #[test]
    fn test_govern_status_verbose() {
        aeterna()
            .args(["govern", "status", "--verbose"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Status"))
            .stdout(predicate::str::contains("Recent Activity"));
    }

    #[test]
    fn test_govern_pending() {
        aeterna()
            .args(["govern", "pending"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Pending Requests"));
    }

    #[test]
    fn test_govern_pending_json() {
        let output = aeterna()
            .args(["govern", "pending", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("total").is_some());
        assert!(json.get("requests").is_some());
    }

    #[test]
    fn test_govern_pending_filter_by_type() {
        aeterna()
            .args(["govern", "pending", "-t", "policy"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Pending Requests"));
    }

    #[test]
    fn test_govern_pending_filter_by_layer() {
        aeterna()
            .args(["govern", "pending", "--layer", "org"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Pending Requests"));
    }

    #[test]
    fn test_govern_pending_filter_by_requestor() {
        aeterna()
            .args(["govern", "pending", "--requestor", "alice"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Pending Requests"));
    }

    #[test]
    fn test_govern_approve() {
        aeterna()
            .args(["govern", "approve", "req_test123", "--yes"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Approve Request"))
            .stdout(predicate::str::contains("approved"));
    }

    #[test]
    fn test_govern_approve_json() {
        let output = aeterna()
            .args(["govern", "approve", "req_test123", "--yes", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.get("action").and_then(|v| v.as_str()),
            Some("approved")
        );
    }

    #[test]
    fn test_govern_approve_with_comment() {
        aeterna()
            .args([
                "govern",
                "approve",
                "req_test123",
                "--yes",
                "--comment",
                "LGTM"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Approve Request"))
            .stdout(predicate::str::contains("Comment: LGTM"));
    }

    #[test]
    fn test_govern_reject() {
        aeterna()
            .args([
                "govern",
                "reject",
                "req_test123",
                "--reason",
                "Needs security review",
                "--yes"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Reject Request"))
            .stdout(predicate::str::contains("rejected"));
    }

    #[test]
    fn test_govern_reject_json() {
        let output = aeterna()
            .args([
                "govern",
                "reject",
                "req_test123",
                "--reason",
                "Needs review",
                "--yes",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.get("action").and_then(|v| v.as_str()),
            Some("rejected")
        );
    }

    #[test]
    fn test_govern_reject_requires_reason() {
        aeterna()
            .args(["govern", "reject", "req_test123", "--reason", "", "--yes"])
            .assert()
            .failure();
    }

    #[test]
    fn test_govern_configure_show() {
        aeterna()
            .args(["govern", "configure", "--show"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Configuration"));
    }

    #[test]
    fn test_govern_configure_show_json() {
        let output = aeterna()
            .args(["govern", "configure", "--show", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("approval_mode").is_some());
        assert!(json.get("min_approvers").is_some());
    }

    #[test]
    fn test_govern_configure_list_templates() {
        aeterna()
            .args(["govern", "configure", "--list-templates"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Available Governance Templates"))
            .stdout(predicate::str::contains("standard"))
            .stdout(predicate::str::contains("strict"))
            .stdout(predicate::str::contains("permissive"));
    }

    #[test]
    fn test_govern_configure_template_standard() {
        aeterna()
            .args(["govern", "configure", "--template", "standard"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Update Governance Configuration"));
    }

    #[test]
    fn test_govern_configure_template_strict() {
        aeterna()
            .args(["govern", "configure", "--template", "strict"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Update Governance Configuration"));
    }

    #[test]
    fn test_govern_configure_template_permissive() {
        aeterna()
            .args(["govern", "configure", "--template", "permissive"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Update Governance Configuration"));
    }

    #[test]
    fn test_govern_configure_approval_mode() {
        aeterna()
            .args(["govern", "configure", "--approval-mode", "quorum"])
            .assert()
            .success()
            .stdout(predicate::str::contains("quorum"));
    }

    #[test]
    fn test_govern_configure_min_approvers() {
        aeterna()
            .args(["govern", "configure", "--min-approvers", "3"])
            .assert()
            .success()
            .stdout(predicate::str::contains("min_approvers"));
    }

    #[test]
    fn test_govern_configure_timeout_hours() {
        aeterna()
            .args(["govern", "configure", "--timeout-hours", "48"])
            .assert()
            .success()
            .stdout(predicate::str::contains("timeout_hours"));
    }

    #[test]
    fn test_govern_configure_auto_approve_enabled() {
        aeterna()
            .args(["govern", "configure", "--auto-approve", "true"])
            .assert()
            .success()
            .stdout(predicate::str::contains("auto_approve"));
    }

    #[test]
    fn test_govern_configure_escalation_contact() {
        aeterna()
            .args([
                "govern",
                "configure",
                "--escalation-contact",
                "security-team@example.com"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("escalation_contact"));
    }

    #[test]
    fn test_govern_configure_json() {
        let output = aeterna()
            .args(["govern", "configure", "--approval-mode", "single", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
    }

    #[test]
    fn test_govern_roles_list() {
        aeterna()
            .args(["govern", "roles", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Role Assignments"));
    }

    #[test]
    fn test_govern_roles_list_json() {
        let output = aeterna()
            .args(["govern", "roles", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("roles").is_some());
    }

    #[test]
    fn test_govern_roles_assign() {
        aeterna()
            .args([
                "govern",
                "roles",
                "assign",
                "--principal",
                "alice",
                "--role",
                "approver",
                "--scope",
                "org"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Assign Role"));
    }

    #[test]
    fn test_govern_roles_revoke() {
        aeterna()
            .args([
                "govern",
                "roles",
                "revoke",
                "--principal",
                "alice",
                "--role",
                "approver",
                "--scope",
                "org"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Revoke Role"));
    }

    #[test]
    fn test_govern_roles_assign_json() {
        let output = aeterna()
            .args([
                "govern",
                "roles",
                "assign",
                "--principal",
                "bob",
                "--role",
                "admin",
                "--scope",
                "company",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("assign"));
    }

    #[test]
    fn test_govern_audit() {
        aeterna()
            .args(["govern", "audit"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_json() {
        let output = aeterna()
            .args(["govern", "audit", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("total").is_some());
        assert!(json.get("entries").is_some());
    }

    #[test]
    fn test_govern_audit_filter_by_action() {
        aeterna()
            .args(["govern", "audit", "--action", "approve"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_filter_by_since() {
        aeterna()
            .args(["govern", "audit", "--since", "24h"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_filter_by_actor() {
        aeterna()
            .args(["govern", "audit", "--actor", "alice"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_filter_by_target_type() {
        aeterna()
            .args(["govern", "audit", "--target-type", "policy"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_limit() {
        aeterna()
            .args(["govern", "audit", "--limit", "10"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Governance Audit Trail"));
    }

    #[test]
    fn test_govern_audit_export_csv() {
        aeterna()
            .args(["govern", "audit", "--export", "csv"])
            .assert()
            .success();
    }

    #[test]
    fn test_govern_audit_export_json_format() {
        aeterna()
            .args(["govern", "audit", "--export", "json"])
            .assert()
            .success();
    }
}

mod agent_subcommand {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_agent_help() {
        aeterna()
            .args(["agent", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("agent"))
            .stdout(predicate::str::contains("register"))
            .stdout(predicate::str::contains("list"))
            .stdout(predicate::str::contains("show"));
    }

    #[test]
    fn test_agent_register_help() {
        aeterna()
            .args(["agent", "register", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Register"))
            .stdout(predicate::str::contains("--agent-type"));
    }

    #[test]
    fn test_agent_register_dry_run() {
        aeterna()
            .args(["agent", "register", "test-agent", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_agent_register_dry_run_json() {
        let output = aeterna()
            .args(["agent", "register", "test-agent", "--dry-run", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("dryRun").and_then(|v| v.as_bool()), Some(true));
        assert!(json.get("agent").is_some());
    }

    #[test]
    fn test_agent_register_dry_run_with_description() {
        aeterna()
            .args([
                "agent",
                "register",
                "test-agent",
                "--dry-run",
                "--description",
                "Test agent for CI"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"))
            .stdout(predicate::str::contains("Test agent for CI"));
    }

    #[test]
    fn test_agent_register_dry_run_with_type() {
        aeterna()
            .args([
                "agent",
                "register",
                "opencode-agent",
                "--dry-run",
                "--agent-type",
                "opencode"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("opencode"));
    }

    #[test]
    fn test_agent_register_invalid_type() {
        aeterna()
            .args([
                "agent",
                "register",
                "test-agent",
                "--agent-type",
                "invalid-type"
            ])
            .assert()
            .failure();
    }

    #[test]
    fn test_agent_register_with_delegated_by() {
        aeterna()
            .args([
                "agent",
                "register",
                "my-agent",
                "--dry-run",
                "--delegated-by",
                "alice@acme.com"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("alice@acme.com"));
    }

    #[test]
    fn test_agent_list_help() {
        aeterna()
            .args(["agent", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List"))
            .stdout(predicate::str::contains("--all"));
    }

    #[test]
    fn test_agent_list() {
        aeterna()
            .args(["agent", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agents"));
    }

    #[test]
    fn test_agent_list_json() {
        let output = aeterna()
            .args(["agent", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("operation").is_some());
    }

    #[test]
    fn test_agent_list_all() {
        aeterna()
            .args(["agent", "list", "--all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agents"));
    }

    #[test]
    fn test_agent_list_filter_by_delegated_by() {
        aeterna()
            .args(["agent", "list", "--delegated-by", "alice"])
            .assert()
            .success()
            .stdout(predicate::str::contains("alice"));
    }

    #[test]
    fn test_agent_list_filter_by_type() {
        aeterna()
            .args(["agent", "list", "--agent-type", "opencode"])
            .assert()
            .success()
            .stdout(predicate::str::contains("opencode"));
    }

    #[test]
    fn test_agent_show_help() {
        aeterna()
            .args(["agent", "show", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Show"))
            .stdout(predicate::str::contains("--verbose"));
    }

    #[test]
    fn test_agent_show() {
        aeterna()
            .args(["agent", "show", "agent-test-123"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agent: agent-test-123"));
    }

    #[test]
    fn test_agent_show_json() {
        let output = aeterna()
            .args(["agent", "show", "agent-test-123", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("agentId").and_then(|v| v.as_str()),
            Some("agent-test-123")
        );
    }

    #[test]
    fn test_agent_show_verbose() {
        aeterna()
            .args(["agent", "show", "agent-test-123", "--verbose"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Agent: agent-test-123"))
            .stdout(predicate::str::contains("Verbose Details"));
    }

    #[test]
    fn test_agent_permissions_help() {
        aeterna()
            .args(["agent", "permissions", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("permission"))
            .stdout(predicate::str::contains("--grant"));
    }

    #[test]
    fn test_agent_permissions_list() {
        aeterna()
            .args(["agent", "permissions", "agent-test-123", "--list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Permissions"));
    }

    #[test]
    fn test_agent_permissions_list_json() {
        let output = aeterna()
            .args(["agent", "permissions", "agent-test-123", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("operation").is_some());
    }

    #[test]
    fn test_agent_permissions_grant() {
        aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--grant",
                "memory:read"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Grant Agent Permission"));
    }

    #[test]
    fn test_agent_permissions_grant_json() {
        let output = aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--grant",
                "memory:write",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("agent_permission_grant")
        );
    }

    #[test]
    fn test_agent_permissions_grant_with_scope() {
        aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--grant",
                "knowledge:read",
                "--scope",
                "org"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Scope"));
    }

    #[test]
    fn test_agent_permissions_grant_invalid() {
        aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--grant",
                "invalid:permission"
            ])
            .assert()
            .failure();
    }

    #[test]
    fn test_agent_permissions_revoke() {
        aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--revoke",
                "memory:write"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Revoke Agent Permission"));
    }

    #[test]
    fn test_agent_permissions_revoke_json() {
        let output = aeterna()
            .args([
                "agent",
                "permissions",
                "agent-test-123",
                "--revoke",
                "memory:write",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("agent_permission_revoke")
        );
    }

    #[test]
    fn test_agent_revoke_help() {
        aeterna()
            .args(["agent", "revoke", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Revoke"))
            .stdout(predicate::str::contains("--force"));
    }

    #[test]
    fn test_agent_revoke() {
        aeterna()
            .args(["agent", "revoke", "agent-test-123"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Revoke Agent"));
    }

    #[test]
    fn test_agent_revoke_json() {
        let output = aeterna()
            .args(["agent", "revoke", "agent-test-123", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("agent_revoke")
        );
    }

    #[test]
    fn test_agent_revoke_force() {
        aeterna()
            .args(["agent", "revoke", "agent-test-123", "--force"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Revoke Agent"));
    }
}

mod org_subcommand {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_org_help() {
        aeterna()
            .args(["org", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("org"))
            .stdout(predicate::str::contains("create"))
            .stdout(predicate::str::contains("list"))
            .stdout(predicate::str::contains("show"));
    }

    #[test]
    fn test_org_create_help() {
        aeterna()
            .args(["org", "create", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Create"))
            .stdout(predicate::str::contains("--description"));
    }

    #[test]
    fn test_org_create_dry_run() {
        aeterna()
            .args(["org", "create", "platform-engineering", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"));
    }

    #[test]
    fn test_org_create_dry_run_json() {
        let output = aeterna()
            .args(["org", "create", "platform-eng", "--dry-run", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json.get("dryRun").and_then(|v| v.as_bool()), Some(true));
        assert!(json.get("org").is_some());
    }

    #[test]
    fn test_org_create_dry_run_with_description() {
        aeterna()
            .args([
                "org",
                "create",
                "product-eng",
                "--dry-run",
                "--description",
                "Product Engineering team"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Dry Run"))
            .stdout(predicate::str::contains("Product Engineering"));
    }

    #[test]
    fn test_org_create_dry_run_with_company() {
        aeterna()
            .args([
                "org",
                "create",
                "security",
                "--dry-run",
                "--company",
                "acme-corp"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("acme-corp"));
    }

    #[test]
    fn test_org_list_help() {
        aeterna()
            .args(["org", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List"))
            .stdout(predicate::str::contains("--all"));
    }

    #[test]
    fn test_org_list() {
        aeterna()
            .args(["org", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Organizations"));
    }

    #[test]
    fn test_org_list_json() {
        let output = aeterna()
            .args(["org", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("operation").is_some());
    }

    #[test]
    fn test_org_list_all() {
        aeterna()
            .args(["org", "list", "--all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Organizations"));
    }

    #[test]
    fn test_org_list_filter_by_company() {
        aeterna()
            .args(["org", "list", "--company", "acme-corp"])
            .assert()
            .success()
            .stdout(predicate::str::contains("acme-corp"));
    }

    #[test]
    fn test_org_show_help() {
        aeterna()
            .args(["org", "show", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Show"))
            .stdout(predicate::str::contains("--verbose"));
    }

    #[test]
    fn test_org_show() {
        aeterna()
            .args(["org", "show", "platform-eng"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Organization: platform-eng"));
    }

    #[test]
    fn test_org_show_json() {
        let output = aeterna()
            .args(["org", "show", "platform-eng", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("orgId").and_then(|v| v.as_str()),
            Some("platform-eng")
        );
    }

    #[test]
    fn test_org_show_verbose() {
        aeterna()
            .args(["org", "show", "platform-eng", "--verbose"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Organization: platform-eng"))
            .stdout(predicate::str::contains("Verbose Details"));
    }

    #[test]
    fn test_org_show_no_arg_uses_context() {
        aeterna()
            .args(["org", "show"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Organization:"));
    }

    #[test]
    fn test_org_members_help() {
        aeterna()
            .args(["org", "members", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("member"))
            .stdout(predicate::str::contains("--add"));
    }

    #[test]
    fn test_org_members_list() {
        aeterna()
            .args(["org", "members"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Members"));
    }

    #[test]
    fn test_org_members_list_json() {
        let output = aeterna()
            .args(["org", "members", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("operation").is_some());
    }

    #[test]
    fn test_org_members_add() {
        aeterna()
            .args(["org", "members", "--add", "alice@acme.com"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Add Organization Member"));
    }

    #[test]
    fn test_org_members_add_json() {
        let output = aeterna()
            .args(["org", "members", "--add", "bob@acme.com", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("org_member_add")
        );
    }

    #[test]
    fn test_org_members_add_with_role() {
        aeterna()
            .args([
                "org",
                "members",
                "--add",
                "carol@acme.com",
                "--role",
                "techlead"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("techlead"));
    }

    #[test]
    fn test_org_members_add_invalid_role() {
        aeterna()
            .args([
                "org",
                "members",
                "--add",
                "dave@acme.com",
                "--role",
                "invalid-role"
            ])
            .assert()
            .failure();
    }

    #[test]
    fn test_org_members_add_with_org() {
        aeterna()
            .args([
                "org",
                "members",
                "--org",
                "platform-eng",
                "--add",
                "eve@acme.com"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("platform-eng"));
    }

    #[test]
    fn test_org_members_remove() {
        aeterna()
            .args(["org", "members", "--remove", "alice@acme.com"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Remove Organization Member"));
    }

    #[test]
    fn test_org_members_remove_json() {
        let output = aeterna()
            .args(["org", "members", "--remove", "bob@acme.com", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("org_member_remove")
        );
    }

    #[test]
    fn test_org_members_set_role() {
        aeterna()
            .args([
                "org",
                "members",
                "--set-role",
                "alice@acme.com",
                "--role",
                "architect"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set Member Role"));
    }

    #[test]
    fn test_org_members_set_role_json() {
        let output = aeterna()
            .args([
                "org",
                "members",
                "--set-role",
                "bob@acme.com",
                "--role",
                "admin",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(
            json.get("operation").and_then(|v| v.as_str()),
            Some("org_member_set_role")
        );
    }

    #[test]
    fn test_org_members_set_role_missing_role() {
        aeterna()
            .args(["org", "members", "--set-role", "alice@acme.com"])
            .assert()
            .failure();
    }

    #[test]
    fn test_org_use_help() {
        aeterna()
            .args(["org", "use", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set default"));
    }

    #[test]
    fn test_org_use() {
        aeterna()
            .args(["org", "use", "platform-eng"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set Default Organization"))
            .stdout(predicate::str::contains("platform-eng"));
    }
}

mod team_subcommand {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_team_help() {
        aeterna()
            .args(["team", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("team"))
            .stdout(predicate::str::contains("create"))
            .stdout(predicate::str::contains("list"));
    }

    #[test]
    fn test_team_create_help() {
        aeterna()
            .args(["team", "create", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Create"))
            .stdout(predicate::str::contains("--description"))
            .stdout(predicate::str::contains("--dry-run"));
    }

    #[test]
    fn test_team_create_dry_run() {
        aeterna()
            .args(["team", "create", "api-team", "--dry-run"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Team Create (Dry Run)"))
            .stdout(predicate::str::contains("api-team"))
            .stdout(predicate::str::contains("What Would Happen"));
    }

    #[test]
    fn test_team_create_dry_run_json() {
        let output = aeterna()
            .args(["team", "create", "api-team", "--dry-run", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["dryRun"], true);
        assert_eq!(json["operation"], "team_create");
        assert_eq!(json["team"]["name"], "api-team");
    }

    #[test]
    fn test_team_create_dry_run_with_description() {
        aeterna()
            .args([
                "team",
                "create",
                "api-team",
                "--description",
                "API development team",
                "--dry-run"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("api-team"))
            .stdout(predicate::str::contains("API development team"));
    }

    #[test]
    fn test_team_create_dry_run_with_org() {
        aeterna()
            .args([
                "team",
                "create",
                "api-team",
                "--org",
                "platform-eng",
                "--dry-run"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("api-team"))
            .stdout(predicate::str::contains("platform-eng"));
    }

    #[test]
    fn test_team_create_dry_run_with_org_json() {
        let output = aeterna()
            .args([
                "team",
                "create",
                "api-team",
                "--org",
                "platform-eng",
                "--dry-run",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["team"]["orgId"], "platform-eng");
    }

    #[test]
    fn test_team_list_help() {
        aeterna()
            .args(["team", "list", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("List"))
            .stdout(predicate::str::contains("--org"))
            .stdout(predicate::str::contains("--all"));
    }

    #[test]
    fn test_team_list() {
        aeterna()
            .args(["team", "list"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Teams"))
            .stdout(predicate::str::contains("Example Output"));
    }

    #[test]
    fn test_team_list_json() {
        let output = aeterna()
            .args(["team", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_list");
        assert!(json.get("filters").is_some());
    }

    #[test]
    fn test_team_list_all() {
        aeterna()
            .args(["team", "list", "--all"])
            .assert()
            .success()
            .stderr(predicate::str::contains("all teams"));
    }

    #[test]
    fn test_team_list_filter_by_org() {
        aeterna()
            .args(["team", "list", "--org", "platform-eng"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Filter: org = platform-eng"));
    }

    #[test]
    fn test_team_list_filter_by_org_json() {
        let output = aeterna()
            .args(["team", "list", "--org", "platform-eng", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["filters"]["org"], "platform-eng");
    }

    #[test]
    fn test_team_show_help() {
        aeterna()
            .args(["team", "show", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Show"))
            .stdout(predicate::str::contains("--verbose"))
            .stdout(predicate::str::contains("--json"));
    }

    #[test]
    fn test_team_show() {
        aeterna()
            .args(["team", "show", "api-team"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Team: api-team"))
            .stdout(predicate::str::contains("Would Show"));
    }

    #[test]
    fn test_team_show_json() {
        let output = aeterna()
            .args(["team", "show", "api-team", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_show");
        assert_eq!(json["teamId"], "api-team");
    }

    #[test]
    fn test_team_show_verbose() {
        aeterna()
            .args(["team", "show", "api-team", "--verbose"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Verbose Details"))
            .stdout(predicate::str::contains("Policy inheritance"));
    }

    #[test]
    fn test_team_show_no_arg_uses_context() {
        aeterna()
            .args(["team", "show"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Team:"));
    }

    #[test]
    fn test_team_members_help() {
        aeterna()
            .args(["team", "members", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Manage team members"))
            .stdout(predicate::str::contains("--add"))
            .stdout(predicate::str::contains("--remove"))
            .stdout(predicate::str::contains("--set-role"));
    }

    #[test]
    fn test_team_members_list() {
        aeterna()
            .args(["team", "members"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Members of:"))
            .stdout(predicate::str::contains("Example Output"));
    }

    #[test]
    fn test_team_members_list_json() {
        let output = aeterna()
            .args(["team", "members", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_members_list");
    }

    #[test]
    fn test_team_members_list_with_team() {
        aeterna()
            .args(["team", "members", "--team", "api-team"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Members of: api-team"));
    }

    #[test]
    fn test_team_members_add() {
        aeterna()
            .args(["team", "members", "--add", "alice@example.com"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Add Team Member"))
            .stdout(predicate::str::contains("alice@example.com"))
            .stdout(predicate::str::contains("Role: developer"));
    }

    #[test]
    fn test_team_members_add_json() {
        let output = aeterna()
            .args(["team", "members", "--add", "alice@example.com", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_member_add");
        assert_eq!(json["userId"], "alice@example.com");
        assert_eq!(json["role"], "developer");
    }

    #[test]
    fn test_team_members_add_with_role() {
        aeterna()
            .args([
                "team",
                "members",
                "--add",
                "alice@example.com",
                "--role",
                "techlead"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Role: techlead"));
    }

    #[test]
    fn test_team_members_add_with_team() {
        aeterna()
            .args([
                "team",
                "members",
                "--team",
                "api-team",
                "--add",
                "alice@example.com"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Team: api-team"))
            .stdout(predicate::str::contains("alice@example.com"));
    }

    #[test]
    fn test_team_members_add_invalid_role() {
        aeterna()
            .args([
                "team",
                "members",
                "--add",
                "alice@example.com",
                "--role",
                "superuser"
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Invalid team role"))
            .stderr(predicate::str::contains("developer, techlead, architect"));
    }

    #[test]
    fn test_team_members_remove() {
        aeterna()
            .args(["team", "members", "--remove", "bob@example.com"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Remove Team Member"))
            .stdout(predicate::str::contains("bob@example.com"));
    }

    #[test]
    fn test_team_members_remove_json() {
        let output = aeterna()
            .args(["team", "members", "--remove", "bob@example.com", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_member_remove");
        assert_eq!(json["userId"], "bob@example.com");
    }

    #[test]
    fn test_team_members_set_role() {
        aeterna()
            .args([
                "team",
                "members",
                "--set-role",
                "alice@example.com",
                "--role",
                "architect"
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set Member Role"))
            .stdout(predicate::str::contains("alice@example.com"))
            .stdout(predicate::str::contains("architect"));
    }

    #[test]
    fn test_team_members_set_role_json() {
        let output = aeterna()
            .args([
                "team",
                "members",
                "--set-role",
                "alice@example.com",
                "--role",
                "techlead",
                "--json"
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["operation"], "team_member_set_role");
        assert_eq!(json["userId"], "alice@example.com");
        assert_eq!(json["newRole"], "techlead");
    }

    #[test]
    fn test_team_members_set_role_missing_role() {
        aeterna()
            .args(["team", "members", "--set-role", "alice@example.com"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Missing --role"));
    }

    #[test]
    fn test_team_use_help() {
        aeterna()
            .args(["team", "use", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set default team"));
    }

    #[test]
    fn test_team_use() {
        aeterna()
            .args(["team", "use", "api-team"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set Default Team"))
            .stdout(predicate::str::contains("api-team"));
    }
}

mod context_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_context_help() {
        aeterna()
            .args(["context", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("context"))
            .stdout(predicate::str::contains("show"))
            .stdout(predicate::str::contains("set"));
    }

    #[test]
    fn test_context_show() {
        aeterna()
            .args(["context", "show"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("tenant")
                    .or(predicate::str::contains("Tenant"))
                    .or(predicate::str::contains("Context"))
            );
    }

    #[test]
    fn test_context_show_json() {
        aeterna()
            .args(["context", "show", "--json"])
            .assert()
            .success()
            .stdout(predicate::str::contains("tenant_id").or(predicate::str::contains("tenantId")));
    }

    #[test]
    fn test_context_set_help() {
        aeterna()
            .args(["context", "set", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Set"));
    }

    #[test]
    fn test_context_clear_help() {
        aeterna()
            .args(["context", "clear", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Clear"));
    }
}

mod error_handling {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_unknown_command() {
        aeterna()
            .arg("nonexistent-command")
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_unknown_subcommand() {
        aeterna()
            .args(["memory", "nonexistent"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("error"));
    }

    #[test]
    fn test_missing_required_argument() {
        aeterna()
            .args(["memory", "delete"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("required"));
    }

    #[test]
    fn test_invalid_flag_value() {
        aeterna()
            .args(["memory", "search", "query", "--limit", "not-a-number"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid"));
    }
}

mod json_output {
    use super::*;

    #[test]
    fn test_memory_add_json_structure() {
        let output = aeterna()
            .args(["memory", "add", "Test content", "--dry-run", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("dryRun").is_some());
        assert!(json.get("content").is_some());
    }

    #[test]
    fn test_user_whoami_json_structure() {
        let output = aeterna()
            .args(["user", "whoami", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("user").is_some());
    }

    #[test]
    fn test_context_show_json_structure() {
        let output = aeterna()
            .args(["context", "show", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        let has_valid_structure =
            json.is_array() || json.get("tenantId").is_some() || json.get("tenant_id").is_some();
        assert!(
            has_valid_structure,
            "Expected array or object with tenant info"
        );
    }
}

mod layer_validation {
    use super::*;

    #[test]
    fn test_memory_add_valid_layers() {
        let valid_layers = [
            "agent", "user", "session", "project", "team", "org", "company"
        ];
        for layer in valid_layers {
            aeterna()
                .args(["memory", "add", "Test", "--layer", layer, "--dry-run"])
                .assert()
                .success();
        }
    }

    #[test]
    fn test_policy_create_valid_layers() {
        let valid_layers = ["company", "org", "team", "project"];
        for layer in valid_layers {
            aeterna()
                .args([
                    "policy",
                    "create",
                    "--description",
                    "Test",
                    "--layer",
                    layer,
                    "--dry-run"
                ])
                .assert()
                .success();
        }
    }
}

mod check_subcommand {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_check_help() {
        aeterna()
            .args(["check", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("check"))
            .stdout(predicate::str::contains("--target"))
            .stdout(predicate::str::contains("--strict"));
    }

    #[test]
    fn test_check_default() {
        aeterna()
            .args(["check"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Constraint Validation"))
            .stdout(predicate::str::contains("Summary"));
    }

    #[test]
    fn test_check_json() {
        let output = aeterna()
            .args(["check", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("success").is_some());
        assert!(json.get("context").is_some());
        assert!(json.get("summary").is_some());
    }

    #[test]
    fn test_check_target_policies() {
        aeterna()
            .args(["check", "--target", "policies"])
            .assert()
            .success()
            .stdout(predicate::str::contains("POLICIES"));
    }

    #[test]
    fn test_check_target_dependencies() {
        aeterna()
            .args(["check", "--target", "dependencies"])
            .assert()
            .success()
            .stdout(predicate::str::contains("DEPENDENCIES"));
    }

    #[test]
    fn test_check_target_architecture() {
        aeterna()
            .args(["check", "--target", "architecture"])
            .assert()
            .success()
            .stdout(predicate::str::contains("ARCHITECTURE"));
    }

    #[test]
    fn test_check_target_security() {
        aeterna()
            .args(["check", "--target", "security"])
            .assert()
            .success()
            .stdout(predicate::str::contains("SECURITY"));
    }

    #[test]
    fn test_check_target_all() {
        aeterna()
            .args(["check", "--target", "all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("ALL"));
    }

    #[test]
    fn test_check_strict() {
        aeterna().args(["check", "--strict"]).assert().success();
    }

    #[test]
    fn test_check_violations_only() {
        aeterna()
            .args(["check", "--violations-only"])
            .assert()
            .success();
    }

    #[test]
    fn test_check_json_with_target() {
        let output = aeterna()
            .args(["check", "--json", "--target", "security"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["target"], "security");
    }

    #[test]
    fn test_check_json_strict_flag() {
        let output = aeterna()
            .args(["check", "--json", "--strict"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["strict"], true);
    }

    #[test]
    fn test_check_with_path() {
        aeterna().args(["check", "."]).assert().success();
    }
}

mod sync_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_sync_help() {
        aeterna()
            .args(["sync", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("sync"))
            .stdout(predicate::str::contains("--direction"))
            .stdout(predicate::str::contains("--dry-run"));
    }

    #[test]
    fn test_sync_default() {
        aeterna()
            .args(["sync"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Memory-Knowledge Sync"));
    }

    #[test]
    fn test_sync_dry_run() {
        aeterna()
            .args(["sync", "--dry-run"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Sync")
                    .or(predicate::str::contains("sync"))
                    .or(predicate::str::contains("Analyzing"))
            );
    }

    #[test]
    fn test_sync_json() {
        let output = aeterna()
            .args(["sync", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("success").is_some() || json.get("results").is_some());
    }

    #[test]
    fn test_sync_json_dry_run() {
        let output = aeterna()
            .args(["sync", "--json", "--dry-run"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert_eq!(json["dry_run"], true);
    }

    #[test]
    fn test_sync_direction_all() {
        aeterna()
            .args(["sync", "--direction", "all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("ALL"));
    }

    #[test]
    fn test_sync_direction_memory_to_knowledge() {
        aeterna()
            .args(["sync", "--direction", "memory-to-knowledge"])
            .assert()
            .success()
            .stdout(predicate::str::contains("MEMORY-TO-KNOWLEDGE"));
    }

    #[test]
    fn test_sync_verbose() {
        aeterna()
            .args(["sync", "--verbose"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Analysis Details"));
    }

    #[test]
    fn test_sync_force() {
        aeterna().args(["sync", "--force"]).assert().success();
    }
}

mod status_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_status_help() {
        aeterna()
            .args(["status", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("status"));
    }

    #[test]
    fn test_status_default() {
        aeterna().args(["status"]).assert().success().stdout(
            predicate::str::contains("Status")
                .or(predicate::str::contains("status"))
                .or(predicate::str::contains("Connection"))
        );
    }

    #[test]
    fn test_status_json() {
        let output = aeterna()
            .args(["status", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.get("tenant_id").is_some() || json.get("hints").is_some());
    }

    #[test]
    fn test_status_verbose() {
        aeterna().args(["status", "--verbose"]).assert().success();
    }
}

mod init_subcommand {
    use super::*;
    use predicates::prelude::predicate;

    #[test]
    fn test_init_help() {
        aeterna()
            .args(["init", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("init"))
            .stdout(predicate::str::contains("--tenant-id"))
            .stdout(predicate::str::contains("--force"));
    }

    #[test]
    fn test_init_preset_flag_in_help() {
        aeterna()
            .args(["init", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--preset"));
    }

    #[test]
    fn test_init_path_flag_in_help() {
        aeterna()
            .args(["init", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--path"));
    }
}

mod hints_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_hints_help() {
        aeterna()
            .args(["hints", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("hints"))
            .stdout(predicate::str::contains("list"))
            .stdout(predicate::str::contains("explain"))
            .stdout(predicate::str::contains("parse"));
    }

    #[test]
    fn test_hints_list() {
        aeterna().args(["hints", "list"]).assert().success().stdout(
            predicate::str::contains("Available Presets")
                .or(predicate::str::contains("minimal"))
                .or(predicate::str::contains("standard"))
        );
    }

    #[test]
    fn test_hints_list_json() {
        let output = aeterna()
            .args(["hints", "list", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.is_array() || json.is_object());
    }

    #[test]
    fn test_hints_explain() {
        aeterna()
            .args(["hints", "explain", "minimal"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Preset")
                    .or(predicate::str::contains("minimal"))
                    .or(predicate::str::contains("Hints"))
            );
    }

    #[test]
    fn test_hints_explain_json() {
        let output = aeterna()
            .args(["hints", "explain", "standard", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.is_object());
    }

    #[test]
    fn test_hints_parse() {
        aeterna()
            .args(["hints", "parse", "fast,no-llm"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Parsed")
                    .or(predicate::str::contains("fast"))
                    .or(predicate::str::contains("Hints"))
            );
    }

    #[test]
    fn test_hints_parse_json() {
        let output = aeterna()
            .args(["hints", "parse", "minimal,verbose", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();

        let json: serde_json::Value = serde_json::from_slice(&output).expect("Valid JSON");
        assert!(json.is_object());
    }
}

mod completion_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;
    use predicates::prelude::predicate;

    #[test]
    fn test_completion_help() {
        aeterna()
            .args(["completion", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("completion"))
            .stdout(predicate::str::contains("shell"));
    }

    #[test]
    fn test_completion_bash() {
        aeterna()
            .args(["completion", "bash"])
            .assert()
            .success()
            .stdout(predicate::str::contains("complete").or(predicate::str::contains("_aeterna")));
    }

    #[test]
    fn test_completion_zsh() {
        aeterna()
            .args(["completion", "zsh"])
            .assert()
            .success()
            .stdout(predicate::str::contains("compdef").or(predicate::str::contains("_aeterna")));
    }

    #[test]
    fn test_completion_fish() {
        aeterna()
            .args(["completion", "fish"])
            .assert()
            .success()
            .stdout(predicate::str::contains("complete"));
    }
}
