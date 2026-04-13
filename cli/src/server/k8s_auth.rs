//! Kubernetes TokenReview authentication.
//!
//! Validates a bearer token by calling the Kubernetes TokenReview API
//! (`POST /apis/authentication.k8s.io/v1/tokenreviews`).  On success returns
//! the authenticated username (e.g. `system:serviceaccount:aeterna:aeterna-sync`).

use std::path::Path;

use reqwest::{Certificate, Client};
use serde::{Deserialize, Serialize};
use tracing::debug;

use config::KubernetesAuthConfig;

const DEFAULT_API_SERVER_URL: &str = "https://kubernetes.default.svc";
const DEFAULT_SA_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";
const DEFAULT_CA_BUNDLE_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/ca.crt";

/// The result of a successful TokenReview validation.
#[derive(Debug, Clone)]
pub struct K8sIdentity {
    /// The authenticated username, e.g. `system:serviceaccount:aeterna:aeterna-sync`.
    pub username: String,
}

#[derive(Debug, Serialize)]
struct TokenReviewRequest {
    #[serde(rename = "apiVersion")]
    api_version: &'static str,
    kind: &'static str,
    spec: TokenReviewSpec,
}

#[derive(Debug, Serialize)]
struct TokenReviewSpec {
    token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    audiences: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct TokenReviewResponse {
    #[serde(default)]
    status: Option<TokenReviewStatus>,
}

#[derive(Debug, Deserialize)]
struct TokenReviewStatus {
    #[serde(default)]
    authenticated: bool,
    #[serde(default)]
    user: Option<TokenReviewUser>,
}

#[derive(Debug, Deserialize)]
struct TokenReviewUser {
    #[serde(default)]
    username: Option<String>,
}

/// Validates a Kubernetes SA bearer token via TokenReview API.
///
/// Returns `None` if the token is invalid, expired, or the API call fails.
pub async fn validate_k8s_bearer(token: &str, cfg: &KubernetesAuthConfig) -> Option<K8sIdentity> {
    let reviewer_token_path = cfg
        .sa_token_path
        .as_deref()
        .unwrap_or(DEFAULT_SA_TOKEN_PATH);
    let reviewer_token = tokio::fs::read_to_string(reviewer_token_path).await.ok()?;
    let reviewer_token = reviewer_token.trim();
    if reviewer_token.is_empty() {
        debug!(
            path = reviewer_token_path,
            "k8s auth reviewer token file was empty"
        );
        return None;
    }

    let client = build_client(cfg).await?;
    let api_server_url = cfg
        .api_server_url
        .as_deref()
        .unwrap_or(DEFAULT_API_SERVER_URL)
        .trim_end_matches('/');
    let url = format!("{api_server_url}/apis/authentication.k8s.io/v1/tokenreviews");
    let request = TokenReviewRequest {
        api_version: "authentication.k8s.io/v1",
        kind: "TokenReview",
        spec: TokenReviewSpec {
            token: token.to_string(),
            audiences: cfg
                .token_review_audience
                .as_ref()
                .map(|audience| vec![audience.clone()]),
        },
    };

    let response = match client
        .post(&url)
        .bearer_auth(reviewer_token)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            debug!(error = %error, "k8s TokenReview request failed");
            return None;
        }
    };

    if !response.status().is_success() {
        debug!(status = %response.status(), "k8s TokenReview returned non-success status");
        return None;
    }

    let body = match response.json::<TokenReviewResponse>().await {
        Ok(body) => body,
        Err(error) => {
            debug!(error = %error, "k8s TokenReview response parsing failed");
            return None;
        }
    };

    parse_token_review_response(body)
}

async fn build_client(cfg: &KubernetesAuthConfig) -> Option<Client> {
    let ca_bundle_path = cfg
        .ca_bundle_path
        .as_deref()
        .unwrap_or(DEFAULT_CA_BUNDLE_PATH);
    let ca_path = Path::new(ca_bundle_path);

    let mut builder = Client::builder();
    if ca_path.exists() {
        match tokio::fs::read(ca_path).await {
            Ok(ca_bytes) => match Certificate::from_pem(&ca_bytes)
                .or_else(|_| Certificate::from_der(&ca_bytes))
            {
                Ok(certificate) => {
                    builder = builder.add_root_certificate(certificate);
                }
                Err(error) => {
                    debug!(path = %ca_bundle_path, error = %error, "k8s CA bundle could not be parsed");
                    return None;
                }
            },
            Err(error) => {
                debug!(path = %ca_bundle_path, error = %error, "k8s CA bundle could not be read");
                return None;
            }
        }
    } else {
        debug!(path = %ca_bundle_path, "k8s CA bundle missing; allowing invalid certs for dev mode");
        builder = builder.danger_accept_invalid_certs(true);
    }

    match builder.build() {
        Ok(client) => Some(client),
        Err(error) => {
            debug!(error = %error, "k8s auth HTTP client construction failed");
            None
        }
    }
}

fn parse_token_review_response(body: TokenReviewResponse) -> Option<K8sIdentity> {
    let status = body.status?;
    if !status.authenticated {
        debug!("k8s TokenReview reported unauthenticated token");
        return None;
    }

    let Some(username) = status.user?.username else {
        debug!("k8s TokenReview authenticated user was missing username");
        return None;
    };
    if username.is_empty() {
        debug!("k8s TokenReview authenticated user had empty username");
        return None;
    }

    Some(K8sIdentity { username })
}

#[cfg(test)]
mod tests {
    use super::{K8sIdentity, TokenReviewResponse, parse_token_review_response};

    #[tokio::test]
    async fn parses_authenticated_tokenreview_response() {
        let response: TokenReviewResponse = serde_json::from_str(
            r#"{
                "status": {
                    "authenticated": true,
                    "user": {
                        "username": "system:serviceaccount:aeterna:aeterna-sync"
                    }
                }
            }"#,
        )
        .unwrap();

        let identity = parse_token_review_response(response);

        assert!(matches!(
            identity,
            Some(K8sIdentity {
                username
            }) if username == "system:serviceaccount:aeterna:aeterna-sync"
        ));
    }

    #[tokio::test]
    async fn rejects_unauthenticated_tokenreview_response() {
        let response: TokenReviewResponse = serde_json::from_str(
            r#"{
                "status": {
                    "authenticated": false,
                    "user": {
                        "username": "system:serviceaccount:aeterna:aeterna-sync"
                    }
                }
            }"#,
        )
        .unwrap();

        assert!(parse_token_review_response(response).is_none());
    }

    #[tokio::test]
    async fn rejects_missing_username_in_tokenreview_response() {
        let response: TokenReviewResponse = serde_json::from_str(
            r#"{
                "status": {
                    "authenticated": true,
                    "user": {}
                }
            }"#,
        )
        .unwrap();

        assert!(parse_token_review_response(response).is_none());
    }
}
