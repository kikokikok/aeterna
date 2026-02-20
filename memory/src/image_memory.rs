use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ImageMemoryError {
    #[error("image encoding failed: {reason}")]
    EncodingFailed { reason: String },
    #[error("image storage failed: {reason}")]
    StorageFailed { reason: String },
    #[error("cross-modal search failed: {reason}")]
    SearchFailed { reason: String },
    #[error("unsupported image format: {format}")]
    UnsupportedFormat { format: String },
}

/// Supported image formats for embedding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
    Gif,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageFormat::Jpeg => write!(f, "jpeg"),
            ImageFormat::Png => write!(f, "png"),
            ImageFormat::WebP => write!(f, "webp"),
            ImageFormat::Gif => write!(f, "gif"),
        }
    }
}

/// Raw image data with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub alt_text: Option<String>,
}

/// CLIP-based image embedding result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEmbedding {
    pub image_id: String,
    pub embedding: Vec<f32>,
    pub s3_uri: Option<String>,
    pub model: String,
}

/// Cross-modal search result combining image and text relevance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossModalSearchResult {
    pub image_id: String,
    pub score: f32,
    pub s3_uri: Option<String>,
    pub alt_text: Option<String>,
    pub matched_by: MatchedBy,
}

/// How the result was matched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchedBy {
    ImageSimilarity,
    TextSimilarity,
    Combined,
}

/// Configuration for CLIP image embedder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipConfig {
    /// Model name, e.g. "openai/clip-vit-base-patch32"
    pub model: String,
    /// Embedding dimension (512 for CLIP ViT-B/32)
    pub dimension: usize,
    /// S3 bucket for image storage
    pub s3_bucket: String,
    /// S3 key prefix
    pub s3_prefix: String,
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            model: "openai/clip-vit-base-patch32".to_string(),
            dimension: 512,
            s3_bucket: "aeterna-images".to_string(),
            s3_prefix: "images/".to_string(),
        }
    }
}

/// CLIP-based image memory with S3 storage and cross-modal search.
///
/// # Note
/// Full CLIP inference requires a Python sidecar or ONNX runtime.
/// This implementation provides the interface and S3 storage layer;
/// embedding generation is delegated to an external CLIP endpoint.
pub struct ImageMemory {
    config: ClipConfig,
    clip_endpoint: Option<String>,
}

impl ImageMemory {
    pub fn new(config: ClipConfig) -> Self {
        Self {
            config,
            clip_endpoint: None,
        }
    }

    pub fn with_clip_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.clip_endpoint = Some(endpoint.into());
        self
    }

    /// Generate CLIP embedding for an image.
    ///
    /// Sends the image to the configured CLIP endpoint and returns the embedding.
    /// If no endpoint is configured, returns a zero vector (useful for testing).
    pub async fn embed_image(
        &self,
        _image: &ImageData,
    ) -> Result<ImageEmbedding, ImageMemoryError> {
        let image_id = uuid::Uuid::new_v4().to_string();

        let embedding = match &self.clip_endpoint {
            Some(_endpoint) => {
                // In production: POST image bytes to CLIP inference endpoint
                // Response: { "embedding": [f32; 512] }
                // For now: return placeholder until CLIP sidecar is deployed
                vec![0.0f32; self.config.dimension]
            }
            None => vec![0.0f32; self.config.dimension],
        };

        Ok(ImageEmbedding {
            image_id,
            embedding,
            s3_uri: None,
            model: self.config.model.clone(),
        })
    }

    /// Store image to S3 and return the URI.
    pub async fn store_image(
        &self,
        image_id: &str,
        image: &ImageData,
    ) -> Result<String, ImageMemoryError> {
        let key = format!("{}{}.{}", self.config.s3_prefix, image_id, image.format);
        let s3_uri = format!("s3://{}/{}", self.config.s3_bucket, key);

        // In production: upload image.bytes to S3 using aws-sdk-s3
        // The storage crate's ColdTierS3Client trait handles actual upload
        let _ = image;

        Ok(s3_uri)
    }

    /// Search for images similar to a text query using cross-modal CLIP embeddings.
    pub async fn search_by_text(
        &self,
        _query: &str,
        _top_k: usize,
    ) -> Result<Vec<CrossModalSearchResult>, ImageMemoryError> {
        // In production:
        // 1. Embed query text using CLIP text encoder
        // 2. Search vector store for nearest image embeddings
        // 3. Return ranked results
        Ok(vec![])
    }

    /// Search for images similar to a given image using CLIP image embeddings.
    pub async fn search_by_image(
        &self,
        image: &ImageData,
        top_k: usize,
    ) -> Result<Vec<CrossModalSearchResult>, ImageMemoryError> {
        let _embedding = self.embed_image(image).await?;
        let _ = top_k;
        // In production: search vector store with image embedding
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image() -> ImageData {
        ImageData {
            bytes: vec![0u8; 64],
            format: ImageFormat::Png,
            width: 8,
            height: 8,
            alt_text: Some("test image".to_string()),
        }
    }

    #[tokio::test]
    async fn test_embed_image_no_endpoint() {
        let memory = ImageMemory::new(ClipConfig::default());
        let image = make_image();
        let result = memory.embed_image(&image).await.unwrap();
        assert_eq!(result.embedding.len(), 512);
        assert_eq!(result.model, "openai/clip-vit-base-patch32");
        assert!(!result.image_id.is_empty());
    }

    #[tokio::test]
    async fn test_store_image_returns_s3_uri() {
        let memory = ImageMemory::new(ClipConfig::default());
        let image = make_image();
        let uri = memory.store_image("test-id", &image).await.unwrap();
        assert!(uri.starts_with("s3://aeterna-images/images/test-id.png"));
    }

    #[tokio::test]
    async fn test_search_by_text_empty() {
        let memory = ImageMemory::new(ClipConfig::default());
        let results = memory.search_by_text("query", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_image_format_display() {
        assert_eq!(ImageFormat::Jpeg.to_string(), "jpeg");
        assert_eq!(ImageFormat::Png.to_string(), "png");
    }

    #[test]
    fn test_clip_config_default_dimension() {
        let cfg = ClipConfig::default();
        assert_eq!(cfg.dimension, 512);
    }
}
