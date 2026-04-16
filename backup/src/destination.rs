//! Export destination abstraction for backup archives.
//!
//! Defines where export archives are stored — either on the local filesystem
//! (for dev/testing) or in S3/S3-compatible object storage (for production).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where export archives are stored.
///
/// Tagged enum that serializes as `{"type": "local", ...}` or `{"type": "s3", ...}`.
///
/// # Examples
///
/// ```
/// use aeterna_backup::destination::ExportDestination;
///
/// let local = ExportDestination::Local {
///     path: "/tmp/exports".into(),
/// };
/// let key = local.archive_key("acme", "20260412_120000", "full");
/// assert!(key.contains("acme"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExportDestination {
    /// Local filesystem (dev/testing only).
    Local {
        /// Directory where archives are stored.
        path: PathBuf,
    },
    /// S3 or S3-compatible object store (MinIO, etc.).
    S3 {
        /// Bucket name.
        bucket: String,
        /// Key prefix, e.g. `"prod"` or `"prod/acme-corp/exports"`.
        prefix: String,
        /// AWS region. Defaults to `us-east-1` when `None`.
        region: Option<String>,
        /// Custom endpoint URL for MinIO or other S3-compatible stores.
        endpoint: Option<String>,
        /// Use path-style addressing (`true` for MinIO).
        force_path_style: bool,
    },
}

/// Configuration for export storage, resolved from platform + tenant config.
///
/// The tenant-level override takes precedence over the platform default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStorageConfig {
    /// Platform-level default destination.
    pub platform_default: ExportDestination,
    /// Optional tenant-level override.
    pub tenant_override: Option<ExportDestination>,
}

impl ExportStorageConfig {
    /// Resolve the effective destination (tenant override > platform default).
    pub fn effective_destination(&self) -> &ExportDestination {
        self.tenant_override
            .as_ref()
            .unwrap_or(&self.platform_default)
    }
}

impl ExportDestination {
    /// Build the full archive key/path for a given tenant and timestamp.
    ///
    /// For S3: `{prefix}/{tenant_id}/exports/{timestamp}-{scope}.tar.gz`
    /// For Local: `{path}/{tenant_id}/exports/{timestamp}-{scope}.tar.gz`
    ///
    /// # Examples
    ///
    /// ```
    /// use aeterna_backup::destination::ExportDestination;
    ///
    /// let s3 = ExportDestination::S3 {
    ///     bucket: "my-bucket".into(),
    ///     prefix: "prod".into(),
    ///     region: None,
    ///     endpoint: None,
    ///     force_path_style: false,
    /// };
    /// let key = s3.archive_key("acme", "20260412_120000", "full");
    /// assert_eq!(key, "prod/acme/exports/20260412_120000-full.tar.gz");
    /// ```
    pub fn archive_key(&self, tenant_id: &str, timestamp: &str, scope: &str) -> String {
        let filename = format!("{timestamp}-{scope}.tar.gz");
        match self {
            Self::Local { path } => path
                .join(tenant_id)
                .join("exports")
                .join(&filename)
                .to_string_lossy()
                .to_string(),
            Self::S3 { prefix, .. } => {
                if prefix.is_empty() {
                    format!("{tenant_id}/exports/{filename}")
                } else {
                    format!("{prefix}/{tenant_id}/exports/{filename}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_key_local() {
        let dest = ExportDestination::Local {
            path: PathBuf::from("/data/backups"),
        };
        let key = dest.archive_key("tenant-1", "20260412_120000", "full");
        assert_eq!(
            key,
            "/data/backups/tenant-1/exports/20260412_120000-full.tar.gz"
        );
    }

    #[test]
    fn archive_key_s3_with_prefix() {
        let dest = ExportDestination::S3 {
            bucket: "my-bucket".into(),
            prefix: "prod/region-a".into(),
            region: None,
            endpoint: None,
            force_path_style: false,
        };
        let key = dest.archive_key("acme", "20260412_120000", "incremental");
        assert_eq!(
            key,
            "prod/region-a/acme/exports/20260412_120000-incremental.tar.gz"
        );
    }

    #[test]
    fn archive_key_s3_empty_prefix() {
        let dest = ExportDestination::S3 {
            bucket: "my-bucket".into(),
            prefix: String::new(),
            region: None,
            endpoint: None,
            force_path_style: false,
        };
        let key = dest.archive_key("acme", "20260412_120000", "full");
        assert_eq!(key, "acme/exports/20260412_120000-full.tar.gz");
    }

    #[test]
    fn effective_destination_returns_tenant_override() {
        let config = ExportStorageConfig {
            platform_default: ExportDestination::Local {
                path: PathBuf::from("/default"),
            },
            tenant_override: Some(ExportDestination::S3 {
                bucket: "tenant-bucket".into(),
                prefix: "override".into(),
                region: None,
                endpoint: None,
                force_path_style: false,
            }),
        };
        match config.effective_destination() {
            ExportDestination::S3 { bucket, .. } => assert_eq!(bucket, "tenant-bucket"),
            _ => panic!("Expected S3 destination from tenant override"),
        }
    }

    #[test]
    fn effective_destination_returns_platform_default() {
        let config = ExportStorageConfig {
            platform_default: ExportDestination::Local {
                path: PathBuf::from("/default"),
            },
            tenant_override: None,
        };
        match config.effective_destination() {
            ExportDestination::Local { path } => {
                assert_eq!(path, &PathBuf::from("/default"));
            }
            _ => panic!("Expected Local destination from platform default"),
        }
    }

    #[test]
    fn serde_round_trip_local() {
        let dest = ExportDestination::Local {
            path: PathBuf::from("/tmp/exports"),
        };
        let json = serde_json::to_string(&dest).expect("serialize");
        let deserialized: ExportDestination = serde_json::from_str(&json).expect("deserialize");
        match deserialized {
            ExportDestination::Local { path } => {
                assert_eq!(path, PathBuf::from("/tmp/exports"));
            }
            _ => panic!("Expected Local variant after round-trip"),
        }
    }

    #[test]
    fn serde_round_trip_s3() {
        let dest = ExportDestination::S3 {
            bucket: "my-bucket".into(),
            prefix: "prod".into(),
            region: Some("eu-west-1".into()),
            endpoint: Some("http://minio:9000".into()),
            force_path_style: true,
        };
        let json = serde_json::to_string(&dest).expect("serialize");
        let deserialized: ExportDestination = serde_json::from_str(&json).expect("deserialize");
        match deserialized {
            ExportDestination::S3 {
                bucket,
                prefix,
                region,
                endpoint,
                force_path_style,
            } => {
                assert_eq!(bucket, "my-bucket");
                assert_eq!(prefix, "prod");
                assert_eq!(region, Some("eu-west-1".into()));
                assert_eq!(endpoint, Some("http://minio:9000".into()));
                assert!(force_path_style);
            }
            _ => panic!("Expected S3 variant after round-trip"),
        }
    }

    #[test]
    fn serde_round_trip_storage_config() {
        let config = ExportStorageConfig {
            platform_default: ExportDestination::Local {
                path: PathBuf::from("/data"),
            },
            tenant_override: Some(ExportDestination::S3 {
                bucket: "b".into(),
                prefix: "p".into(),
                region: None,
                endpoint: None,
                force_path_style: false,
            }),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: ExportStorageConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.tenant_override.is_some());
    }
}
