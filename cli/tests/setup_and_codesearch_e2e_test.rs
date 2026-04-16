//! E2E tests for `aeterna setup` and `aeterna code-search` binary-level coverage.
//!
//! These cover the two command groups that had **zero** binary-level test coverage:
//!
//!   - `setup` (all non-interactive paths + `--validate` + `--show` + file generation)
//!   - `code-search` (help flags; failure-path tests for all subcommands that require
//!     either a `codesearch` backend binary or a live Aeterna server)
//!
//! Context set/clear filesystem tests are also included here because the existing
//! `cli_e2e_test.rs` only has `--help` for those subcommands.

use assert_cmd::{Command, cargo_bin_cmd};
use predicates::prelude::predicate;
use std::fs;
use tempfile::TempDir;

fn aeterna() -> Command {
    cargo_bin_cmd!("aeterna")
}

// ─── setup ────────────────────────────────────────────────────────────────────

mod setup_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;

    // ── help ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_setup_help_exits_success() {
        aeterna()
            .args(["setup", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("setup"))
            .stdout(predicate::str::contains("--non-interactive"))
            .stdout(predicate::str::contains("--target"))
            .stdout(predicate::str::contains("--mode"));
    }

    #[test]
    fn test_setup_help_shows_llm_and_vector_flags() {
        aeterna()
            .args(["setup", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--llm"))
            .stdout(predicate::str::contains("--vector-backend"));
    }

    #[test]
    fn test_setup_help_shows_validate_and_show_flags() {
        aeterna()
            .args(["setup", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--validate"))
            .stdout(predicate::str::contains("--show"));
    }

    // ── --validate / --show without config ───────────────────────────────────

    #[test]
    fn test_setup_validate_without_config_prints_error_and_exits_success() {
        // run_validate returns Ok(()) even when config is missing (prints error msg)
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--validate",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .stderr(
                predicate::str::contains("No configuration found")
                    .or(predicate::str::contains("Run 'aeterna setup' first")),
            );
    }

    #[test]
    fn test_setup_show_without_config_prints_error_and_exits_success() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args(["setup", "--show", "--output", temp.path().to_str().unwrap()])
            .assert()
            .success()
            .stderr(
                predicate::str::contains("No configuration found")
                    .or(predicate::str::contains("Run 'aeterna setup' first")),
            );
    }

    // ── non-interactive: missing --target ────────────────────────────────────

    #[test]
    fn test_setup_non_interactive_requires_target() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("--target is required")
                    .or(predicate::str::contains("target")),
            );
    }

    // ── non-interactive: missing --mode ──────────────────────────────────────

    #[test]
    fn test_setup_non_interactive_requires_mode() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("--mode is required").or(predicate::str::contains("mode")),
            );
    }

    // ── non-interactive: hybrid requires --central-url ───────────────────────

    #[test]
    fn test_setup_non_interactive_hybrid_requires_central_url() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "kubernetes",
                "--mode",
                "hybrid",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--central-url"));
    }

    // ── non-interactive: docker-compose / local → generates files ────────────

    #[test]
    fn test_setup_non_interactive_docker_compose_local_generates_files() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("generated successfully")
                    .or(predicate::str::contains("Configuration generated")),
            );

        assert!(
            temp.path().join(".aeterna").join("config.toml").exists(),
            ".aeterna/config.toml must be generated"
        );
        assert!(
            temp.path().join("docker-compose.yaml").exists(),
            "docker-compose.yaml must be generated"
        );
    }

    #[test]
    fn test_setup_non_interactive_docker_compose_local_config_toml_content() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        let config = fs::read_to_string(temp.path().join(".aeterna").join("config.toml")).unwrap();
        assert!(
            config.contains("docker-compose") || config.contains("local"),
            "config.toml should reflect deployment mode"
        );
    }

    // ── non-interactive: kubernetes / local → generates values.yaml ──────────

    #[test]
    fn test_setup_non_interactive_kubernetes_local_generates_files() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "kubernetes",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        assert!(
            temp.path().join(".aeterna").join("config.toml").exists(),
            ".aeterna/config.toml must be generated"
        );
        assert!(
            temp.path().join("values.yaml").exists(),
            "values.yaml must be generated for kubernetes target"
        );
    }

    // ── --validate with existing config ──────────────────────────────────────

    #[test]
    fn test_setup_validate_with_generated_config_reports_valid() {
        let temp = TempDir::new().unwrap();

        // First generate a valid config
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        // Then validate it
        aeterna()
            .args([
                "setup",
                "--validate",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("valid").or(predicate::str::contains("Validating")));
    }

    // ── --show with existing config ───────────────────────────────────────────

    #[test]
    fn test_setup_show_with_generated_config_prints_content() {
        let temp = TempDir::new().unwrap();

        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        aeterna()
            .args(["setup", "--show", "--output", temp.path().to_str().unwrap()])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Current configuration")
                    .or(predicate::str::contains("config.toml")),
            );
    }

    // ── --show masks sensitive values ─────────────────────────────────────────

    #[test]
    fn test_setup_show_masks_api_key_in_config() {
        let temp = TempDir::new().unwrap();
        let aeterna_dir = temp.path().join(".aeterna");
        fs::create_dir_all(&aeterna_dir).unwrap();
        fs::write(
            aeterna_dir.join("config.toml"),
            r#"api_key = "sk-supersecret-12345"
mode = "local"
"#,
        )
        .unwrap();

        aeterna()
            .args(["setup", "--show", "--output", temp.path().to_str().unwrap()])
            .assert()
            .success()
            .stdout(predicate::str::contains("***MASKED***"))
            .stdout(predicate::str::contains("sk-supersecret-12345").not());
    }

    // ── invalid enum values ───────────────────────────────────────────────────

    #[test]
    fn test_setup_invalid_target_fails() {
        aeterna()
            .args(["setup", "--non-interactive", "--target", "baremetal"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("baremetal")));
    }

    #[test]
    fn test_setup_invalid_mode_fails() {
        aeterna()
            .args(["setup", "--non-interactive", "--mode", "cloud-only"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("cloud-only")));
    }

    #[test]
    fn test_setup_invalid_vector_backend_fails() {
        aeterna()
            .args(["setup", "--vector-backend", "chroma"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("chroma")));
    }

    #[test]
    fn test_setup_invalid_llm_provider_fails() {
        aeterna()
            .args(["setup", "--llm", "gpt-5"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid").or(predicate::str::contains("gpt-5")));
    }

    // ── google LLM requires --google-project-id ───────────────────────────────

    #[test]
    fn test_setup_google_llm_requires_project_id() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--llm",
                "google",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--google-project-id"));
    }

    // ── bedrock LLM requires --bedrock-region ─────────────────────────────────

    #[test]
    fn test_setup_bedrock_llm_requires_region() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "docker-compose",
                "--mode",
                "local",
                "--llm",
                "bedrock",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--bedrock-region"));
    }

    // ── opencode-only target ──────────────────────────────────────────────────

    #[test]
    fn test_setup_non_interactive_opencode_only() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .args([
                "setup",
                "--non-interactive",
                "--target",
                "opencode-only",
                "--mode",
                "local",
                "--output",
                temp.path().to_str().unwrap(),
            ])
            .assert()
            .success();

        // opencode-only target does not generate docker-compose.yaml or values.yaml
        assert!(
            !temp.path().join("docker-compose.yaml").exists(),
            "opencode-only should not generate docker-compose.yaml"
        );
        assert!(
            !temp.path().join("values.yaml").exists(),
            "opencode-only should not generate values.yaml"
        );
        // but always generates config.toml
        assert!(
            temp.path().join(".aeterna").join("config.toml").exists(),
            ".aeterna/config.toml must always be generated"
        );
    }
}

