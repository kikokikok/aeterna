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
        "platformadmin",
        "admin",
        "architect",
        "techlead",
        "developer",
        "viewer",
    ] {
        assert!(
            cedar.contains(&format!("principal.role == \"{role}\"")),
            "rbac.cedar must include role '{role}' so the policy matches the control-plane role catalog"
        );
    }
}
