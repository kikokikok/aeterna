use std::{fs, path::Path};

#[test]
fn cedar_role_catalog_matches_control_plane_matrix_roles() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate must live under workspace root");
    let cedar_path = project_root.join("policies/cedar/rbac.cedar");
    let cedar = fs::read_to_string(&cedar_path)
        .unwrap_or_else(|err| panic!("read rbac policy at {}: {err}", cedar_path.display()));

    for role in [
        "PlatformAdmin",
        "Admin",
        "Architect",
        "TechLead",
        "Developer",
        "Viewer",
    ] {
        assert!(
            cedar.contains(&format!("principal in Aeterna::Role::\"{role}\"")),
            "rbac.cedar must include role '{role}' so the policy matches the control-plane role catalog"
        );
    }
}

#[test]
fn cedar_policy_contains_project_authorization_actions() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate must live under workspace root");
    let cedar_path = project_root.join("policies/cedar/rbac.cedar");
    let cedar = fs::read_to_string(&cedar_path)
        .unwrap_or_else(|err| panic!("read rbac policy at {}: {err}", cedar_path.display()));

    for action in ["CreateProject", "ManageTeamAssignments"] {
        assert!(
            cedar.contains(&format!("Aeterna::Action::\"{action}\"")),
            "rbac.cedar must include action '{action}' for project authorization"
        );
    }
}
