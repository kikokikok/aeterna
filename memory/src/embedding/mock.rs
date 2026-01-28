use async_trait::async_trait;
use mk_core::traits::EmbeddingService;

pub struct MockEmbeddingService {
    dimension: usize
}

impl MockEmbeddingService {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    fn generate_mock_embedding(text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0; 384];
        let text_lower = text.to_lowercase();

        if text_lower.contains("rust") {
            embedding[0] = 0.8;
            embedding[1] = 0.6;
        }
        if text_lower.contains("typescript") || text_lower.contains("javascript") {
            embedding[2] = 0.7;
            embedding[3] = 0.5;
        }
        if text_lower.contains("python") {
            embedding[4] = 0.9;
            embedding[5] = 0.4;
        }
        if text_lower.contains("database") {
            embedding[6] = 0.6;
            embedding[7] = 0.7;
        }
        if text_lower.contains("api") {
            embedding[8] = 0.5;
            embedding[9] = 0.8;
        }

        let length_factor = (text.len() as f32).min(1000.0) / 1000.0;
        embedding[10] = length_factor;

        embedding
    }
}

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error> {
        Ok(Self::generate_mock_embedding(text))
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedding_service() {
        let service = MockEmbeddingService::new(384);

        let embedding1 = service.embed("Rust programming language").await.unwrap();
        assert_eq!(embedding1.len(), 384);
        assert!(embedding1[0] > 0.0);
        assert!(embedding1[1] > 0.0);

        let embedding2 = service.embed("Python data science").await.unwrap();
        assert_eq!(embedding2.len(), 384);
        assert!(embedding2[4] > 0.0);
        assert!(embedding2[5] > 0.0);

        assert_ne!(embedding1, embedding2);
    }

    #[tokio::test]
    async fn test_mock_embedding_service_batch() {
        let service = MockEmbeddingService::new(384);

        let texts = vec![
            "Rust programming".to_string(),
            "Python scripting".to_string(),
            "Database management".to_string(),
        ];

        let embeddings = service.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 3);
        for embedding in embeddings {
            assert_eq!(embedding.len(), 384);
        }
    }

    #[tokio::test]
    async fn test_mock_embedding_dimension() {
        let service = MockEmbeddingService::new(512);
        assert_eq!(service.dimension(), 512);

        let service2 = MockEmbeddingService::new(1024);
        assert_eq!(service2.dimension(), 1024);
    }

    #[tokio::test]
    async fn test_mock_embedding_typescript_javascript() {
        let service = MockEmbeddingService::new(384);

        let ts_embedding = service.embed("TypeScript framework").await.unwrap();
        assert!(ts_embedding[2] > 0.0);
        assert!(ts_embedding[3] > 0.0);

        let js_embedding = service.embed("JavaScript library").await.unwrap();
        assert!(js_embedding[2] > 0.0);
        assert!(js_embedding[3] > 0.0);
    }

    #[tokio::test]
    async fn test_mock_embedding_database() {
        let service = MockEmbeddingService::new(384);
        let embedding = service.embed("Database management system").await.unwrap();
        assert!(embedding[6] > 0.0);
        assert!(embedding[7] > 0.0);
    }

    #[tokio::test]
    async fn test_mock_embedding_api() {
        let service = MockEmbeddingService::new(384);
        let embedding = service.embed("API endpoint design").await.unwrap();
        assert!(embedding[8] > 0.0);
        assert!(embedding[9] > 0.0);
    }

    #[tokio::test]
    async fn test_mock_embedding_length_factor() {
        let service = MockEmbeddingService::new(384);

        let short_text = service.embed("short").await.unwrap();
        let long_text = service.embed(&"a".repeat(500)).await.unwrap();

        assert!(long_text[10] > short_text[10]);
    }
}
