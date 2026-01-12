use async_trait::async_trait;
use mk_core::traits::EventPublisher;
use mk_core::types::GovernanceEvent;
use redis::AsyncCommands;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    Internal(String)
}

pub struct RedisPublisher {
    client: Arc<redis::Client>,
    stream_name: String
}

impl RedisPublisher {
    pub fn new(connection_url: &str, stream_name: &str) -> Result<Self, EventError> {
        let client = redis::Client::open(connection_url)?;
        Ok(Self {
            client: Arc::new(client),
            stream_name: stream_name.to_string()
        })
    }
}

#[async_trait]
impl EventPublisher for RedisPublisher {
    type Error = EventError;

    async fn publish(&self, event: GovernanceEvent) -> Result<(), Self::Error> {
        let mut conn = self.client.get_connection_manager().await?;
        let event_json = serde_json::to_string(&event)?;

        let _: String = conn
            .xadd(&self.stream_name, "*", &[("event", event_json)])
            .await?;

        Ok(())
    }

    async fn subscribe(
        &self,
        _channels: &[&str]
    ) -> Result<tokio::sync::mpsc::Receiver<GovernanceEvent>, Self::Error> {
        let client = self.client.clone();
        let stream_name = self.stream_name.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            if let Ok(mut conn) = client.get_connection_manager().await {
                let mut last_id = "$".to_string();

                loop {
                    let opts = redis::streams::StreamReadOptions::default()
                        .block(0)
                        .count(10);

                    let result: Result<redis::streams::StreamReadReply, redis::RedisError> = conn
                        .xread_options(&[&stream_name], &[&last_id], &opts)
                        .await;

                    match result {
                        Ok(reply) => {
                            for stream in reply.keys {
                                for record in stream.ids {
                                    if let Some(event_json) = record.map.get("event") {
                                        let event_bytes: Vec<u8> =
                                            redis::from_redis_value(event_json.clone())
                                                .unwrap_or_default();
                                        if let Ok(event) =
                                            serde_json::from_slice::<GovernanceEvent>(&event_bytes)
                                        {
                                            if tx.send(event).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                    last_id = record.id;
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Redis subscription error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

pub struct MultiPublisher<E: std::error::Error + Send + Sync + 'static> {
    publishers: Vec<Box<dyn EventPublisher<Error = E> + Send + Sync>>
}

impl<E: std::error::Error + Send + Sync + 'static> MultiPublisher<E> {
    pub fn new(publishers: Vec<Box<dyn EventPublisher<Error = E> + Send + Sync>>) -> Self {
        Self { publishers }
    }
}

#[async_trait]
impl<E: std::error::Error + Send + Sync + 'static> EventPublisher for MultiPublisher<E> {
    type Error = E;

    async fn publish(&self, event: GovernanceEvent) -> Result<(), Self::Error> {
        for publisher in &self.publishers {
            publisher.publish(event.clone()).await?;
        }
        Ok(())
    }

    async fn subscribe(
        &self,
        _channels: &[&str]
    ) -> Result<tokio::sync::mpsc::Receiver<GovernanceEvent>, Self::Error> {
        panic!("Subscribe not implemented for multi-publisher")
    }
}
