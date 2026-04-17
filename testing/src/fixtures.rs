use std::sync::atomic::{AtomicU32, Ordering};
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::{ContainerAsync, GenericImage, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;
use tokio::sync::OnceCell;

use sqlx::postgres::PgPoolOptions;
use storage::migrations as db_migrations;
use storage::postgres::PostgresBackend;

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn unique_id(prefix: &str) -> String {
    let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}-{}", prefix, id)
}

pub fn unique_tenant_id() -> String {
    unique_id("test-tenant")
}

pub struct PostgresFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Postgres>,
    url: String,
}

impl PostgresFixture {
    pub fn url(&self) -> &str {
        &self.url
    }
}

static POSTGRES: OnceCell<Option<PostgresFixture>> = OnceCell::const_new();

pub async fn postgres() -> Option<&'static PostgresFixture> {
    POSTGRES
        .get_or_init(|| async {
            // We use the pgvector-enabled Postgres image (not the stock
            // `postgres:*-alpine` that testcontainers_modules::postgres
            // defaults to) because the aeterna schema declares `VECTOR(N)`
            // columns — e.g. `memory_entries.embedding VECTOR(1536)` — and
            // the `vector` type only exists once the pgvector extension is
            // installed. On stock Postgres those `CREATE TABLE` statements
            // either fail outright or are silently swallowed by inline
            // `.ok()` in `PostgresBackend::initialize_schema`, leaving
            // downstream tests with partial schemas.
            //
            // In production the same extension is provided by the CNPG
            // operator managing the Helm-deployed Postgres cluster.
            let container_result = Postgres::default()
                .with_db_name("testdb")
                .with_user("testuser")
                .with_password("testpass")
                .with_name("pgvector/pgvector")
                .with_tag("pg16")
                .start()
                .await;

            let container = match container_result {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Failed to start PostgreSQL container: {:?}", e);
                    return None;
                }
            };

            let port = container.get_host_port_ipv4(5432).await.ok()?;
            let url = format!("postgres://testuser:testpass@localhost:{}/testdb", port);
            tracing::info!("PostgreSQL fixture started on port {}", port);

            // Bring the database up to the same schema version that runs
            // in production. The order mirrors the Helm chart deployment
            // sequence exactly:
            //
            //   1. Pods start → `bootstrap()` calls
            //      `PostgresBackend::initialize_schema()` which creates
            //      the "core" inline tables, including several tables
            //      defined only inline (notably `organizational_units`,
            //      which migration 012 FK-references).
            //
            //   2. Helm `post-install` Job runs `aeterna admin migrate up`
            //      → applies migrations 003..=21, creating the rest of
            //      the schema (`users`, `organizations`, `teams`,
            //      `agents`, `memberships`, codesearch tables, …) and
            //      running `ALTER TABLE ADD COLUMN IF NOT EXISTS` to
            //      extend tables that exist in both sources.
            //
            // If we skipped step 1 here, migration 012 would fail with
            // `relation "organizational_units" does not exist`. If we
            // skipped step 2, callers would hit `relation "users" does
            // not exist` (and similar) as soon as they query a
            // migration-only table — which is exactly the regression PR
            // #35 surfaced for `server::auth_middleware` and
            // `server::knowledge_api` tests.
            //
            // `PostgresBackend::initialize_schema` is idempotent
            // (`CREATE TABLE IF NOT EXISTS` throughout), so tests
            // calling it again on their own backend instances remain
            // safe.
            let pool = match PgPoolOptions::new()
                .max_connections(2)
                .connect(&url)
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("Failed to open setup pool on fixture DB: {e:?}");
                    return None;
                }
            };
            let backend = match PostgresBackend::new(&url).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!("Failed to construct setup PostgresBackend: {e:?}");
                    pool.close().await;
                    return None;
                }
            };
            if let Err(e) = backend.initialize_schema().await {
                tracing::warn!("initialize_schema on fixture failed: {e:?}");
                pool.close().await;
                return None;
            }
            if let Err(e) = db_migrations::apply_all(&pool).await {
                tracing::warn!("apply_all migrations on fixture failed: {e:?}");
                pool.close().await;
                return None;
            }
            pool.close().await;

            Some(PostgresFixture { container, url })
        })
        .await
        .as_ref()
}

pub struct RedisFixture {
    #[allow(dead_code)]
    container: ContainerAsync<Redis>,
    url: String,
}

impl RedisFixture {
    pub fn url(&self) -> &str {
        &self.url
    }
}

static REDIS: OnceCell<Option<RedisFixture>> = OnceCell::const_new();

pub async fn redis() -> Option<&'static RedisFixture> {
    REDIS
        .get_or_init(|| async {
            match Redis::default().start().await {
                Ok(container) => {
                    let port = match container.get_host_port_ipv4(6379).await {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!("Failed to get Redis port: {:?}", e);
                            return None;
                        }
                    };
                    let url = format!("redis://localhost:{}", port);

                    if let Err(e) = verify_redis_connection(&url).await {
                        tracing::warn!("Redis connection verification failed: {:?}", e);
                        return None;
                    }

                    tracing::info!("Redis fixture started on port {}", port);
                    Some(RedisFixture { container, url })
                }
                Err(e) => {
                    tracing::warn!("Failed to start Redis container: {:?}", e);
                    None
                }
            }
        })
        .await
        .as_ref()
}

