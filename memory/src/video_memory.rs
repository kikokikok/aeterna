use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VideoMemoryError {
    #[error("video support not yet implemented: {reason}")]
    NotImplemented { reason: String },
}

/// Video format (future support).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormat {
    Mp4,
    Webm,
    Avi,
    Mov,
}

/// Placeholder for future video memory support.
///
/// Video memory will support:
/// - Frame-level CLIP embeddings for visual search
/// - Audio track extraction and embedding
/// - Scene boundary detection
/// - Temporal search across video segments
///
/// Timeline: Phase 4 (Months 7-8)
pub struct VideoMemory;

impl VideoMemory {
    pub fn new() -> Self {
        Self
    }

    pub async fn embed_video(
        &self,
        _bytes: &[u8],
        _format: VideoFormat,
    ) -> Result<Vec<Vec<f32>>, VideoMemoryError> {
        Err(VideoMemoryError::NotImplemented {
            reason: "video embedding planned for Phase 4".to_string(),
        })
    }
}

impl Default for VideoMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_video_not_implemented() {
        let memory = VideoMemory::new();
        let result = memory.embed_video(&[], VideoFormat::Mp4).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not yet implemented"));
    }
}
