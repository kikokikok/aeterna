use crate::config::OktaConfig;
use crate::error::{IdpSyncError, IdpSyncResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

#[async_trait]
pub trait IdpClient: Send + Sync {
    async fn list_users(&self, page_token: Option<&str>) -> IdpSyncResult<UserPage>;
    async fn list_groups(&self, page_token: Option<&str>) -> IdpSyncResult<GroupPage>;
    async fn get_group_members(&self, group_id: &str) -> IdpSyncResult<Vec<IdpUser>>;
    async fn get_user(&self, user_id: &str) -> IdpSyncResult<IdpUser>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpUser {
    pub id: String,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub display_name: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub idp_provider: String,
    pub idp_subject: String
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserStatus {
    Active,
    Inactive,
    Suspended,
    Deprovisioned,
    Pending
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdpGroup {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub group_type: GroupType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupType {
    OktaGroup,
    AppGroup,
    BuiltIn
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPage {
    pub users: Vec<IdpUser>,
    pub next_page_token: Option<String>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPage {
    pub groups: Vec<IdpGroup>,
    pub next_page_token: Option<String>
}

pub struct OktaClient {
    client: Client,
    config: OktaConfig
}

impl OktaClient {
    pub fn new(config: OktaConfig) -> IdpSyncResult<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(IdpSyncError::HttpError)?;

        Ok(Self { client, config })
    }

    fn base_url(&self) -> String {
        format!("https://{}/api/v1", self.config.domain)
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str
    ) -> IdpSyncResult<(T, Option<String>)> {
        let url = format!("{}{}", self.base_url(), path);
        debug!(url = %url, "Making Okta API request");

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("SSWS {}", self.config.api_token))
            .header("Accept", "application/json")
            .send()
            .await?;

        let next_link = self.extract_next_link(response.headers());

        match response.status() {
            StatusCode::OK => {
                let body = response.json::<T>().await?;
                Ok((body, next_link))
            }
            StatusCode::TOO_MANY_REQUESTS => {
                let retry_after = response
                    .headers()
                    .get("x-rate-limit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                Err(IdpSyncError::RateLimited {
                    retry_after_seconds: retry_after
                })
            }
            StatusCode::UNAUTHORIZED => Err(IdpSyncError::AuthenticationError(
                "Invalid Okta API token".to_string()
            )),
            StatusCode::NOT_FOUND => Err(IdpSyncError::UserNotFound(path.to_string())),
            status => {
                let body = response.text().await.unwrap_or_default();
                Err(IdpSyncError::IdpApiError {
                    status: status.as_u16(),
                    message: body
                })
            }
        }
    }

    fn extract_next_link(&self, headers: &reqwest::header::HeaderMap) -> Option<String> {
        headers
            .get("link")
            .and_then(|v| v.to_str().ok())
            .and_then(|link| {
                for part in link.split(',') {
                    if part.contains("rel=\"next\"") {
                        let url_part = part.split(';').next()?;
                        let url = url_part
                            .trim()
                            .trim_start_matches('<')
                            .trim_end_matches('>');
                        return Some(url.to_string());
                    }
                }
                None
            })
    }

    fn okta_user_to_idp_user(&self, okta_user: OktaUserResponse) -> IdpUser {
        IdpUser {
            id: okta_user.id.clone(),
            email: okta_user.profile.email.clone(),
            first_name: Some(okta_user.profile.first_name),
            last_name: Some(okta_user.profile.last_name),
            display_name: okta_user.profile.display_name,
            status: match okta_user.status.as_str() {
                "ACTIVE" => UserStatus::Active,
                "SUSPENDED" => UserStatus::Suspended,
                "DEPROVISIONED" => UserStatus::Deprovisioned,
                "STAGED" | "PROVISIONED" => UserStatus::Pending,
                _ => UserStatus::Inactive
            },
            created_at: okta_user.created,
            updated_at: okta_user.last_updated,
            idp_provider: "okta".to_string(),
            idp_subject: okta_user.id
        }
    }
}

#[async_trait]
impl IdpClient for OktaClient {
    async fn list_users(&self, page_token: Option<&str>) -> IdpSyncResult<UserPage> {
        let path = match page_token {
            Some(token) => format!("/users?after={}&limit=200", token),
            None => {
                let mut path = "/users?limit=200".to_string();
                if let Some(filter) = &self.config.user_filter {
                    path.push_str(&format!("&filter={}", urlencoding::encode(filter)));
                }
                path
            }
        };

        let (users, next_token): (Vec<OktaUserResponse>, _) = self.get(&path).await?;

        Ok(UserPage {
            users: users
                .into_iter()
                .map(|u| self.okta_user_to_idp_user(u))
                .collect(),
            next_page_token: next_token.and_then(|url| {
                url.split("after=")
                    .nth(1)
                    .and_then(|s| s.split('&').next())
                    .map(|s| s.to_string())
            })
        })
    }

    async fn list_groups(&self, page_token: Option<&str>) -> IdpSyncResult<GroupPage> {
        let path = match page_token {
            Some(token) => format!("/groups?after={}&limit=200", token),
            None => {
                let mut path = "/groups?limit=200".to_string();
                if let Some(filter) = &self.config.group_filter {
                    path.push_str(&format!("&filter={}", urlencoding::encode(filter)));
                }
                path
            }
        };

        let (groups, next_token): (Vec<OktaGroupResponse>, _) = self.get(&path).await?;

        Ok(GroupPage {
            groups: groups
                .into_iter()
                .map(|g| IdpGroup {
                    id: g.id,
                    name: g.profile.name,
                    description: g.profile.description,
                    group_type: match g.group_type.as_str() {
                        "OKTA_GROUP" => GroupType::OktaGroup,
                        "APP_GROUP" => GroupType::AppGroup,
                        "BUILT_IN" => GroupType::BuiltIn,
                        _ => GroupType::OktaGroup
                    },
                    created_at: g.created,
                    updated_at: g.last_updated
                })
                .collect(),
            next_page_token: next_token.and_then(|url| {
                url.split("after=")
                    .nth(1)
                    .and_then(|s| s.split('&').next())
                    .map(|s| s.to_string())
            })
        })
    }

    async fn get_group_members(&self, group_id: &str) -> IdpSyncResult<Vec<IdpUser>> {
        let path = format!("/groups/{}/users?limit=200", group_id);
        let mut all_members = Vec::new();
        let mut current_path = path;

        loop {
            let (users, next_token): (Vec<OktaUserResponse>, _) = self.get(&current_path).await?;
            all_members.extend(users.into_iter().map(|u| self.okta_user_to_idp_user(u)));

            match next_token {
                Some(url) if url.contains("after=") => {
                    let after = url
                        .split("after=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .unwrap_or("");
                    current_path = format!("/groups/{}/users?after={}&limit=200", group_id, after);
                }
                _ => break
            }
        }

        Ok(all_members)
    }

    async fn get_user(&self, user_id: &str) -> IdpSyncResult<IdpUser> {
        let path = format!("/users/{}", user_id);
        let (user, _): (OktaUserResponse, _) = self.get(&path).await?;
        Ok(self.okta_user_to_idp_user(user))
    }
}

#[derive(Debug, Deserialize)]
struct OktaUserResponse {
    id: String,
    status: String,
    created: DateTime<Utc>,
    #[serde(rename = "lastUpdated")]
    last_updated: DateTime<Utc>,
    profile: OktaUserProfile
}

#[derive(Debug, Deserialize)]
struct OktaUserProfile {
    email: String,
    #[serde(rename = "firstName")]
    first_name: String,
    #[serde(rename = "lastName")]
    last_name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>
}

#[derive(Debug, Deserialize)]
struct OktaGroupResponse {
    id: String,
    #[serde(rename = "type")]
    group_type: String,
    created: DateTime<Utc>,
    #[serde(rename = "lastUpdated")]
    last_updated: DateTime<Utc>,
    profile: OktaGroupProfile
}

#[derive(Debug, Deserialize)]
struct OktaGroupProfile {
    name: String,
    description: Option<String>
}

pub fn create_okta_client(config: OktaConfig) -> IdpSyncResult<Arc<dyn IdpClient>> {
    Ok(Arc::new(OktaClient::new(config)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_status_serialization() {
        let status = UserStatus::Active;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"ACTIVE\"");
    }

    #[test]
    fn test_group_type_serialization() {
        let group_type = GroupType::OktaGroup;
        let json = serde_json::to_string(&group_type).unwrap();
        assert_eq!(json, "\"OKTA_GROUP\"");
    }

    #[test]
    fn test_idp_error_retryable() {
        let rate_limited = IdpSyncError::RateLimited {
            retry_after_seconds: 60
        };
        assert!(rate_limited.is_retryable());
        assert_eq!(rate_limited.retry_after(), Some(60));

        let auth_error = IdpSyncError::AuthenticationError("test".to_string());
        assert!(!auth_error.is_retryable());
        assert_eq!(auth_error.retry_after(), None);
    }
}