async fn verify_redis_connection(url: &str) -> anyhow::Result<()> {
    let client = redis::Client::open(url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let _: String = redis::cmd("PING").query_async(&mut conn).await?;
    Ok(())
}

pub struct QdrantFixture {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    grpc_url: String,
    http_url: String,
}

impl QdrantFixture {
    pub fn grpc_url(&self) -> &str {
        &self.grpc_url
    }

    pub fn http_url(&self) -> &str {
        &self.http_url
    }

    #[deprecated(note = "Use grpc_url() for gRPC (6334) or http_url() for REST (6333)")]
    pub fn url(&self) -> &str {
        &self.grpc_url
    }
}

static QDRANT: OnceCell<Option<QdrantFixture>> = OnceCell::const_new();

pub async fn qdrant() -> Option<&'static QdrantFixture> {
    QDRANT
        .get_or_init(|| async {
            let container_result = GenericImage::new("qdrant/qdrant", "latest")
                .with_exposed_port(ContainerPort::Tcp(6333))
                .with_exposed_port(ContainerPort::Tcp(6334))
                // Wait for gRPC to be ready (appears after HTTP)
                .with_wait_for(WaitFor::message_on_stdout("Qdrant gRPC listening on 6334"))
                .with_startup_timeout(std::time::Duration::from_secs(60))
                .start()
                .await;

            match container_result {
                Ok(container) => {
                    let http_port = container.get_host_port_ipv4(6333).await.ok()?;
                    let grpc_port = container.get_host_port_ipv4(6334).await.ok()?;
                    let http_url = format!("http://localhost:{}", http_port);
                    let grpc_url = format!("http://localhost:{}", grpc_port);

                    // Brief delay for gRPC to fully initialize
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    tracing::info!(
                        "Qdrant fixture started - HTTP: {}, gRPC: {}",
                        http_port,
                        grpc_port
                    );

                    if let Err(e) = verify_qdrant_connection(&http_url).await {
                        tracing::warn!("Qdrant connection verification failed: {:?}", e);
                        return None;
                    }

                    Some(QdrantFixture {
                        container,
                        grpc_url,
                        http_url,
                    })
                }
                Err(e) => {
                    tracing::warn!("Failed to start Qdrant container: {:?}", e);
                    None
                }
            }
        })
        .await
        .as_ref()
}

async fn verify_qdrant_connection(http_url: &str) -> anyhow::Result<()> {
    let health_url = format!("{}/healthz", http_url);
    for attempt in 0..10 {
        match reqwest::get(&health_url).await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                if attempt < 9 {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    }
    anyhow::bail!("Qdrant health check failed after 10 attempts")
}

pub const MINIO_ACCESS_KEY: &str = "minioadmin";
pub const MINIO_SECRET_KEY: &str = "minioadmin";
pub const MINIO_DEFAULT_BUCKET: &str = "aeterna-test";

pub struct MinioFixture {
    #[allow(dead_code)]
    container: ContainerAsync<GenericImage>,
    endpoint: String,
}

impl MinioFixture {
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn access_key(&self) -> &str {
        MINIO_ACCESS_KEY
    }

    pub fn secret_key(&self) -> &str {
        MINIO_SECRET_KEY
    }
}

static MINIO: OnceCell<Option<MinioFixture>> = OnceCell::const_new();

pub async fn minio() -> Option<&'static MinioFixture> {
    MINIO
        .get_or_init(|| async {
            let image = GenericImage::new("minio/minio", "latest")
                .with_exposed_port(ContainerPort::Tcp(9000))
                .with_wait_for(WaitFor::message_on_stdout("API:"));

            let container_result = image
                .with_env_var("MINIO_ROOT_USER", MINIO_ACCESS_KEY)
                .with_env_var("MINIO_ROOT_PASSWORD", MINIO_SECRET_KEY)
                .with_cmd(vec!["server", "/data"])
                .start()
                .await;

            match container_result {
                Ok(container) => {
                    let port = container.get_host_port_ipv4(9000).await.ok()?;
                    let endpoint = format!("http://localhost:{}", port);
                    tracing::info!("MinIO fixture started on port {}", port);
                    Some(MinioFixture {
                        container,
                        endpoint,
                    })
                }
                Err(e) => {
                    tracing::warn!("Failed to start MinIO container: {:?}", e);
                    None
                }
            }
        })
        .await
        .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_unique_id_generation() {
        let id1 = unique_id("test");
        let id2 = unique_id("test");
        assert_ne!(id1, id2);
        assert!(id1.starts_with("test-"));
        assert!(id2.starts_with("test-"));
    }
}
