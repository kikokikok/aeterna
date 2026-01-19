use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use mk_core::types::{ErrorSignature, HindsightNote, Resolution};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, info_span, instrument};

#[derive(Debug, Clone)]
pub struct DeduplicationConfig {
    pub similarity_threshold: f32,
    pub embedding_weight: f32,
    pub message_weight: f32,
    pub context_weight: f32,
    pub stack_weight: f32,
    pub background_scan_interval_secs: u64,
    pub max_signatures_per_scan: usize,
}

impl Default for DeduplicationConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.95,
            embedding_weight: 0.5,
            message_weight: 0.3,
            context_weight: 0.1,
            stack_weight: 0.1,
            background_scan_interval_secs: 3600,
            max_signatures_per_scan: 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedSignature {
    pub id: String,
    pub error_type: String,
    pub normalized_message: String,
    pub context_hash: u64,
    pub embedding: Option<Vec<f32>>,
    pub created_at: i64,
    pub merge_count: u32,
}

impl IndexedSignature {
    pub fn from_signature(id: impl Into<String>, sig: &ErrorSignature) -> Self {
        let context_hash = compute_context_hash(&sig.context_patterns);
        Self {
            id: id.into(),
            error_type: sig.error_type.clone(),
            normalized_message: sig.message_pattern.clone(),
            context_hash,
            embedding: sig.embedding.clone(),
            created_at: current_timestamp(),
            merge_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeduplicationMetrics {
    pub duplicates_detected: u64,
    pub duplicates_merged: u64,
    pub unique_signatures: u64,
    pub last_scan_at: Option<i64>,
    pub last_scan_duration_ms: Option<u64>,
    pub scans_completed: u64,
}

impl Default for DeduplicationMetrics {
    fn default() -> Self {
        Self {
            duplicates_detected: 0,
            duplicates_merged: 0,
            unique_signatures: 0,
            last_scan_at: None,
            last_scan_duration_ms: None,
            scans_completed: 0,
        }
    }
}

#[derive(Debug, Error)]
pub enum DeduplicationError {
    #[error("Index error: {0}")]
    Index(String),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Merge error: {0}")]
    Merge(String),
}

#[async_trait]
pub trait SignatureStorage: Send + Sync {
    async fn get_signature(&self, id: &str)
    -> Result<Option<IndexedSignature>, DeduplicationError>;
    async fn save_signature(&self, sig: &IndexedSignature) -> Result<(), DeduplicationError>;
    async fn list_signatures_by_type(
        &self,
        error_type: &str,
    ) -> Result<Vec<IndexedSignature>, DeduplicationError>;
    async fn delete_signature(&self, id: &str) -> Result<(), DeduplicationError>;
    async fn get_all_signatures(&self) -> Result<Vec<IndexedSignature>, DeduplicationError>;
    async fn merge_signatures(
        &self,
        keep_id: &str,
        remove_id: &str,
    ) -> Result<(), DeduplicationError>;
}

pub struct InMemorySignatureStorage {
    signatures: RwLock<HashMap<String, IndexedSignature>>,
}

impl InMemorySignatureStorage {
    pub fn new() -> Self {
        Self {
            signatures: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySignatureStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SignatureStorage for InMemorySignatureStorage {
    async fn get_signature(
        &self,
        id: &str,
    ) -> Result<Option<IndexedSignature>, DeduplicationError> {
        let sigs = self
            .signatures
            .read()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(sigs.get(id).cloned())
    }

    async fn save_signature(&self, sig: &IndexedSignature) -> Result<(), DeduplicationError> {
        let mut sigs = self
            .signatures
            .write()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;
        sigs.insert(sig.id.clone(), sig.clone());
        Ok(())
    }

    async fn list_signatures_by_type(
        &self,
        error_type: &str,
    ) -> Result<Vec<IndexedSignature>, DeduplicationError> {
        let sigs = self
            .signatures
            .read()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(sigs
            .values()
            .filter(|s| s.error_type == error_type)
            .cloned()
            .collect())
    }

    async fn delete_signature(&self, id: &str) -> Result<(), DeduplicationError> {
        let mut sigs = self
            .signatures
            .write()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;
        sigs.remove(id);
        Ok(())
    }

    async fn get_all_signatures(&self) -> Result<Vec<IndexedSignature>, DeduplicationError> {
        let sigs = self
            .signatures
            .read()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;
        Ok(sigs.values().cloned().collect())
    }

    async fn merge_signatures(
        &self,
        keep_id: &str,
        remove_id: &str,
    ) -> Result<(), DeduplicationError> {
        let mut sigs = self
            .signatures
            .write()
            .map_err(|e| DeduplicationError::Storage(format!("Lock poisoned: {e}")))?;

        if let Some(removed) = sigs.remove(remove_id) {
            if let Some(kept) = sigs.get_mut(keep_id) {
                kept.merge_count += removed.merge_count + 1;
            }
        }
        Ok(())
    }
}

pub struct ErrorSignatureIndex<S: SignatureStorage> {
    storage: Arc<S>,
    cfg: DeduplicationConfig,
    metrics: Arc<RwLock<DeduplicationMetrics>>,
}

impl<S: SignatureStorage> ErrorSignatureIndex<S> {
    pub fn new(storage: Arc<S>, cfg: DeduplicationConfig) -> Self {
        Self {
            storage,
            cfg,
            metrics: Arc::new(RwLock::new(DeduplicationMetrics::default())),
        }
    }

    pub fn metrics(&self) -> DeduplicationMetrics {
        self.metrics.read().map(|m| m.clone()).unwrap_or_default()
    }

    #[instrument(skip(self, signature), fields(error_type = %signature.error_type))]
    pub async fn find_duplicate(
        &self,
        signature: &ErrorSignature,
    ) -> Result<Option<IndexedSignature>, DeduplicationError> {
        let candidates = self
            .storage
            .list_signatures_by_type(&signature.error_type)
            .await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        let candidate = IndexedSignature::from_signature("temp", signature);

        for existing in &candidates {
            let score = self.compute_similarity(&candidate, existing);
            if score >= self.cfg.similarity_threshold {
                if let Ok(mut m) = self.metrics.write() {
                    m.duplicates_detected += 1;
                }
                return Ok(Some(existing.clone()));
            }
        }

        Ok(None)
    }

    #[instrument(skip(self, id, signature))]
    pub async fn insert_or_deduplicate(
        &self,
        id: impl Into<String>,
        signature: &ErrorSignature,
    ) -> Result<DeduplicationResult, DeduplicationError> {
        let id = id.into();

        if let Some(existing) = self.find_duplicate(signature).await? {
            info!(
                existing_id = %existing.id,
                new_id = %id,
                "Duplicate signature detected, merging"
            );
            return Ok(DeduplicationResult::Duplicate {
                existing_id: existing.id,
                new_id: id,
            });
        }

        let indexed = IndexedSignature::from_signature(&id, signature);
        self.storage.save_signature(&indexed).await?;

        if let Ok(mut m) = self.metrics.write() {
            m.unique_signatures += 1;
        }

        Ok(DeduplicationResult::Unique { id })
    }

    fn compute_similarity(&self, a: &IndexedSignature, b: &IndexedSignature) -> f32 {
        let mut total_weight = 0.0;
        let mut weighted_score = 0.0;

        if let (Some(emb_a), Some(emb_b)) = (&a.embedding, &b.embedding) {
            let emb_sim = cosine_similarity(emb_a, emb_b);
            weighted_score += emb_sim * self.cfg.embedding_weight;
            total_weight += self.cfg.embedding_weight;
        }

        let msg_sim = jaccard_similarity(
            &tokenize(&a.normalized_message),
            &tokenize(&b.normalized_message),
        );
        weighted_score += msg_sim * self.cfg.message_weight;
        total_weight += self.cfg.message_weight;

        let ctx_sim = if a.context_hash == b.context_hash {
            1.0
        } else {
            0.0
        };
        weighted_score += ctx_sim * self.cfg.context_weight;
        total_weight += self.cfg.context_weight;

        if total_weight > 0.0 {
            weighted_score / total_weight
        } else {
            0.0
        }
    }

    #[instrument(skip(self))]
    pub async fn run_background_scan(&self) -> Result<ScanResult, DeduplicationError> {
        let start = std::time::Instant::now();
        let _span = info_span!("dedup_background_scan").entered();

        let all_sigs = self.storage.get_all_signatures().await?;
        let mut duplicates_found = 0;
        let mut merges_performed = 0;

        let mut by_type: HashMap<String, Vec<IndexedSignature>> = HashMap::new();
        for sig in all_sigs.into_iter().take(self.cfg.max_signatures_per_scan) {
            by_type.entry(sig.error_type.clone()).or_default().push(sig);
        }

        for (_error_type, sigs) in by_type {
            if sigs.len() < 2 {
                continue;
            }

            let mut to_merge: Vec<(String, String)> = Vec::new();

            for i in 0..sigs.len() {
                for j in (i + 1)..sigs.len() {
                    let score = self.compute_similarity(&sigs[i], &sigs[j]);
                    if score >= self.cfg.similarity_threshold {
                        duplicates_found += 1;
                        let keep = if sigs[i].created_at <= sigs[j].created_at {
                            &sigs[i]
                        } else {
                            &sigs[j]
                        };
                        let remove = if sigs[i].created_at <= sigs[j].created_at {
                            &sigs[j]
                        } else {
                            &sigs[i]
                        };
                        to_merge.push((keep.id.clone(), remove.id.clone()));
                    }
                }
            }

            for (keep_id, remove_id) in to_merge {
                if self
                    .storage
                    .merge_signatures(&keep_id, &remove_id)
                    .await
                    .is_ok()
                {
                    merges_performed += 1;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        if let Ok(mut m) = self.metrics.write() {
            m.duplicates_merged += merges_performed;
            m.last_scan_at = Some(current_timestamp());
            m.last_scan_duration_ms = Some(duration_ms);
            m.scans_completed += 1;
        }

        info!(
            duplicates_found = duplicates_found,
            merges_performed = merges_performed,
            duration_ms = duration_ms,
            "Background deduplication scan completed"
        );

        Ok(ScanResult {
            duplicates_found,
            merges_performed,
            duration_ms,
        })
    }
}

#[derive(Debug, Clone)]
pub enum DeduplicationResult {
    Unique { id: String },
    Duplicate { existing_id: String, new_id: String },
}

impl DeduplicationResult {
    pub fn is_duplicate(&self) -> bool {
        matches!(self, DeduplicationResult::Duplicate { .. })
    }

    pub fn effective_id(&self) -> &str {
        match self {
            DeduplicationResult::Unique { id } => id,
            DeduplicationResult::Duplicate { existing_id, .. } => existing_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub duplicates_found: u64,
    pub merges_performed: u64,
    pub duration_ms: u64,
}

pub struct ResolutionMerger;

impl ResolutionMerger {
    pub fn merge_resolutions(primary: &mut Resolution, secondary: &Resolution) {
        let combined_applications =
            primary.application_count as f64 + secondary.application_count as f64;

        if combined_applications > 0.0 {
            let primary_weight = primary.application_count as f64 * primary.success_rate as f64;
            let secondary_weight =
                secondary.application_count as f64 * secondary.success_rate as f64;
            primary.success_rate =
                ((primary_weight + secondary_weight) / combined_applications) as f32;
        }

        primary.application_count += secondary.application_count;

        if secondary.last_success_at > primary.last_success_at {
            primary.last_success_at = secondary.last_success_at;
        }

        for change in &secondary.changes {
            if !primary
                .changes
                .iter()
                .any(|c| c.file_path == change.file_path)
            {
                primary.changes.push(change.clone());
            }
        }
    }

    pub fn merge_hindsight_notes(primary: &mut HindsightNote, secondary: &HindsightNote) {
        for resolution in &secondary.resolutions {
            if !primary.resolutions.iter().any(|r| r.id == resolution.id) {
                primary.resolutions.push(resolution.clone());
            }
        }

        for tag in &secondary.tags {
            if !primary.tags.contains(tag) {
                primary.tags.push(tag.clone());
            }
        }

        if secondary.updated_at > primary.updated_at {
            primary.updated_at = secondary.updated_at;
        }
    }
}

fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn compute_context_hash(patterns: &[String]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut sorted: Vec<_> = patterns.iter().collect();
    sorted.sort();

    let mut hasher = DefaultHasher::new();
    for p in sorted {
        p.hash(&mut hasher);
    }
    hasher.finish()
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .collect()
}

fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    use std::collections::HashSet;
    let a_set: HashSet<_> = a.iter().collect();
    let b_set: HashSet<_> = b.iter().collect();

    let intersection = a_set.intersection(&b_set).count() as f32;
    let union = a_set.union(&b_set).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signature(error_type: &str, message: &str) -> ErrorSignature {
        ErrorSignature {
            error_type: error_type.to_string(),
            message_pattern: message.to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: None,
        }
    }

    fn sample_signature_with_embedding(
        error_type: &str,
        message: &str,
        embedding: Vec<f32>,
    ) -> ErrorSignature {
        ErrorSignature {
            error_type: error_type.to_string(),
            message_pattern: message.to_string(),
            stack_patterns: vec![],
            context_patterns: vec![],
            embedding: Some(embedding),
        }
    }

    #[tokio::test]
    async fn test_insert_unique_signature() {
        let storage = Arc::new(InMemorySignatureStorage::new());
        let index = ErrorSignatureIndex::new(storage, DeduplicationConfig::default());

        let sig = sample_signature("TypeError", "Cannot read property");
        let result = index.insert_or_deduplicate("sig1", &sig).await.unwrap();

        assert!(!result.is_duplicate());
        assert_eq!(result.effective_id(), "sig1");

        let metrics = index.metrics();
        assert_eq!(metrics.unique_signatures, 1);
    }

    #[tokio::test]
    async fn test_detect_duplicate_signature() {
        let storage = Arc::new(InMemorySignatureStorage::new());
        let index = ErrorSignatureIndex::new(storage, DeduplicationConfig::default());

        let sig1 = sample_signature("TypeError", "Cannot read property");
        index.insert_or_deduplicate("sig1", &sig1).await.unwrap();

        let sig2 = sample_signature("TypeError", "Cannot read property");
        let result = index.insert_or_deduplicate("sig2", &sig2).await.unwrap();

        assert!(result.is_duplicate());
        assert_eq!(result.effective_id(), "sig1");

        let metrics = index.metrics();
        assert_eq!(metrics.duplicates_detected, 1);
    }

    #[tokio::test]
    async fn test_different_error_types_not_duplicate() {
        let storage = Arc::new(InMemorySignatureStorage::new());
        let index = ErrorSignatureIndex::new(storage, DeduplicationConfig::default());

        let sig1 = sample_signature("TypeError", "Cannot read property");
        index.insert_or_deduplicate("sig1", &sig1).await.unwrap();

        let sig2 = sample_signature("ReferenceError", "Cannot read property");
        let result = index.insert_or_deduplicate("sig2", &sig2).await.unwrap();

        assert!(!result.is_duplicate());
    }

    #[tokio::test]
    async fn test_embedding_similarity_detection() {
        let storage = Arc::new(InMemorySignatureStorage::new());
        let cfg = DeduplicationConfig {
            embedding_weight: 1.0,
            message_weight: 0.0,
            context_weight: 0.0,
            stack_weight: 0.0,
            similarity_threshold: 0.95,
            ..Default::default()
        };
        let index = ErrorSignatureIndex::new(storage, cfg);

        let emb1 = vec![1.0, 0.0, 0.0];
        let sig1 = sample_signature_with_embedding("TypeError", "msg1", emb1.clone());
        index.insert_or_deduplicate("sig1", &sig1).await.unwrap();

        let sig2 = sample_signature_with_embedding("TypeError", "different message", emb1);
        let result = index.insert_or_deduplicate("sig2", &sig2).await.unwrap();

        assert!(result.is_duplicate());
    }

    #[tokio::test]
    async fn test_background_scan_finds_duplicates() {
        let storage = Arc::new(InMemorySignatureStorage::new());
        let cfg = DeduplicationConfig {
            similarity_threshold: 0.9,
            ..Default::default()
        };
        let index = ErrorSignatureIndex::new(storage.clone(), cfg);

        let sig1 = IndexedSignature {
            id: "s1".to_string(),
            error_type: "TypeError".to_string(),
            normalized_message: "cannot read property foo".to_string(),
            context_hash: 0,
            embedding: None,
            created_at: 100,
            merge_count: 0,
        };
        let sig2 = IndexedSignature {
            id: "s2".to_string(),
            error_type: "TypeError".to_string(),
            normalized_message: "cannot read property foo".to_string(),
            context_hash: 0,
            embedding: None,
            created_at: 200,
            merge_count: 0,
        };

        storage.save_signature(&sig1).await.unwrap();
        storage.save_signature(&sig2).await.unwrap();

        let scan_result = index.run_background_scan().await.unwrap();

        assert!(scan_result.duplicates_found > 0);
    }

    #[test]
    fn test_resolution_merger() {
        let mut primary = Resolution {
            id: "r1".to_string(),
            error_signature_id: "e1".to_string(),
            description: "Primary fix".to_string(),
            changes: vec![],
            success_rate: 0.8,
            application_count: 10,
            last_success_at: 100,
        };

        let secondary = Resolution {
            id: "r2".to_string(),
            error_signature_id: "e1".to_string(),
            description: "Secondary fix".to_string(),
            changes: vec![],
            success_rate: 0.9,
            application_count: 5,
            last_success_at: 200,
        };

        ResolutionMerger::merge_resolutions(&mut primary, &secondary);

        assert_eq!(primary.application_count, 15);
        assert_eq!(primary.last_success_at, 200);
        let expected_rate = (0.8 * 10.0 + 0.9 * 5.0) / 15.0;
        assert!((primary.success_rate - expected_rate as f32).abs() < 0.01);
    }

    #[test]
    fn test_hindsight_note_merger() {
        let mut primary = HindsightNote {
            id: "n1".to_string(),
            error_signature: sample_signature("Err", "msg"),
            resolutions: vec![],
            content: "Primary note".to_string(),
            tags: vec!["tag1".to_string()],
            created_at: 100,
            updated_at: 100,
        };

        let secondary = HindsightNote {
            id: "n2".to_string(),
            error_signature: sample_signature("Err", "msg"),
            resolutions: vec![],
            content: "Secondary note".to_string(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            created_at: 50,
            updated_at: 200,
        };

        ResolutionMerger::merge_hindsight_notes(&mut primary, &secondary);

        assert_eq!(primary.tags.len(), 2);
        assert!(primary.tags.contains(&"tag2".to_string()));
        assert_eq!(primary.updated_at, 200);
    }

    #[test]
    fn test_context_hash_consistency() {
        let patterns1 = vec!["tool:test".to_string(), "file:main.rs".to_string()];
        let patterns2 = vec!["file:main.rs".to_string(), "tool:test".to_string()];

        let hash1 = compute_context_hash(&patterns1);
        let hash2 = compute_context_hash(&patterns2);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_jaccard_similarity() {
        let a = vec!["foo".to_string(), "bar".to_string()];
        let b = vec!["foo".to_string(), "bar".to_string()];
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec!["foo".to_string(), "baz".to_string()];
        let sim = jaccard_similarity(&a, &c);
        assert!(sim > 0.0 && sim < 1.0);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
    }

    #[test]
    fn test_deduplication_result() {
        let unique = DeduplicationResult::Unique {
            id: "sig1".to_string(),
        };
        assert!(!unique.is_duplicate());
        assert_eq!(unique.effective_id(), "sig1");

        let duplicate = DeduplicationResult::Duplicate {
            existing_id: "sig1".to_string(),
            new_id: "sig2".to_string(),
        };
        assert!(duplicate.is_duplicate());
        assert_eq!(duplicate.effective_id(), "sig1");
    }
}