// ─── context set / clear (filesystem-asserting) ───────────────────────────────

mod context_set_and_clear {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;

    // ── context set ──────────────────────────────────────────────────────────

    #[test]
    fn test_context_set_tenant_id_writes_file() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "tenant-id", "acme-corp"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("tenant-id").or(predicate::str::contains("acme-corp")),
            );

        let content =
            fs::read_to_string(temp.path().join(".aeterna").join("context.toml")).unwrap();
        assert!(
            content.contains("acme-corp"),
            "tenant-id value must be in context.toml"
        );
    }

    #[test]
    fn test_context_set_user_id_writes_file() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "user-id", "alice@acme.com"])
            .assert()
            .success();

        let content =
            fs::read_to_string(temp.path().join(".aeterna").join("context.toml")).unwrap();
        assert!(content.contains("alice@acme.com"));
    }

    #[test]
    fn test_context_set_multiple_keys_accumulate() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "tenant-id", "t1"])
            .assert()
            .success();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "org-id", "o1"])
            .assert()
            .success();

        let content =
            fs::read_to_string(temp.path().join(".aeterna").join("context.toml")).unwrap();
        assert!(
            content.contains("t1"),
            "tenant-id must persist after second set"
        );
        assert!(content.contains("o1"), "org-id must be written");
    }

    #[test]
    fn test_context_set_overwrites_existing_key() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "tenant-id", "first-value"])
            .assert()
            .success();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "set", "tenant-id", "second-value"])
            .assert()
            .success();

        let content =
            fs::read_to_string(temp.path().join(".aeterna").join("context.toml")).unwrap();
        assert!(
            content.contains("second-value"),
            "overwritten value must be present"
        );
        assert!(
            !content.contains("first-value"),
            "old value must not remain"
        );
    }

    // ── context clear ─────────────────────────────────────────────────────────

    #[test]
    fn test_context_clear_removes_context_toml() {
        let temp = TempDir::new().unwrap();
        // Create a context.toml first
        let aeterna_dir = temp.path().join(".aeterna");
        fs::create_dir_all(&aeterna_dir).unwrap();
        fs::write(aeterna_dir.join("context.toml"), "tenant-id = \"test\"").unwrap();

        aeterna()
            .current_dir(temp.path())
            .args(["context", "clear"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("Removed").or(predicate::str::contains("context.toml")),
            );

        assert!(
            !temp.path().join(".aeterna").join("context.toml").exists(),
            "context.toml should be removed"
        );
        // .aeterna directory itself should remain
        assert!(
            temp.path().join(".aeterna").exists(),
            ".aeterna dir should survive without --all"
        );
    }

    #[test]
    fn test_context_clear_all_removes_aeterna_dir() {
        let temp = TempDir::new().unwrap();
        let aeterna_dir = temp.path().join(".aeterna");
        fs::create_dir_all(&aeterna_dir).unwrap();
        fs::write(aeterna_dir.join("context.toml"), "tenant-id = \"test\"").unwrap();

        aeterna()
            .current_dir(temp.path())
            .args(["context", "clear", "--all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Removed"));

        assert!(
            !temp.path().join(".aeterna").exists(),
            ".aeterna directory should be removed with --all"
        );
    }

    #[test]
    fn test_context_clear_when_no_file_is_noop() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "clear"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("No context.toml found")
                    .or(predicate::str::contains("No")),
            );
    }

    #[test]
    fn test_context_clear_all_when_no_dir_is_noop() {
        let temp = TempDir::new().unwrap();
        aeterna()
            .current_dir(temp.path())
            .args(["context", "clear", "--all"])
            .assert()
            .success()
            .stdout(predicate::str::contains("No .aeterna").or(predicate::str::contains("No")));
    }
}

