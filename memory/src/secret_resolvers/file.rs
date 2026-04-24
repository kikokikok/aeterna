//! B4 §3.3 — [`FileRefResolver`].
//!
//! Resolves `SecretReference::File { path }` by reading an absolute
//! path on the server's local filesystem.
//!
//! Security contract:
//!
//! * **Absolute path required.** Relative paths are rejected at
//!   resolve time (independently of whatever the manifest validator
//!   did on apply). This is a defense-in-depth check against bugs
//!   that let a relative path slip past.
//! * **Mode check (Unix).** The file's mode is checked to exclude
//!   permission bits beyond owner read/write. Anything matching
//!   `mode & 0o077 != 0` — i.e. any group or world bit — fails with
//!   [`ResolveError::PermissionDenied`]. This matches the behaviour
//!   of `ssh` on `~/.ssh` files.
//! * **Size cap.** Files larger than 1 MiB are refused as malformed
//!   (any secret material that large is almost certainly a misuse).
//! * **Symlinks are followed.** We rely on the container image /
//!   K8s volume mount to provide the correct view; we do not try to
//!   harden against symlink-swap races (TOCTOU) on a trusted
//!   filesystem.

use std::path::Path;

use async_trait::async_trait;
use mk_core::SecretBytes;
use mk_core::secret::SecretReference;
use mk_core::types::TenantId;
use tokio::io::AsyncReadExt;

use crate::secret_resolver::{ResolveError, SecretRefResolver};

/// Maximum file size we accept, in bytes. Above this, refuse.
const MAX_SECRET_FILE_BYTES: u64 = 1024 * 1024;

/// Resolver for [`SecretReference::File`].
#[derive(Debug, Default, Clone, Copy)]
pub struct FileRefResolver;

impl FileRefResolver {
    pub fn new() -> Self {
        Self
    }

    /// Unix mode policy: owner r/w only. Returns `Err` with an
    /// explanation if the file has any group or world permissions.
    #[cfg(unix)]
    async fn check_mode(path: &Path) -> Result<(), ResolveError> {
        use std::os::unix::fs::MetadataExt;
        let md = tokio::fs::metadata(path)
            .await
            .map_err(|e| ResolveError::BackendUnavailable {
                kind: "file",
                reason: format!("stat failed: {e}"),
            })?;
        let mode = md.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(ResolveError::PermissionDenied {
                kind: "file",
                reason: format!(
                    "file mode {mode:03o} has group or world bits set; expected owner-only (e.g. 0600)"
                ),
            });
        }
        Ok(())
    }

    #[cfg(not(unix))]
    async fn check_mode(_path: &Path) -> Result<(), ResolveError> {
        // Non-Unix platforms don't have a comparable mode concept.
        // The production server runs on Linux; this path exists only
        // to keep the crate buildable on dev Windows machines.
        Ok(())
    }
}

