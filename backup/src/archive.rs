/// Archive writer and reader for `.tar.gz` backup bundles.
///
/// The writer accumulates files in a temporary directory, computes SHA-256
/// checksums, embeds them in the manifest, then packages everything into a
/// single compressed tarball. The reader extracts the tarball, validates the
/// manifest, and provides streaming access to each NDJSON data file.
use crate::checksum;
use crate::manifest::BackupManifest;
use crate::ndjson::{NdjsonReader, NdjsonWriter};
use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::de::DeserializeOwned;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

/// Builds a backup archive in a temporary directory, then packages it as
/// `tar.gz`.
pub struct ArchiveWriter {
    /// The final output path for the `.tar.gz` file.
    output_path: PathBuf,
    /// Temporary directory where files are staged before tarballing.
    temp_dir: tempfile::TempDir,
    /// Filenames of NDJSON data files created so far (relative to temp_dir).
    data_files: Vec<String>,
}

impl ArchiveWriter {
    /// Create a new writer that will produce the archive at `output_path`.
    pub fn new(output_path: &Path) -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        Ok(Self {
            output_path: output_path.to_path_buf(),
            temp_dir,
            data_files: Vec::new(),
        })
    }

    /// Write the manifest as `manifest.json` in the staging directory.
    pub fn add_manifest(&mut self, manifest: &BackupManifest) -> anyhow::Result<()> {
        let manifest_path = self.temp_dir.path().join("manifest.json");
        let file = File::create(&manifest_path)?;
        serde_json::to_writer_pretty(BufWriter::new(file), manifest)?;
        Ok(())
    }

    /// Create a new NDJSON data file inside the staging directory and return
    /// a streaming writer for it. The caller writes records and then drops
    /// (or calls `finish()` on) the writer.
    pub fn create_ndjson_writer(&mut self, filename: &str) -> anyhow::Result<NdjsonWriter<File>> {
        let file_path = self.temp_dir.path().join(filename);
        let file = File::create(file_path)?;
        self.data_files.push(filename.to_string());
        Ok(NdjsonWriter::new(file))
    }

    /// Compute checksums for every data file, update the manifest, and
    /// produce the final `.tar.gz` archive. Returns the path to the archive.
    pub fn finalize(self) -> anyhow::Result<PathBuf> {
        // 1. Compute checksums for all data files.
        let mut checksums = std::collections::HashMap::new();
        for filename in &self.data_files {
            let path = self.temp_dir.path().join(filename);
            if path.exists() {
                let hash = checksum::compute_file_sha256(&path)?;
                checksums.insert(filename.clone(), hash);
            }
        }

        // 2. Write the checksums file.
        let checksums_path = self.temp_dir.path().join("SHA256SUMS");
        checksum::write_checksums_file(&checksums_path, &checksums)?;

        // 3. Re-read the existing manifest, patch in checksums, and re-write.
        let manifest_path = self.temp_dir.path().join("manifest.json");
        if manifest_path.exists() {
            let raw = std::fs::read_to_string(&manifest_path)?;
            let mut manifest: BackupManifest = serde_json::from_str(&raw)?;
            manifest.file_checksums = checksums;
            let file = File::create(&manifest_path)?;
            serde_json::to_writer_pretty(BufWriter::new(file), &manifest)?;
        }

        // 4. Package everything into a tar.gz.
        let archive_file = File::create(&self.output_path)?;
        let encoder = GzEncoder::new(BufWriter::new(archive_file), Compression::default());
        let mut tar_builder = tar::Builder::new(encoder);

        // Walk the temp directory and add every file.
        for entry in std::fs::read_dir(self.temp_dir.path())? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            tar_builder.append_path_with_name(entry.path(), name_str.as_ref())?;
        }

        tar_builder.into_inner()?.finish()?;
        Ok(self.output_path.clone())
    }
}

/// Reads and validates a `.tar.gz` backup archive.
#[derive(Debug)]
pub struct ArchiveReader {
    /// Temporary directory holding the extracted archive contents.
    _temp_dir: tempfile::TempDir,
    /// Path to the extracted directory.
    extract_dir: PathBuf,
    /// The parsed manifest from the archive.
    manifest: BackupManifest,
}

impl ArchiveReader {
    /// Open and extract a `.tar.gz` backup archive, then parse its manifest.
    pub fn open(archive_path: &Path) -> anyhow::Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let extract_dir = temp_dir.path().to_path_buf();

        // Extract tar.gz
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(&extract_dir)?;

        // Parse manifest
        let manifest_path = extract_dir.join("manifest.json");
        if !manifest_path.exists() {
            anyhow::bail!("Archive does not contain manifest.json");
        }
        let manifest_raw = std::fs::read_to_string(&manifest_path)?;
        let manifest: BackupManifest = serde_json::from_str(&manifest_raw)?;

