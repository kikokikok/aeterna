use crate::config::AzureAdConfig;
use crate::error::{IdpSyncError, IdpSyncResult};
use crate::okta::{GroupPage, GroupType, IdpClient, IdpGroup, IdpUser, UserPage, UserStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

pub struct AzureAdClient {
    http_client: Client,
    config: AzureAdConfig,
    access_token: Arc<RwLock<Option<CachedToken>>>
}

struct CachedToken {
    token: String,
    expires_at: DateTime<Utc>
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    expires_in: u64
}

impl AzureAdClient {
    pub fn new(config: AzureAdConfig) -> IdpSyncResult<Self> {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(IdpSyncError::HttpError)?;

        Ok(Self {
            http_client,
            config,
            access_token: Arc::new(RwLock::new(None))
        })
    }

    async fn get_access_token(&self) -> IdpSyncResult<String> {
        {
            let cached = self.access_token.read().await;
            if let Some(ref token) = *cached {
                if token.expires_at > Utc::now() + chrono::Duration::minutes(5) {
                    return Ok(token.token.clone());
                }
            }
        }

        let token_url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.config.tenant_id
        );

        let body = format!(
            "client_id={}&client_secret={}&scope={}&grant_type=client_credentials",
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&self.config.client_secret),
            urlencoding::encode("https://graph.microsoft.com/.default")
        );

        let response = self
            .http_client
            .post(&token_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(IdpSyncError::HttpError)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response.text().await.unwrap_or_default();
            return Err(IdpSyncError::OAuthError(format!(
                "Token request failed: {} - {}",
                status, error_body
            )));
        }

        let token_response: OAuthTokenResponse = response.json().await.map_err(|e| {
            IdpSyncError::OAuthError(format!("Failed to parse token response: {}", e))
        })?;

        let expires_at = Utc::now() + chrono::Duration::seconds(token_response.expires_in as i64);

        {
            let mut cached = self.access_token.write().await;
            *cached = Some(CachedToken {
                token: token_response.access_token.clone(),
                expires_at
            });
        }

        Ok(token_response.access_token)
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, url: &str) -> IdpSyncResult<T> {
        let token = self.get_access_token().await?;
        debug!(url = %url, "Making Microsoft Graph API request");

        let response = self
            .http_client
            .get(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/json")
            .send()
            .await?;

        match response.status() {
            StatusCode::OK => {
                let body = response.json::<T>().await?;
                Ok(body)
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                Err(IdpSyncError::RateLimited {
                    retry_after_seconds: retry_after
                })
            }
            StatusCode::UNAUTHORIZED => {
                let mut cached = self.access_token.write().await;
                *cached = None;
                Err(IdpSyncError::AuthenticationError(
                    "Azure AD authentication failed".to_string()
                ))
            }
            StatusCode::NOT_FOUND => Err(IdpSyncError::UserNotFound(url.to_string())),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(IdpSyncError::IdpApiError {
                    status: status.as_u16(),
                    message: body
                })
            }
        }
    }

    fn graph_user_to_idp_user(&self, user: GraphUser) -> IdpUser {
        IdpUser {
            id: user.id.clone(),
            email: user.mail.or(user.user_principal_name).unwrap_or_default(),
            first_name: user.given_name,
            last_name: user.surname,
            display_name: user.display_name,
            status: if user.account_enabled.unwrap_or(true) {
                UserStatus::Active
            } else {
                UserStatus::Inactive
            },
            created_at: user.created_date_time.unwrap_or_else(Utc::now),
            updated_at: Utc::now(),
            idp_provider: "azure_ad".to_string(),
            idp_subject: user.id
        }
    }
}

#[async_trait]
impl IdpClient for AzureAdClient {
    async fn list_users(&self, page_token: Option<&str>) -> IdpSyncResult<UserPage> {
        let url = match page_token {
            Some(next_link) => next_link.to_string(),
            None => {
                let mut url = "https://graph.microsoft.com/v1.0/users?$top=100&$select=id,mail,userPrincipalName,givenName,surname,displayName,accountEnabled,createdDateTime".to_string();
                if let Some(filter) = &self.config.group_filter {
                    url.push_str(&format!("&$filter={}", urlencoding::encode(filter)));
                }
                url
            }
        };

        let response: GraphListResponse<GraphUser> = self.get(&url).await?;

        Ok(UserPage {
            users: response
                .value
                .into_iter()
                .map(|u| self.graph_user_to_idp_user(u))
                .collect(),
            next_page_token: response.odata_next_link
        })
    }

