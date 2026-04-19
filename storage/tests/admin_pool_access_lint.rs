//! Static lint: no code outside `with_admin_context` references
//! `admin_pool()` directly.
//!
//! (Bundle A.2, task 3.3.7 — warn-level here, graduates to
//! deny-level in Bundle A.3 Wave 6 once every legitimate call site has
//! been refactored through the helper.)
//!
//! The lint scans every `.rs` source file under the workspace (sans
//! `target/` and the helper's own file) for the literal substring
//! `.admin_pool()`. Any hit outside the exempt list is a candidate
//! RLS-bypass site that skips the audit write.
//!
//! Warn-level means: if this test panics, CI is still green but a
//! `stderr` notice appears with the offending path. The graduation in
//! A.3 Wave 6 swaps the `eprintln!` for a `panic!`.

use std::fs;
use std::path::{Path, PathBuf};

/// Files allowed to mention `.admin_pool()`:
/// - `storage/src/postgres.rs` — defines the helper itself.
/// - `storage/tests/admin_pool_access_lint.rs` — this file.
/// - `storage/tests/rls_admin_surface_test.rs` — verifies the helper.
const EXEMPT_SUFFIXES: &[&str] = &[
    "storage/src/postgres.rs",
    "storage/tests/admin_pool_access_lint.rs",
    "storage/tests/rls_admin_surface_test.rs",
];

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at storage/ during test execution;
    // the workspace root is one level up.
    let storage_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    storage_crate
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(storage_crate)
}

fn visit(dir: &Path, hits: &mut Vec<(PathBuf, usize, String)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if matches!(
                name,
                "target" | ".git" | "node_modules" | ".playwright-dust"
            ) {
                continue;
            }
            visit(&path, hits);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let normalized = path.to_string_lossy().replace('\\', "/");
            if EXEMPT_SUFFIXES.iter().any(|s| normalized.ends_with(s)) {
                continue;
            }
            for (lineno, line) in content.lines().enumerate() {
                // Look for the method call, not a field access or string mention.
                if line.contains(".admin_pool()") {
                    hits.push((path.clone(), lineno + 1, line.trim().to_string()));
                }
            }
        }
    }
}

#[test]
fn no_direct_admin_pool_access_outside_helper() {
    let root = workspace_root();
    let mut hits = Vec::new();
    visit(&root, &mut hits);

    if !hits.is_empty() {
        // Warn-level: print and continue. Graduates to panic in A.3 Wave 6.
        eprintln!("\n⚠️  admin_pool_access_lint (WARN — graduates to deny in A.3 Wave 6):");
        eprintln!(
            "   Found {} direct `.admin_pool()` call(s) outside with_admin_context:",
            hits.len()
        );
        for (path, lineno, line) in &hits {
            eprintln!("     {}:{}  {}", path.display(), lineno, line);
        }
        eprintln!(
            "   These call sites bypass the admin audit write. Refactor them \
             through `PostgresBackend::with_admin_context` before A.3 Wave 6.\n"
        );
    }
}