        Ok(Self {
            _temp_dir: temp_dir,
            extract_dir,
            manifest,
        })
    }

    /// Return a reference to the parsed manifest.
    pub fn manifest(&self) -> &BackupManifest {
        &self.manifest
    }

    /// Verify all file checksums declared in the manifest against the
    /// extracted files on disk.
    pub fn validate_checksums(&self) -> anyhow::Result<Vec<checksum::ChecksumMismatch>> {
        checksum::verify_checksums(&self.extract_dir, &self.manifest.file_checksums)
    }

    /// Open one of the NDJSON data files inside the archive for streaming
    /// typed deserialization.
    pub fn open_ndjson<T: DeserializeOwned>(
        &self,
        filename: &str,
    ) -> anyhow::Result<NdjsonReader<BufReader<File>>> {
        let path = self.extract_dir.join(filename);
        if !path.exists() {
            anyhow::bail!("Data file not found in archive: {filename}");
        }
        let file = File::open(path)?;
        Ok(NdjsonReader::new(BufReader::new(file)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{EntityCounts, ExportScope};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
    struct Row {
        id: u64,
        text: String,
    }

    fn make_manifest() -> BackupManifest {
        BackupManifest::new("test-node".into(), ExportScope::FullInstance)
    }

    #[test]
    fn full_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("backup.tar.gz");

        // ---- Write ----
        let mut writer = ArchiveWriter::new(&archive_path).unwrap();

        let mut manifest = make_manifest();
        manifest.entity_counts = EntityCounts {
            memories: 3,
            ..Default::default()
        };

        // Write data file first
        {
            let mut ndjson = writer.create_ndjson_writer("memories.ndjson").unwrap();
            for i in 0..3 {
                ndjson
                    .write_record(&Row {
                        id: i,
                        text: format!("mem-{i}"),
                    })
                    .unwrap();
            }
            ndjson.finish().unwrap();
        }

        writer.add_manifest(&manifest).unwrap();
        let result_path = writer.finalize().unwrap();
        assert!(result_path.exists());

        // ---- Read ----
        let reader = ArchiveReader::open(&result_path).unwrap();
        let m = reader.manifest();
        assert_eq!(m.schema_version, "1.0.0");
        assert_eq!(m.entity_counts.memories, 3);

        // Checksum verification
        let mismatches = reader.validate_checksums().unwrap();
        assert!(
            mismatches.is_empty(),
            "unexpected mismatches: {mismatches:?}"
        );

        // Read back NDJSON data
        let ndjson_reader = reader.open_ndjson::<Row>("memories.ndjson").unwrap();
        let rows: Vec<Row> = ndjson_reader
            .map(|r| {
                let val = r.unwrap();
                serde_json::from_value(val).unwrap()
            })
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].id, 0);
        assert_eq!(rows[2].text, "mem-2");
    }

    #[test]
    fn missing_manifest_fails() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("bad.tar.gz");

        // Create an empty tar.gz
        let file = File::create(&archive_path).unwrap();
        let encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        let builder = tar::Builder::new(encoder);
        builder.into_inner().unwrap().finish().unwrap();

        let result = ArchiveReader::open(&archive_path);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("manifest.json"), "got: {err_msg}");
    }

    #[test]
    fn open_missing_ndjson_fails() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("backup.tar.gz");

        let mut writer = ArchiveWriter::new(&archive_path).unwrap();
        writer.add_manifest(&make_manifest()).unwrap();
        writer.finalize().unwrap();

        let reader = ArchiveReader::open(&archive_path).unwrap();
        let result = reader.open_ndjson::<serde_json::Value>("nonexistent.ndjson");
        assert!(result.is_err());
    }

    #[test]
    fn checksums_embedded_in_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("backup.tar.gz");

        let mut writer = ArchiveWriter::new(&archive_path).unwrap();
        {
            let mut ndjson = writer.create_ndjson_writer("data.ndjson").unwrap();
            ndjson.write_record(&serde_json::json!({"x": 1})).unwrap();
            ndjson.finish().unwrap();
        }
        writer.add_manifest(&make_manifest()).unwrap();
        writer.finalize().unwrap();

        let reader = ArchiveReader::open(&archive_path).unwrap();
        assert!(
            reader.manifest().file_checksums.contains_key("data.ndjson"),
            "checksums should include data.ndjson"
        );
    }

    #[test]
    fn multiple_data_files() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("backup.tar.gz");

        let mut writer = ArchiveWriter::new(&archive_path).unwrap();

        for name in &["memories.ndjson", "knowledge.ndjson", "policies.ndjson"] {
            let mut ndjson = writer.create_ndjson_writer(name).unwrap();
            ndjson
                .write_record(&serde_json::json!({"file": name}))
                .unwrap();
            ndjson.finish().unwrap();
        }

        writer.add_manifest(&make_manifest()).unwrap();
        writer.finalize().unwrap();

        let reader = ArchiveReader::open(&archive_path).unwrap();
        assert_eq!(reader.manifest().file_checksums.len(), 3);
        let mismatches = reader.validate_checksums().unwrap();
        assert!(mismatches.is_empty());
    }
}
