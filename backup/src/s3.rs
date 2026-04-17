//! S3 upload, download, list, and delete operations for backup archives.
//!
//! Supports both AWS S3 and S3-compatible stores like MinIO via endpoint
//! override and path-style addressing.
//!
//! # Integration Testing
//!
//! Integration tests for this module require a running MinIO instance.
//! See the project's `docker-compose.yml` for a pre-configured setup.

use anyhow::Context;
use aws_sdk_s3::Client as S3Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Create an S3 client from destination config.
///
/// Mirrors the pattern established in `storage/src/graph_duckdb.rs`.
///
/// # Arguments
///
/// * `region` - AWS region (e.g. `"us-east-1"`). Uses SDK default if `None`.
/// * `endpoint` - Custom endpoint URL for MinIO or other S3-compatible stores.
/// * `force_path_style` - Use path-style addressing (`true` for MinIO).
///
/// # Errors
///
/// Returns an error if the AWS SDK configuration cannot be loaded.
pub async fn create_s3_client(
    region: Option<&str>,
    endpoint: Option<&str>,
    force_path_style: bool,
) -> anyhow::Result<S3Client> {
    use aws_config::BehaviorVersion;

    let mut config_builder = aws_config::defaults(BehaviorVersion::latest());
    if let Some(endpoint) = endpoint {
        config_builder = config_builder.endpoint_url(endpoint);
    }
    if let Some(region) = region {
        config_builder = config_builder.region(aws_config::Region::new(region.to_string()));
    }
    let aws_config = config_builder.load().await;

    if force_path_style {
        let s3_config = aws_sdk_s3::config::Builder::from(&aws_config)
            .force_path_style(true)
            .build();
        Ok(S3Client::from_conf(s3_config))
    } else {
        Ok(S3Client::new(&aws_config))
    }
}

/// Upload a local archive file to S3.
///
/// # Arguments
///
/// * `client` - Configured S3 client.
/// * `bucket` - Target bucket name.
/// * `key` - Object key (path within the bucket).
/// * `local_path` - Path to the local archive file.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the upload fails.
pub async fn upload_archive(
    client: &S3Client,
    bucket: &str,
    key: &str,
    local_path: &Path,
) -> anyhow::Result<()> {
    let body = aws_sdk_s3::primitives::ByteStream::from_path(local_path)
        .await
        .context("Failed to open archive file for upload")?;

    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .content_type("application/gzip")
        .send()
        .await
        .context("Failed to upload archive to S3")?;

    tracing::info!(bucket, key, "Archive uploaded to S3");
    Ok(())
}

/// Download an archive from S3 to a local file.
///
/// Creates parent directories if they do not exist.
///
/// # Arguments
///
/// * `client` - Configured S3 client.
/// * `bucket` - Source bucket name.
/// * `key` - Object key to download.
/// * `local_path` - Local path where the archive will be written.
///
/// # Errors
///
/// Returns an error if the download fails or the file cannot be written.
pub async fn download_archive(
    client: &S3Client,
    bucket: &str,
    key: &str,
    local_path: &Path,
) -> anyhow::Result<()> {
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("Failed to download archive from S3")?;

    let body = resp
        .body
        .collect()
        .await
        .context("Failed to read S3 response body")?;

    std::fs::create_dir_all(local_path.parent().unwrap_or(Path::new(".")))?;
    std::fs::write(local_path, body.into_bytes())
        .context("Failed to write downloaded archive to disk")?;

    tracing::info!(bucket, key, path = %local_path.display(), "Archive downloaded from S3");
    Ok(())
}

/// List export archives in S3 for a given tenant.
///
/// Returns all objects under `{prefix}/{tenant_id}/exports/`.
///
/// # Arguments
///
/// * `client` - Configured S3 client.
/// * `bucket` - Bucket to list from.
/// * `prefix` - Key prefix (may be empty).
/// * `tenant_id` - Tenant identifier.
///
/// # Errors
///
/// Returns an error if the S3 list operation fails.
pub async fn list_archives(
    client: &S3Client,
    bucket: &str,
    prefix: &str,
    tenant_id: &str,
) -> anyhow::Result<Vec<ArchiveEntry>> {
    let search_prefix = if prefix.is_empty() {
        format!("{tenant_id}/exports/")
    } else {
        format!("{prefix}/{tenant_id}/exports/")
    };

    let resp = client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(&search_prefix)
        .send()
        .await
        .context("Failed to list archives in S3")?;

    let entries = resp
        .contents()
        .iter()
        .filter_map(|obj| {
            Some(ArchiveEntry {
                key: obj.key()?.to_string(),
                size_bytes: obj.size().unwrap_or(0) as u64,
                last_modified: obj.last_modified()?.to_string(),
            })
        })
        .collect();

    Ok(entries)
}

/// Delete an archive from S3.
///
/// # Arguments
///
/// * `client` - Configured S3 client.
/// * `bucket` - Bucket containing the object.
/// * `key` - Object key to delete.
///
/// # Errors
///
/// Returns an error if the deletion fails.
pub async fn delete_archive(client: &S3Client, bucket: &str, key: &str) -> anyhow::Result<()> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .context("Failed to delete archive from S3")?;

    tracing::info!(bucket, key, "Archive deleted from S3");
    Ok(())
}

/// Metadata for an archive object stored in S3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveEntry {
    /// Full object key in the bucket.
    pub key: String,
    /// Object size in bytes.
    pub size_bytes: u64,
    /// Last-modified timestamp (ISO 8601 string from S3).
    pub last_modified: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_entry_serde_round_trip() {
        let entry = ArchiveEntry {
            key: "prod/acme/exports/20260412_120000-full.tar.gz".into(),
            size_bytes: 1024,
            last_modified: "2026-04-12T12:00:00Z".into(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        let deserialized: ArchiveEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.key, entry.key);
        assert_eq!(deserialized.size_bytes, 1024);
        assert_eq!(deserialized.last_modified, "2026-04-12T12:00:00Z");
    }

    #[test]
    fn archive_entry_debug_and_clone() {
        let entry = ArchiveEntry {
            key: "test/key".into(),
            size_bytes: 42,
            last_modified: "2026-01-01T00:00:00Z".into(),
        };
        let cloned = entry.clone();
        assert_eq!(format!("{cloned:?}"), format!("{:?}", entry));
    }

    // Integration tests for upload_archive, download_archive, list_archives,
    // delete_archive, and create_s3_client require a running MinIO instance.
    // Run them with: `docker-compose up -d && cargo test -p aeterna-backup -- --ignored`
}
