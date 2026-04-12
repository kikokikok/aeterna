/// Error types for the backup/restore system.
///
/// Uses `thiserror` to provide structured, displayable error variants
/// covering all failure modes during archive creation, validation, and
/// restoration.
use thiserror::Error;

/// Errors that can occur during backup or restore operations.
#[derive(Error, Debug)]
pub enum BackupError {
    /// The archive manifest is missing or could not be parsed.
    #[error("Archive manifest missing or invalid: {0}")]
    ManifestError(String),

    /// The archive schema version is not compatible with the running system.
    #[error("Schema version incompatible: archive={archive}, system={system}")]
    SchemaIncompatible {
        /// Schema version found in the archive.
        archive: String,
        /// Schema version expected by this build.
        system: String,
    },

    /// A file inside the archive did not match its expected SHA-256 checksum.
    #[error("Checksum mismatch for {filename}: expected={expected}, actual={actual}")]
    ChecksumMismatch {
        /// The file whose checksum failed verification.
        filename: String,
        /// The hex-encoded SHA-256 that the manifest declared.
        expected: String,
        /// The hex-encoded SHA-256 computed from the actual file bytes.
        actual: String,
    },

    /// The archive is structurally corrupt (e.g. truncated tar, bad gzip).
    #[error("Archive corruption: {0}")]
    CorruptArchive(String),

    /// An underlying I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization / deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_manifest_error() {
        let err = BackupError::ManifestError("file not found".into());
        assert_eq!(
            err.to_string(),
            "Archive manifest missing or invalid: file not found"
        );
    }

    #[test]
    fn display_schema_incompatible() {
        let err = BackupError::SchemaIncompatible {
            archive: "0.9.0".into(),
            system: "1.0.0".into(),
        };
        assert!(err
            .to_string()
            .contains("archive=0.9.0, system=1.0.0"));
    }

    #[test]
    fn display_checksum_mismatch() {
        let err = BackupError::ChecksumMismatch {
            filename: "data.ndjson".into(),
            expected: "aaa".into(),
            actual: "bbb".into(),
        };
        assert!(err.to_string().contains("data.ndjson"));
        assert!(err.to_string().contains("expected=aaa"));
    }

    #[test]
    fn display_corrupt_archive() {
        let err = BackupError::CorruptArchive("truncated".into());
        assert_eq!(err.to_string(), "Archive corruption: truncated");
    }

    #[test]
    fn io_error_converts() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: BackupError = io_err.into();
        assert!(err.to_string().contains("gone"));
    }

    #[test]
    fn serde_error_converts() {
        let serde_err = serde_json::from_str::<String>("not json").unwrap_err();
        let err: BackupError = serde_err.into();
        assert!(err.to_string().contains("Serialization error"));
    }
}