#[async_trait]
impl SecretRefResolver for FileRefResolver {
    fn kind(&self) -> &'static str {
        "file"
    }

    async fn resolve(
        &self,
        tenant: &TenantId,
        reference: &SecretReference,
    ) -> Result<SecretBytes, ResolveError> {
        let SecretReference::File { path } = reference else {
            return Err(ResolveError::WrongKind {
                expected: "file",
                actual: reference.kind(),
            });
        };

        if path.is_empty() {
            return Err(ResolveError::MalformedReference {
                kind: "file",
                reason: "path is empty".to_string(),
            });
        }

        let p = Path::new(path);
        if !p.is_absolute() {
            return Err(ResolveError::MalformedReference {
                kind: "file",
                reason: format!("path is not absolute: {path}"),
            });
        }

        // Size check up front — fails fast on obviously-wrong inputs.
        let md = match tokio::fs::metadata(p).await {
            Ok(md) => md,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ResolveError::NotFound {
                    tenant: tenant.clone(),
                    kind: "file",
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(ResolveError::PermissionDenied {
                    kind: "file",
                    reason: format!("os permission denied on stat: {e}"),
                });
            }
            Err(e) => {
                return Err(ResolveError::BackendUnavailable {
                    kind: "file",
                    reason: format!("stat failed: {e}"),
                });
            }
        };

        if !md.is_file() {
            return Err(ResolveError::MalformedReference {
                kind: "file",
                reason: format!("path is not a regular file: {path}"),
            });
        }
        if md.len() > MAX_SECRET_FILE_BYTES {
            return Err(ResolveError::MalformedReference {
                kind: "file",
                reason: format!(
                    "file size {} exceeds {MAX_SECRET_FILE_BYTES}-byte limit",
                    md.len()
                ),
            });
        }

        Self::check_mode(p).await?;

        let mut file = tokio::fs::File::open(p).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => ResolveError::PermissionDenied {
                kind: "file",
                reason: format!("os permission denied on open: {e}"),
            },
            _ => ResolveError::BackendUnavailable {
                kind: "file",
                reason: format!("open failed: {e}"),
            },
        })?;

        let mut buf = Vec::with_capacity(md.len() as usize);
        file.read_to_end(&mut buf)
            .await
            .map_err(|e| ResolveError::BackendUnavailable {
                kind: "file",
                reason: format!("read failed: {e}"),
            })?;

        Ok(SecretBytes::from(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tid() -> TenantId {
        TenantId::new("11111111-1111-1111-1111-111111111111".to_string()).unwrap()
    }

    async fn write_file(dir: &TempDir, name: &str, contents: &[u8], mode: u32) -> String {
        let path = dir.path().join(name);
        tokio::fs::write(&path, contents).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(&path).await.unwrap().permissions();
            perms.set_mode(mode);
            tokio::fs::set_permissions(&path, perms).await.unwrap();
        }
        #[cfg(not(unix))]
        let _ = mode;
        path.to_string_lossy().into_owned()
    }

    fn file_ref(p: &str) -> SecretReference {
        SecretReference::File {
            path: p.to_string(),
        }
    }

    #[tokio::test]
    async fn reports_kind_file() {
        assert_eq!(FileRefResolver::new().kind(), "file");
    }

    #[tokio::test]
    async fn reads_owner_only_file() {
        let dir = TempDir::new().unwrap();
        let p = write_file(&dir, "ok.secret", b"topsecret", 0o600).await;
        let r = FileRefResolver::new();
        let out = r.resolve(&tid(), &file_ref(&p)).await.unwrap();
        assert_eq!(out.expose(), b"topsecret");
    }

    #[tokio::test]
    async fn missing_file_is_not_found() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("never-created");
        let r = FileRefResolver::new();
        let err = r
            .resolve(&tid(), &file_ref(missing.to_str().unwrap()))
            .await
            .unwrap_err();
        assert!(matches!(err, ResolveError::NotFound { kind: "file", .. }));
    }

    #[tokio::test]
    async fn empty_path_is_malformed() {
        let r = FileRefResolver::new();
        let err = r.resolve(&tid(), &file_ref("")).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::MalformedReference { kind: "file", .. }
        ));
    }

    #[tokio::test]
    async fn relative_path_is_malformed() {
        let r = FileRefResolver::new();
        let err = r
            .resolve(&tid(), &file_ref("relative/path"))
            .await
            .unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "file");
                assert!(reason.contains("not absolute"), "{reason}");
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn wrong_kind_is_rejected() {
        let r = FileRefResolver::new();
        let env = SecretReference::Env {
            var: "X".to_string(),
        };
        let err = r.resolve(&tid(), &env).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::WrongKind {
                expected: "file",
                actual: "env"
            }
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn group_readable_file_is_permission_denied() {
        let dir = TempDir::new().unwrap();
        let p = write_file(&dir, "leaky.secret", b"oops", 0o640).await;
        let r = FileRefResolver::new();
        let err = r.resolve(&tid(), &file_ref(&p)).await.unwrap_err();
        match err {
            ResolveError::PermissionDenied { kind, reason } => {
                assert_eq!(kind, "file");
                assert!(reason.contains("640"), "{reason}");
            }
            other => panic!("expected PermissionDenied, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn world_readable_file_is_permission_denied() {
        let dir = TempDir::new().unwrap();
        let p = write_file(&dir, "world.secret", b"oops", 0o604).await;
        let r = FileRefResolver::new();
        let err = r.resolve(&tid(), &file_ref(&p)).await.unwrap_err();
        assert!(matches!(
            err,
            ResolveError::PermissionDenied { kind: "file", .. }
        ));
    }

    #[tokio::test]
    async fn directory_is_malformed() {
        let dir = TempDir::new().unwrap();
        let r = FileRefResolver::new();
        let err = r
            .resolve(&tid(), &file_ref(dir.path().to_str().unwrap()))
            .await
            .unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "file");
                assert!(reason.contains("not a regular file"), "{reason}");
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn oversized_file_is_malformed() {
        let dir = TempDir::new().unwrap();
        // Use 2 MiB to exceed the 1 MiB cap. Mode 0600 so we don't trip
        // the permission check first.
        let big = vec![0u8; (MAX_SECRET_FILE_BYTES as usize) + 1024];
        let p = write_file(&dir, "big.secret", &big, 0o600).await;
        let r = FileRefResolver::new();
        let err = r.resolve(&tid(), &file_ref(&p)).await.unwrap_err();
        match err {
            ResolveError::MalformedReference { kind, reason } => {
                assert_eq!(kind, "file");
                assert!(reason.contains("exceeds"), "{reason}");
            }
            other => panic!("expected MalformedReference, got {other:?}"),
        }
    }
}