    async fn list_groups(&self, page_token: Option<&str>) -> IdpSyncResult<GroupPage> {
        let url = match page_token {
            Some(next_link) => next_link.to_string(),
            None => {
                let mut url = "https://graph.microsoft.com/v1.0/groups?$top=100&$select=id,displayName,description,groupTypes,createdDateTime".to_string();
                if let Some(filter) = &self.config.group_filter {
                    url.push_str(&format!("&$filter={}", urlencoding::encode(filter)));
                }
                url
            }
        };

        let response: GraphListResponse<GraphGroup> = self.get(&url).await?;

        Ok(GroupPage {
            groups: response
                .value
                .into_iter()
                .map(|g| IdpGroup {
                    id: g.id,
                    name: g.display_name,
                    description: g.description,
                    group_type: if g.group_types.contains(&"Unified".to_string()) {
                        GroupType::AppGroup
                    } else {
                        GroupType::OktaGroup
                    },
                    created_at: g.created_date_time.unwrap_or_else(Utc::now),
                    updated_at: Utc::now()
                })
                .collect(),
            next_page_token: response.odata_next_link
        })
    }

    async fn get_group_members(&self, group_id: &str) -> IdpSyncResult<Vec<IdpUser>> {
        let mut all_members = Vec::new();
        let mut url = format!(
            "https://graph.microsoft.com/v1.0/groups/{}/members?$top=100&$select=id,mail,userPrincipalName,givenName,surname,displayName,accountEnabled,createdDateTime",
            group_id
        );

        loop {
            let response: GraphListResponse<GraphUser> = self.get(&url).await?;
            all_members.extend(
                response
                    .value
                    .into_iter()
                    .map(|u| self.graph_user_to_idp_user(u))
            );

            match response.odata_next_link {
                Some(next_url) => url = next_url,
                None => break
            }
        }

        if self.config.include_nested_groups {
            let transitive_url = format!(
                "https://graph.microsoft.com/v1.0/groups/{}/transitiveMembers?$top=100&$select=id,mail,userPrincipalName,givenName,surname,displayName,accountEnabled,createdDateTime",
                group_id
            );
            let response: GraphListResponse<GraphUser> = self.get(&transitive_url).await?;
            for user in response.value {
                let idp_user = self.graph_user_to_idp_user(user);
                if !all_members.iter().any(|m| m.id == idp_user.id) {
                    all_members.push(idp_user);
                }
            }
        }

        Ok(all_members)
    }

    async fn get_user(&self, user_id: &str) -> IdpSyncResult<IdpUser> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{}?$select=id,mail,userPrincipalName,givenName,surname,displayName,accountEnabled,createdDateTime",
            user_id
        );
        let user: GraphUser = self.get(&url).await?;
        Ok(self.graph_user_to_idp_user(user))
    }
}

#[derive(Debug, Deserialize)]
struct GraphListResponse<T> {
    value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    odata_next_link: Option<String>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphUser {
    id: String,
    mail: Option<String>,
    user_principal_name: Option<String>,
    given_name: Option<String>,
    surname: Option<String>,
    display_name: Option<String>,
    account_enabled: Option<bool>,
    created_date_time: Option<DateTime<Utc>>
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphGroup {
    id: String,
    display_name: String,
    description: Option<String>,
    #[serde(default)]
    group_types: Vec<String>,
    created_date_time: Option<DateTime<Utc>>
}

pub fn create_azure_client(config: AzureAdConfig) -> IdpSyncResult<Arc<dyn IdpClient>> {
    Ok(Arc::new(AzureAdClient::new(config)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_user_conversion() {
        let graph_user = GraphUser {
            id: "user-123".to_string(),
            mail: Some("test@example.com".to_string()),
            user_principal_name: Some("test@example.onmicrosoft.com".to_string()),
            given_name: Some("Test".to_string()),
            surname: Some("User".to_string()),
            display_name: Some("Test User".to_string()),
            account_enabled: Some(true),
            created_date_time: Some(Utc::now())
        };

        let config = AzureAdConfig {
            tenant_id: "tenant".to_string(),
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            group_filter: None,
            include_nested_groups: false
        };

        let client = AzureAdClient::new(config).unwrap();
        let idp_user = client.graph_user_to_idp_user(graph_user);

        assert_eq!(idp_user.id, "user-123");
        assert_eq!(idp_user.email, "test@example.com");
        assert_eq!(idp_user.status, UserStatus::Active);
        assert_eq!(idp_user.idp_provider, "azure_ad");
    }
}
