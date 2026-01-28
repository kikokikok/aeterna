use assert_cmd::{Command, cargo_bin_cmd};
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn aeterna() -> Command {
    cargo_bin_cmd!("aeterna")
}

#[test]
fn test_init_command_help() {
    aeterna()
        .arg("init")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Directory to initialize"))
        .stdout(predicate::str::contains("Tenant ID to use"))
        .stdout(predicate::str::contains(
            "Force overwrite existing context.toml"
        ));
}

#[test]
fn test_init_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("test-tenant")
        .arg("--user-id")
        .arg("test-user")
        .arg("--org-id")
        .arg("test-org")
        .arg("--team-id")
        .arg("test-team")
        .arg("--project-id")
        .arg("test-project")
        .arg("--preset")
        .arg("standard")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"))
        .stdout(predicate::str::contains("tenant_id:  test-tenant"))
        .stdout(predicate::str::contains("user_id:    test-user"))
        .stdout(predicate::str::contains("project_id: test-project"))
        .stdout(predicate::str::contains("preset:     standard"));

    assert!(aeterna_dir.exists(), ".aeterna directory should exist");
    assert!(context_file.exists(), "context.toml file should exist");

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("tenant-id = \"test-tenant\""));
    assert!(content.contains("user-id = \"test-user\""));
    assert!(content.contains("org-id = \"test-org\""));
    assert!(content.contains("team-id = \"test-team\""));
    assert!(content.contains("project-id = \"test-project\""));
    assert!(content.contains("preset = \"standard\""));
}

#[test]
fn test_init_command_minimal() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("minimal-tenant")
        .arg("--user-id")
        .arg("minimal-user")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"))
        .stdout(predicate::str::contains("tenant_id:  minimal-tenant"))
        .stdout(predicate::str::contains("user_id:    minimal-user"));

    assert!(aeterna_dir.exists(), ".aeterna directory should exist");
    assert!(context_file.exists(), "context.toml file should exist");

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("tenant-id = \"minimal-tenant\""));
    assert!(content.contains("user-id = \"minimal-user\""));
    assert!(
        !content.contains("org-id"),
        "Should not contain org-id when not specified"
    );
    assert!(
        !content.contains("team-id"),
        "Should not contain team-id when not specified"
    );
    assert!(
        !content.contains("project-id"),
        "Should not contain project-id when not specified"
    );
    assert!(
        content.contains("preset = \"standard\""),
        "Should use default preset"
    );
}

#[test]
fn test_init_command_without_force_on_existing() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    fs::create_dir_all(&aeterna_dir).unwrap();
    fs::write(&context_file, "existing content").unwrap();

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("new-tenant")
        .arg("--user-id")
        .arg("new-user")
        .arg("--yes")
        .assert()
        .success()
        .stderr(predicate::str::contains("Context already exists"))
        .stderr(predicate::str::contains("Use --force to overwrite"));

    let content = fs::read_to_string(context_file).unwrap();
    assert_eq!(
        content, "existing content",
        "Should not overwrite without --force"
    );
}

#[test]
fn test_init_command_with_force() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    fs::create_dir_all(&aeterna_dir).unwrap();
    fs::write(&context_file, "existing content").unwrap();

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("forced-tenant")
        .arg("--user-id")
        .arg("forced-user")
        .arg("--force")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"))
        .stdout(predicate::str::contains("tenant_id:  forced-tenant"))
        .stdout(predicate::str::contains("user_id:    forced-user"));

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("tenant-id = \"forced-tenant\""));
    assert!(content.contains("user-id = \"forced-user\""));
    assert!(
        !content.contains("existing content"),
        "Should overwrite with --force"
    );
}

#[test]
fn test_init_command_different_preset() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("preset-tenant")
        .arg("--user-id")
        .arg("preset-user")
        .arg("--preset")
        .arg("strict")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("preset:     strict"));

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("preset = \"strict\""));
}

#[test]
fn test_init_command_current_directory() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--tenant-id")
        .arg("current-tenant")
        .arg("--user-id")
        .arg("current-user")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"));

    assert!(aeterna_dir.exists(), ".aeterna directory should exist");
    assert!(context_file.exists(), "context.toml file should exist");

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("tenant-id = \"current-tenant\""));
    assert!(content.contains("user-id = \"current-user\""));

    std::env::set_current_dir(original_dir).unwrap();
}

#[test]
fn test_init_command_without_yes_flag() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--tenant-id")
        .arg("no-yes-tenant")
        .arg("--user-id")
        .arg("no-yes-user")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"));

    assert!(aeterna_dir.exists(), ".aeterna directory should exist");
    assert!(context_file.exists(), "context.toml file should exist");
}

#[test]
fn test_init_command_missing_required_args() {
    let temp_dir = TempDir::new().unwrap();

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--yes")
        .assert()
        .success();

    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    assert!(aeterna_dir.exists(), ".aeterna directory should exist");
    assert!(context_file.exists(), "context.toml file should exist");

    let content = fs::read_to_string(context_file).unwrap();
    assert!(
        content.contains("tenant-id = \"default\""),
        "Should use default tenant-id"
    );
    assert!(
        content.contains("user-id ="),
        "Should have user-id from context resolution"
    );
}

#[test]
fn test_init_command_directory_creation() {
    let temp_dir = TempDir::new().unwrap();
    let nested_dir = temp_dir.path().join("deep").join("nested").join("path");
    let aeterna_dir = nested_dir.join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(&nested_dir)
        .arg("--tenant-id")
        .arg("nested-tenant")
        .arg("--user-id")
        .arg("nested-user")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized Aeterna"));

    assert!(
        aeterna_dir.exists(),
        "Should create nested .aeterna directory"
    );
    assert!(
        context_file.exists(),
        "Should create context.toml in nested directory"
    );
}

#[test]
fn test_init_command_context_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let aeterna_dir = temp_dir.path().join(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    aeterna()
        .arg("init")
        .arg("--path")
        .arg(temp_dir.path())
        .arg("--yes")
        .assert()
        .success();

    let content = fs::read_to_string(context_file).unwrap();
    assert!(content.contains("tenant-id ="), "Should have tenant-id");
    assert!(content.contains("user-id ="), "Should have user-id");
    assert!(
        content.contains("preset = \"standard\""),
        "Should have default preset"
    );
}
