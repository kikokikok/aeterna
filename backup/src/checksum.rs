/// SHA-256 checksum utilities for backup archive integrity verification.
///
/// Every data file inside a backup archive has its SHA-256 recorded in the
/// manifest. These helpers compute, persist, and verify those checksums.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::File;
use std::hash::BuildHasher;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

/// Result of a single file whose checksum did not match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumMismatch {
    /// Relative filename inside the archive.
    pub filename: String,
    /// The hex-encoded SHA-256 the manifest declared.
    pub expected: String,
    /// The hex-encoded SHA-256 computed from actual bytes.
    pub actual: String,
}

/// Compute the SHA-256 hex digest of an arbitrary byte stream.
pub fn compute_sha256<R: Read>(mut reader: R) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Compute the SHA-256 hex digest of a file on disk.
pub fn compute_file_sha256(path: &Path) -> anyhow::Result<String> {
    let file = File::open(path)?;
    compute_sha256(BufReader::new(file))
}

/// Write a checksums file (one `sha256  filename` pair per line).
pub fn write_checksums_file<S: BuildHasher>(
    path: &Path,
    checksums: &HashMap<String, String, S>,
) -> anyhow::Result<()> {
    let mut file = File::create(path)?;
    let mut entries: Vec<_> = checksums.iter().collect();
    entries.sort_by_key(|(k, _)| (*k).clone());
    for (filename, hash) in entries {
        writeln!(file, "{hash}  {filename}")?;
    }
    Ok(())
}

/// Read a checksums file produced by [`write_checksums_file`].
pub fn read_checksums_file(path: &Path) -> anyhow::Result<HashMap<String, String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut map = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Format: "<hex>  <filename>"
        if let Some((hash, filename)) = trimmed.split_once("  ") {
            map.insert(filename.to_string(), hash.to_string());
        } else {
            anyhow::bail!("malformed checksums line: {trimmed}");
        }
    }
    Ok(map)
}

/// Verify that every file listed in `checksums` exists under `dir` and has
/// the expected SHA-256 digest. Returns a (possibly empty) list of mismatches.
pub fn verify_checksums<S: BuildHasher>(
    dir: &Path,
    checksums: &HashMap<String, String, S>,
) -> anyhow::Result<Vec<ChecksumMismatch>> {
    let mut mismatches = Vec::new();
    for (filename, expected) in checksums {
        let file_path = dir.join(filename);
        if !file_path.exists() {
            mismatches.push(ChecksumMismatch {
                filename: filename.clone(),
                expected: expected.clone(),
                actual: "<missing>".to_string(),
            });
            continue;
        }
        let actual = compute_file_sha256(&file_path)?;
        if actual != *expected {
            mismatches.push(ChecksumMismatch {
                filename: filename.clone(),
                expected: expected.clone(),
                actual,
            });
        }
    }
    Ok(mismatches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn sha256_of_empty() {
        let hash = compute_sha256(Cursor::new(b"")).unwrap();
        // Well-known SHA-256 of empty input
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_of_known_input() {
        let hash = compute_sha256(Cursor::new(b"hello world")).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn file_sha256_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"hello world").unwrap();
        let hash = compute_file_sha256(&file_path).unwrap();
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn write_and_read_checksums_file() {
        let dir = tempfile::tempdir().unwrap();
        let cksum_path = dir.path().join("SHA256SUMS");

        let mut map = HashMap::new();
        map.insert("a.ndjson".to_string(), "aaa111".to_string());
        map.insert("b.ndjson".to_string(), "bbb222".to_string());

        write_checksums_file(&cksum_path, &map).unwrap();
        let back = read_checksums_file(&cksum_path).unwrap();

        assert_eq!(back.len(), 2);
        assert_eq!(back["a.ndjson"], "aaa111");
        assert_eq!(back["b.ndjson"], "bbb222");
    }

    #[test]
    fn verify_checksums_all_ok() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("data.bin");
        std::fs::write(&f, b"payload").unwrap();

        let hash = compute_file_sha256(&f).unwrap();
        let checksums = HashMap::from([("data.bin".to_string(), hash)]);

        let mismatches = verify_checksums(dir.path(), &checksums).unwrap();
        assert!(mismatches.is_empty());
    }

    #[test]
    fn verify_checksums_detects_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("data.bin");
        std::fs::write(&f, b"payload").unwrap();

        let checksums = HashMap::from([("data.bin".to_string(), "wrong_hash".to_string())]);

        let mismatches = verify_checksums(dir.path(), &checksums).unwrap();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].filename, "data.bin");
        assert_eq!(mismatches[0].expected, "wrong_hash");
        assert_ne!(mismatches[0].actual, "wrong_hash");
    }

    #[test]
    fn verify_checksums_detects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let checksums = HashMap::from([("gone.bin".to_string(), "abc".to_string())]);

        let mismatches = verify_checksums(dir.path(), &checksums).unwrap();
        assert_eq!(mismatches.len(), 1);
        assert_eq!(mismatches[0].actual, "<missing>");
    }
}
