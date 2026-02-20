use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioMemoryError {
    #[error("audio support not yet implemented: {reason}")]
    NotImplemented { reason: String },
}

/// Audio format (future support).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioFormat {
    Mp3,
    Wav,
    Flac,
    Ogg,
}

/// Placeholder for future audio memory support.
///
/// Audio memory will support:
/// - Speech-to-text transcription for embedding
/// - Audio fingerprinting for similarity search
/// - Speaker diarization for multi-speaker memory
///
/// Timeline: Phase 4 (Months 7-8)
pub struct AudioMemory;

impl AudioMemory {
    pub fn new() -> Self {
        Self
    }

    pub async fn embed_audio(
        &self,
        _bytes: &[u8],
        _format: AudioFormat,
    ) -> Result<Vec<f32>, AudioMemoryError> {
        Err(AudioMemoryError::NotImplemented {
            reason: "audio embedding planned for Phase 4".to_string(),
        })
    }
}

impl Default for AudioMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audio_not_implemented() {
        let memory = AudioMemory::new();
        let result = memory.embed_audio(&[], AudioFormat::Mp3).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not yet implemented"));
    }
}
