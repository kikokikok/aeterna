use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct RateLimiter {
    limits: Arc<RwLock<HashMap<String, RateLimitState>>>,
    max_requests: u32,
    window: Duration
}

struct RateLimitState {
    requests: Vec<Instant>
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            limits: Arc::new(RwLock::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_seconds)
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> bool {
        let mut limits = self.limits.write().await;
        let now = Instant::now();

        let state = limits
            .entry(key.to_string())
            .or_insert_with(|| RateLimitState {
                requests: Vec::new()
            });

        state
            .requests
            .retain(|&req| now.duration_since(req) < self.window);

        if state.requests.len() >= self.max_requests as usize {
            return false;
        }

        state.requests.push(now);
        true
    }

    pub async fn remaining(&self, key: &str) -> u32 {
        let limits = self.limits.read().await;

        if let Some(state) = limits.get(key) {
            let now = Instant::now();
            let valid_requests = state
                .requests
                .iter()
                .filter(|&&req| now.duration_since(req) < self.window)
                .count();

            self.max_requests.saturating_sub(valid_requests as u32)
        } else {
            self.max_requests
        }
    }
}

pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiter>>,
    request: Request,
    next: Next
) -> Result<Response, StatusCode> {
    let key = request
        .headers()
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default")
        .to_string();

    if limiter.check_rate_limit(&key).await {
        let mut response = next.run(request).await;
        let remaining = limiter.remaining(&key).await;
        response.headers_mut().insert(
            "x-ratelimit-remaining",
            remaining.to_string().parse().unwrap()
        );
        Ok(response)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}
