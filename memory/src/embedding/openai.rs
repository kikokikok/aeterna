use async_trait::async_trait;
use mk_core::traits::EmbeddingService;
use std::sync::Arc;
use storage::redis::RedisStorage;
use tokio::sync::RwLock;

pub struct OpenAIEmbeddingService {
    client: async_openai::Client<async_openai::config::OpenAIConfig>,
    model: String,
    dimension: usize,
    cache: Arc<RwLock<lru::LruCache<String, Vec<f32>>>>,
    redis: Option<Arc<RwLock<RedisStorage>>>
}

impl OpenAIEmbeddingService {
    pub fn new(api_key: String, model: &str) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        let client = async_openai::Client::with_config(config);

        let dimension = match model {
            "text-embedding-ada-002" => 1536,
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            _ => 1536
        };

        Self {
            client,
            model: model.to_string(),
            dimension,
            cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(1000).unwrap()
            ))),
            redis: None
        }
    }

    pub fn with_redis(mut self, redis: Arc<RwLock<RedisStorage>>) -> Self {
        self.redis = Some(redis);
        self
    }

    pub fn with_cache_size(api_key: String, model: &str, cache_size: usize) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        let client = async_openai::Client::with_config(config);

        let dimension = match model {
            "text-embedding-ada-002" => 1536,
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            _ => 1536
        };

        Self {
            client,
            model: model.to_string(),
            dimension,
            cache: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(cache_size).unwrap()
            ))),
            redis: None
        }
    }

    pub fn with_default_model(api_key: String) -> Self {
        Self::new(api_key, "text-embedding-ada-002")
    }
}

#[async_trait]
impl EmbeddingService for OpenAIEmbeddingService {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    async fn embed(&self, text: &str) -> Result<Vec<f32>, Self::Error> {
        {
            let mut cache = self.cache.write().await;
            if let Some(cached) = cache.get(text) {
                return Ok(cached.clone());
            }
        }

        if let Some(redis) = &self.redis {
            let redis = redis.write().await;
            let key = format!("emb:{}:{}", self.model, text);
            if let Ok(Some(cached_json)) = redis.get(&key).await {
                if let Ok(embedding) = serde_json::from_str::<Vec<f32>>(&cached_json) {
                    let mut cache = self.cache.write().await;
                    cache.put(text.to_string(), embedding.clone());
                    return Ok(embedding);
                }
            }
        }

        let request = async_openai::types::CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(text)
            .build()?;

        let response = self.client.embeddings().create(request).await?;

        let embedding = response
            .data
            .first()
            .ok_or("No embedding returned")?
            .embedding
            .clone();

        {
            let mut cache = self.cache.write().await;
            cache.put(text.to_string(), embedding.clone());
        }

        if let Some(redis) = &self.redis {
            let redis = redis.write().await;
            let key = format!("emb:{}:{}", self.model, text);
            if let Ok(json) = serde_json::to_string(&embedding) {
                let _ = redis.set(&key, &json, Some(86400)).await;
            }
        }

        Ok(embedding)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, Self::Error> {
        let mut results = Vec::with_capacity(texts.len());
        let mut uncached_texts = Vec::new();
        let mut uncached_indices = Vec::new();

        let mut cache = self.cache.write().await;

        for (i, text) in texts.iter().enumerate() {
            if let Some(cached) = cache.get(text) {
                results.push(cached.clone());
            } else {
                results.push(Vec::new());
                uncached_texts.push(text.clone());
                uncached_indices.push(i);
            }
        }

        if !uncached_texts.is_empty() {
            let request = async_openai::types::CreateEmbeddingRequestArgs::default()
                .model(&self.model)
                .input(uncached_texts.clone())
                .build()?;

            let response = self.client.embeddings().create(request).await?;

            for (i, embedding_data) in response.data.into_iter().enumerate() {
                let idx = uncached_indices[i];
                let text: &String = &uncached_texts[i];
                let embedding = embedding_data.embedding;

                cache.put(text.clone(), embedding.clone());
                results[idx] = embedding;
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires OpenAI API key"]
    async fn test_openai_embedding_service() {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return;
        }

        let service = OpenAIEmbeddingService::with_default_model(api_key);

        let embedding = service.embed("Test text").await.unwrap();
        assert_eq!(embedding.len(), 1536);
        assert!(service.dimension() == 1536);

        let texts = vec!["First text".to_string(), "Second text".to_string()];
        let embeddings = service.embed_batch(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 2);
        for embedding in embeddings {
            assert_eq!(embedding.len(), 1536);
        }
    }

    #[tokio::test]
    async fn test_lru_cache_hit() {
        let service = OpenAIEmbeddingService::with_cache_size(
            "sk-fake-key".to_string(),
            "text-embedding-ada-002",
            10
        );

        let mut cache = service.cache.write().await;

        let test_vector = vec![0.1; 1536];
        cache.put("test_text".to_string(), test_vector.clone());

        let cached = cache.get("test_text");
        assert!(cached.is_some(), "Cached value should be found");
        assert_eq!(*cached.unwrap(), *test_vector, "Cached vector should match");
    }

    #[tokio::test]
    async fn test_lru_cache_miss() {
        let service = OpenAIEmbeddingService::with_cache_size(
            "sk-fake-key".to_string(),
            "text-embedding-ada-002",
            10
        );

        let mut cache = service.cache.write().await;

        let cached = cache.get("nonexistent_text");
        assert!(cached.is_none(), "Should return None for nonexistent key");
    }

    #[test]
    fn test_dimension_configuration() {
        let service = OpenAIEmbeddingService::new("sk-test".to_string(), "text-embedding-ada-002");
        assert_eq!(
            service.dimension(),
            1536,
            "ada-002 should have 1536 dimensions"
        );

        let service = OpenAIEmbeddingService::new("sk-test".to_string(), "text-embedding-3-small");
        assert_eq!(
            service.dimension(),
            1536,
            "3-small should have 1536 dimensions"
        );

        let service = OpenAIEmbeddingService::new("sk-test".to_string(), "text-embedding-3-large");
        assert_eq!(
            service.dimension(),
            3072,
            "3-large should have 3072 dimensions"
        );
    }

    #[tokio::test]
    async fn test_custom_cache_size() {
        let service = OpenAIEmbeddingService::with_cache_size(
            "sk-test".to_string(),
            "text-embedding-ada-002",
            500
        );

        let cache = service.cache.read().await;
        assert_eq!(cache.cap().get(), 500, "Cache capacity should be 500");
    }
}
