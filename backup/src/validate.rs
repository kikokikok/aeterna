/// Offline validation of a backup archive without performing a restore.
///
/// Checks that the archive is structurally sound, the manifest is parseable
/// and schema-compatible, and all file checksums match.
use crate::archive::ArchiveReader;
use crate::checksum::ChecksumMismatch;
use crate::manifest::CURRENT_SCHEMA_VERSION;
use std::path::Path;

/// Summary of an offline archive validation pass.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// `true` if the archive passed all checks.
    pub valid: bool,
    /// Whether the manifest was successfully parsed.
    pub manifest_ok: bool,
    /// Whether the archive schema version is compatible with this build.
    pub schema_compatible: bool,
    /// Any files whose SHA-256 did not match the manifest.
    pub checksum_mismatches: Vec<ChecksumMismatch>,
    /// Human-readable error descriptions collected during validation.
    pub errors: Vec<String>,
}

/// Validate a backup archive on disk without restoring it.
///
/// Opens the archive, checks the manifest, verifies schema compatibility,
/// and confirms that all file checksums match.
pub fn validate_archive(path: &Path) -> anyhow::Result<ValidationReport> {
    let mut report = ValidationReport {
        valid: false,
        manifest_ok: false,
        schema_compatible: false,
        checksum_mismatches: Vec::new(),
        errors: Vec::new(),
    };

    // 1. Try to open and extract the archive.
    let reader = match ArchiveReader::open(path) {
        Ok(r) => r,
        Err(e) => {
            report.errors.push(format!("Failed to open archive: {e}"));
            return Ok(report);
        }
    };

    // 2. Manifest parsed successfully.
    report.manifest_ok = true;

    // 3. Schema compatibility.
    let manifest = reader.manifest();
    report.schema_compatible = manifest.is_schema_compatible();
    if !report.schema_compatible {
        report.errors.push(format!(
            "Schema version incompatible: archive={}, system={}",
            manifest.schema_version, CURRENT_SCHEMA_VERSION
        ));
    }

    // 4. Checksum verification.
    match reader.validate_checksums() {
        Ok(mismatches) => {
            for m in &mismatches {
                report.errors.push(format!(
                    "Checksum mismatch for {}: expected={}, actual={}",
                    m.filename, m.expected, m.actual
                ));
            }
            report.checksum_mismatches = mismatches;
        }
        Err(e) => {
            report
                .errors
                .push(format!("Checksum verification failed: {e}"));
        }
    }

    // 5. Overall verdict.
    report.valid = report.manifest_ok
        && report.schema_compatible
        && report.checksum_mismatches.is_empty()
        && report.errors.is_empty();

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::ArchiveWriter;
    use crate::manifest::{BackupManifest, ExportScope};
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs::File;
    use std::io::BufWriter;

    fn make_valid_archive(dir: &Path) -> std::path::PathBuf {
        let archive_path = dir.join("valid.tar.gz");
        let mut writer = ArchiveWriter::new(&archive_path).unwrap();
        {
            let mut ndjson = writer.create_ndjson_writer("data.ndjson").unwrap();
            ndjson.write_record(&serde_json::json!({"id": 1})).unwrap();
            ndjson.finish().unwrap();
        }
        writer
            .add_manifest(&BackupManifest::new(
                "host".into(),
                ExportScope::FullInstance,
            ))
            .unwrap();
        writer.finalize().unwrap();
        archive_path
    }

    #[test]
    fn valid_archive_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = make_valid_archive(dir.path());

        let report = validate_archive(&path).unwrap();
        assert!(report.valid, "errors: {:?}", report.errors);
        assert!(report.manifest_ok);
        assert!(report.schema_compatible);
        assert!(report.checksum_mismatches.is_empty());
    }

    #[test]
    fn missing_manifest_fails_validation() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("no-manifest.tar.gz");

        // Create empty tar.gz
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        let builder = tar::Builder::new(encoder);
        builder.into_inner().unwrap().finish().unwrap();

        let report = validate_archive(&archive_path).unwrap();
        assert!(!report.valid);
        assert!(!report.manifest_ok);
        assert!(!report.errors.is_empty());
    }

    #[test]
    fn corrupted_archive_fails_validation() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("corrupt.tar.gz");
        std::fs::write(&archive_path, b"this is not a tar.gz").unwrap();

        let report = validate_archive(&archive_path).unwrap();
        assert!(!report.valid);
        assert!(!report.manifest_ok);
    }

    #[test]
    fn incompatible_schema_fails_validation() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("old-schema.tar.gz");

        let mut writer = ArchiveWriter::new(&archive_path).unwrap();
        let mut manifest = BackupManifest::new("host".into(), ExportScope::FullInstance);
        manifest.schema_version = "99.0.0".to_string();
        writer.add_manifest(&manifest).unwrap();
        writer.finalize().unwrap();

        let report = validate_archive(&archive_path).unwrap();
        assert!(!report.valid);
        assert!(report.manifest_ok);
        assert!(!report.schema_compatible);
        assert!(
            report.errors.iter().any(|e| e.contains("incompatible")),
            "errors: {:?}",
            report.errors
        );
    }

    #[test]
    fn checksum_mismatch_fails_validation() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("bad-checksum.tar.gz");

        let mut writer = ArchiveWriter::new(&archive_path).unwrap();
        {
            let mut ndjson = writer.create_ndjson_writer("data.ndjson").unwrap();
            ndjson.write_record(&serde_json::json!({"x": 1})).unwrap();
            ndjson.finish().unwrap();
        }

        // Write manifest with a wrong checksum
        let mut manifest = BackupManifest::new("host".into(), ExportScope::FullInstance);
        manifest
            .file_checksums
            .insert("data.ndjson".to_string(), "0000dead".to_string());
        writer.add_manifest(&manifest).unwrap();

        // Finalize will overwrite the manifest with correct checksums.
        // So instead, we need to build the archive manually to inject a bad checksum.
        // Let's take a different approach: create a valid archive, then tamper.
        drop(writer);

        // Build a valid archive first
        let valid_path = make_valid_archive(dir.path());

        // Extract, tamper with manifest checksums, re-pack
        let extract_dir = dir.path().join("extracted");
        std::fs::create_dir_all(&extract_dir).unwrap();
        {
            let file = File::open(&valid_path).unwrap();
            let decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
            let mut archive = tar::Archive::new(decoder);
            archive.unpack(&extract_dir).unwrap();
        }

        // Tamper with the manifest to set a wrong checksum
        let manifest_path = extract_dir.join("manifest.json");
        let raw = std::fs::read_to_string(&manifest_path).unwrap();
        let mut manifest: BackupManifest = serde_json::from_str(&raw).unwrap();
        for v in manifest.file_checksums.values_mut() {
            *v = "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        }
        let file = File::create(&manifest_path).unwrap();
        serde_json::to_writer_pretty(BufWriter::new(file), &manifest).unwrap();

        // Re-pack
        let tampered_path = dir.path().join("tampered.tar.gz");
        let out_file = File::create(&tampered_path).unwrap();
        let encoder = GzEncoder::new(BufWriter::new(out_file), Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for entry in std::fs::read_dir(&extract_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            builder
                .append_path_with_name(entry.path(), name.to_string_lossy().as_ref())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap();

        let report = validate_archive(&tampered_path).unwrap();
        assert!(!report.valid);
        assert!(!report.checksum_mismatches.is_empty());
    }
}
