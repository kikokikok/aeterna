use std::sync::Arc;
use tracing::{Level, info};

use agent_a2a::{Config, auth::AuthState, create_router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting A2A Agent Server");

    let config = Config::from_env().unwrap_or_default();
    info!("Configuration loaded");

    let auth_state = Arc::new(AuthState {
        enabled: config.auth.enabled,
        api_key: config.auth.api_key.clone(),
        jwt_secret: config.auth.jwt_secret.clone(),
        trusted_identity: config.auth.trusted_identity.clone(),
    });

    auth_state.validate()?;

    let app = create_router(Arc::new(config.clone()), auth_state);

    let addr = config.socket_addr()?;
    info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