// ─── code-search ──────────────────────────────────────────────────────────────

mod code_search_subcommand {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;

    // ── top-level help ────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_help_shows_all_subcommands() {
        aeterna()
            .args(["code-search", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("init"))
            .stdout(predicate::str::contains("search"))
            .stdout(predicate::str::contains("trace"))
            .stdout(predicate::str::contains("status"))
            .stdout(predicate::str::contains("repo"))
            .stdout(predicate::str::contains("index"));
    }

    // ── init ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_init_help() {
        aeterna()
            .args(["code-search", "init", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Initialize"))
            .stdout(predicate::str::contains("--embedder"))
            .stdout(predicate::str::contains("--store"))
            .stdout(predicate::str::contains("--force"));
    }

    #[test]
    fn test_codesearch_init_fails_without_backend_binary() {
        // `code-search init` shells out to `codesearch --version`.
        // In CI there is no `codesearch` binary, so it must fail with a
        // clear "backend not found" message.
        let temp = TempDir::new().unwrap();
        let output = aeterna()
            .args(["code-search", "init", temp.path().to_str().unwrap()])
            .output()
            .expect("process ran");

        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("Code Search backend not found")
                || combined.contains("backend not found")
                || combined.contains("codesearch"),
            "Expected backend-not-found message, got: {combined}"
        );
    }

    #[test]
    fn test_codesearch_init_missing_path_fails() {
        // Non-existent path → explicit error before backend check
        let output = aeterna()
            .args(["code-search", "init", "/this/path/does/not/exist/aeterna"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("does not exist") || combined.contains("not found"),
            "Expected path-not-found message, got: {combined}"
        );
    }

    // ── search ────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_search_help() {
        aeterna()
            .args(["code-search", "search", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("query").or(predicate::str::contains("Search")))
            .stdout(predicate::str::contains("--limit"))
            .stdout(predicate::str::contains("--threshold"));
    }

    #[test]
    fn test_codesearch_search_fails_without_backend_binary() {
        let output = aeterna()
            .args(["code-search", "search", "authentication middleware"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        // search shells out to `codesearch search` — fails on missing binary or error
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        // Either OS "not found" or our own error message
        assert!(
            combined.contains("Search failed")
                || combined.contains("codesearch")
                || combined.contains("No such file")
                || combined.contains("not found"),
            "Expected search-failed message, got: {combined}"
        );
    }

    #[test]
    fn test_codesearch_search_missing_query_fails() {
        aeterna()
            .args(["code-search", "search"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("required").or(predicate::str::contains("<QUERY>")));
    }

    // ── trace ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_trace_help() {
        aeterna()
            .args(["code-search", "trace", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("callers").or(predicate::str::contains("Trace")))
            .stdout(predicate::str::contains("callees"))
            .stdout(predicate::str::contains("graph"));
    }

    #[test]
    fn test_codesearch_trace_callers_help() {
        aeterna()
            .args(["code-search", "trace", "callers", "--help"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("symbol")
                    .or(predicate::str::contains("Find all functions")),
            )
            .stdout(predicate::str::contains("--max-depth"))
            .stdout(predicate::str::contains("--recursive"));
    }

    #[test]
    fn test_codesearch_trace_callers_fails_without_backend() {
        let output = aeterna()
            .args(["code-search", "trace", "callers", "handle_payment"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("Trace failed")
                || combined.contains("codesearch")
                || combined.contains("No such file")
                || combined.contains("not found"),
            "Expected trace-failed message, got: {combined}"
        );
    }

    #[test]
    fn test_codesearch_trace_callees_help() {
        aeterna()
            .args(["code-search", "trace", "callees", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("symbol").or(predicate::str::contains("callees")));
    }

    #[test]
    fn test_codesearch_trace_callees_fails_without_backend() {
        let output = aeterna()
            .args(["code-search", "trace", "callees", "process_payment"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
    }

    #[test]
    fn test_codesearch_trace_graph_help() {
        aeterna()
            .args(["code-search", "trace", "graph", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("symbol").or(predicate::str::contains("graph")))
            .stdout(predicate::str::contains("--depth"))
            .stdout(predicate::str::contains("--format"));
    }

    #[test]
    fn test_codesearch_trace_graph_fails_without_backend() {
        let output = aeterna()
            .args([
                "code-search",
                "trace",
                "graph",
                "PaymentService",
                "--depth",
                "2",
            ])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
    }

    #[test]
    fn test_codesearch_trace_missing_subcommand_fails() {
        aeterna()
            .args(["code-search", "trace"])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("subcommand").or(predicate::str::contains("required")),
            );
    }

    // ── status ────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_status_help() {
        aeterna()
            .args(["code-search", "status", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("status").or(predicate::str::contains("Status")))
            .stdout(predicate::str::contains("--project"))
            .stdout(predicate::str::contains("--watch"));
    }

    #[test]
    fn test_codesearch_status_fails_without_backend() {
        let output = aeterna()
            .args(["code-search", "status"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("Status check failed")
                || combined.contains("codesearch")
                || combined.contains("No such file")
                || combined.contains("not found"),
            "Expected status-failed message, got: {combined}"
        );
    }

    #[test]
    fn test_codesearch_status_with_project_flag_fails_without_backend() {
        let output = aeterna()
            .args(["code-search", "status", "--project", "payments-service"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
    }

    // ── repo ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_repo_help() {
        aeterna()
            .args(["code-search", "repo", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("request").or(predicate::str::contains("repo")))
            .stdout(predicate::str::contains("list"))
            .stdout(predicate::str::contains("approve"))
            .stdout(predicate::str::contains("reject"))
            .stdout(predicate::str::contains("identity"));
    }

    #[test]
    fn test_codesearch_repo_reject_help() {
        aeterna()
            .args(["code-search", "repo", "reject", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--reason").or(predicate::str::contains("reason")));
    }

    #[test]
    fn test_codesearch_repo_reject_fails_without_server() {
        let output = aeterna()
            .args([
                "code-search",
                "repo",
                "reject",
                "req-456",
                "--reason",
                "Not authorized",
            ])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("no longer shells out to the legacy `codesearch` binary")
                || combined.contains("MCP-compatible code intelligence backend")
                || combined.contains("JetBrains Code Intelligence MCP")
                || combined.contains("not supported")
                || combined.contains("not yet available"),
            "got: {combined}"
        );
    }

    #[test]
    fn test_codesearch_repo_identity_help() {
        aeterna()
            .args(["code-search", "repo", "identity", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("identity").or(predicate::str::contains("Git")));
    }

    #[test]
    fn test_codesearch_repo_identity_list_fails_without_server() {
        let output = aeterna()
            .args(["code-search", "repo", "identity", "list"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
    }

    #[test]
    fn test_codesearch_repo_identity_add_help() {
        aeterna()
            .args(["code-search", "repo", "identity", "add", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--name").or(predicate::str::contains("identity")))
            .stdout(predicate::str::contains("--provider"))
            .stdout(predicate::str::contains("--secret-id"));
    }

    // ── index ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_codesearch_index_help() {
        aeterna()
            .args(["code-search", "index", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("--repo"))
            .stdout(predicate::str::contains("index").or(predicate::str::contains("Index")));
    }

    #[test]
    fn test_codesearch_index_requires_repo_flag() {
        aeterna()
            .args(["code-search", "index"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("required").or(predicate::str::contains("--repo")));
    }

    #[test]
    fn test_codesearch_index_incremental_flag_help() {
        aeterna()
            .args(["code-search", "index", "--help"])
            .assert()
            .success()
            .stdout(
                predicate::str::contains("--incremental")
                    .or(predicate::str::contains("incremental")),
            );
    }
}

// ─── additional gap-filling: memory show/list, user show/list, policy list/explain/validate ──

mod additional_coverage {
    use super::*;
    use predicates::prelude::PredicateBooleanExt;

    // memory show (not-connected output)
    #[test]
    fn test_memory_show_help() {
        aeterna()
            .args(["memory", "show", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Show").or(predicate::str::contains("show")));
    }

    #[test]
    fn test_memory_show_fails_without_server() {
        aeterna()
            .args(["memory", "show", "mem-abc"])
            .assert()
            .failure()
            .stderr(
                predicate::str::contains("not connected")
                    .or(predicate::str::contains("AETERNA_SERVER_URL"))
                    .or(predicate::str::contains("not yet available")),
            );
    }

    #[test]
    fn test_memory_show_json_fails_without_server() {
        let output = aeterna()
            .args(["memory", "show", "mem-abc", "--json"])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("not yet available"),
            "got: {combined}"
        );
    }

    // memory list (not-connected output)
    #[test]
    fn test_memory_list_json_not_connected() {
        let output = aeterna()
            .args(["memory", "list", "--json"])
            .output()
            .expect("process ran");
        // may succeed (returning not_connected JSON) or fail — both are acceptable
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}{stderr}");
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "Expected not-connected output, got: {combined}"
        );
    }

    // user show / user list
    #[test]
    fn test_user_show_help() {
        aeterna()
            .args(["user", "show", "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains("Show").or(predicate::str::contains("user")));
    }

    #[test]
    fn test_user_show_json_not_connected() {
        let output = aeterna()
            .args(["user", "show", "--json"])
            .output()
            .expect("process ran");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "got: {combined}"
        );
    }

    #[test]
    fn test_user_list_json_not_connected() {
        let output = aeterna()
            .args(["user", "list", "--json"])
            .output()
            .expect("process ran");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "got: {combined}"
        );
    }

    // policy list / explain / validate
    #[test]
    fn test_policy_list_json_not_connected() {
        let output = aeterna()
            .args(["policy", "list", "--json"])
            .output()
            .expect("process ran");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "got: {combined}"
        );
    }

    #[test]
    fn test_policy_explain_json_not_connected() {
        let output = aeterna()
            .args(["policy", "explain", "policy-1", "--json"])
            .output()
            .expect("process ran");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "got: {combined}"
        );
    }

    #[test]
    fn test_policy_validate_json_not_connected() {
        let output = aeterna()
            .args(["policy", "validate", "policy-1", "--json"])
            .output()
            .expect("process ran");
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("not_connected")
                || combined.contains("not connected")
                || combined.contains("AETERNA_SERVER_URL"),
            "got: {combined}"
        );
    }

    // govern audit --export to file
    #[test]
    fn test_govern_audit_export_json_to_file() {
        let temp = TempDir::new().unwrap();
        let out_path = temp.path().join("audit.json");
        let output = aeterna()
            .args([
                "govern",
                "audit",
                "--export",
                "json",
                "--output",
                out_path.to_str().unwrap(),
            ])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("Cannot list governance audit entries: server not connected")
                || combined.contains("No Aeterna server URL is configured")
                || combined.contains("AETERNA_SERVER_URL"),
            "Expected govern audit export to fail closed without a server, got: {combined}"
        );
        assert!(
            !out_path.exists(),
            "audit export should not write a file when the server is not connected"
        );
    }

    #[test]
    fn test_govern_audit_export_csv_to_file() {
        let temp = TempDir::new().unwrap();
        let out_path = temp.path().join("audit.csv");
        let output = aeterna()
            .args([
                "govern",
                "audit",
                "--export",
                "csv",
                "--output",
                out_path.to_str().unwrap(),
            ])
            .output()
            .expect("process ran");
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            combined.contains("Cannot list governance audit entries: server not connected")
                || combined.contains("No Aeterna server URL is configured")
                || combined.contains("AETERNA_SERVER_URL"),
            "Expected govern audit export to fail closed without a server, got: {combined}"
        );
        assert!(
            !out_path.exists(),
            "audit export should not write a file when the server is not connected"
        );
    }
}
