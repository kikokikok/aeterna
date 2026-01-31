use crate::config::{IdpProvider, IdpSyncConfig};
use crate::error::IdpSyncResult;
use crate::okta::{IdpClient, IdpGroup, IdpUser, UserStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct IdpSyncService {
    config: IdpSyncConfig,
    client: Arc<dyn IdpClient>,
    db_pool: PgPool
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub users_created: u32,
    pub users_updated: u32,
    pub users_deactivated: u32,
    pub groups_synced: u32,
    pub memberships_added: u32,
    pub memberships_removed: u32,
    pub errors: Vec<SyncError>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncError {
    pub entity_type: String,
    pub entity_id: String,
    pub error: String,
    pub timestamp: DateTime<Utc>
}

impl SyncReport {
    pub fn new() -> Self {
        Self {
            started_at: Utc::now(),
            ..Default::default()
        }
    }

    pub fn complete(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn add_error(&mut self, entity_type: &str, entity_id: &str, error: impl ToString) {
        self.errors.push(SyncError {
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            error: error.to_string(),
            timestamp: Utc::now()
        });
    }
}

impl IdpSyncService {
    pub fn new(config: IdpSyncConfig, client: Arc<dyn IdpClient>, db_pool: PgPool) -> Self {
        Self {
            config,
            client,
            db_pool
        }
    }

    pub async fn sync_all(&self) -> IdpSyncResult<SyncReport> {
        let mut report = SyncReport::new();
        info!("Starting full IdP sync");

        let users = self.fetch_all_users().await?;
        info!(count = users.len(), "Fetched users from IdP");

        let groups = self.fetch_all_groups().await?;
        info!(count = groups.len(), "Fetched groups from IdP");

        self.sync_users(&users, &mut report).await?;
        self.sync_groups_and_memberships(&groups, &mut report)
            .await?;

        report.complete();
        info!(
            users_created = report.users_created,
            users_updated = report.users_updated,
            users_deactivated = report.users_deactivated,
            groups_synced = report.groups_synced,
            memberships_added = report.memberships_added,
            memberships_removed = report.memberships_removed,
            errors = report.errors.len(),
            "Sync completed"
        );

        Ok(report)
    }

    async fn fetch_all_users(&self) -> IdpSyncResult<Vec<IdpUser>> {
        let mut all_users = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let page = self.client.list_users(page_token.as_deref()).await?;
            all_users.extend(page.users);
            page_token = page.next_page_token;

            if page_token.is_none() {
                break;
            }
        }

        Ok(all_users)
    }

    async fn fetch_all_groups(&self) -> IdpSyncResult<Vec<IdpGroup>> {
        let mut all_groups = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let page = self.client.list_groups(page_token.as_deref()).await?;
            all_groups.extend(page.groups);
            page_token = page.next_page_token;

            if page_token.is_none() {
                break;
            }
        }

        Ok(all_groups)
    }

    async fn sync_users(&self, users: &[IdpUser], report: &mut SyncReport) -> IdpSyncResult<()> {
        let idp_provider = self.get_provider_name();
        let existing_users = self.get_existing_users_by_idp(&idp_provider).await?;

        let mut seen_idp_subjects: HashSet<String> = HashSet::new();

        for user in users {
            seen_idp_subjects.insert(user.idp_subject.clone());

            match existing_users.get(&user.idp_subject) {
                Some(existing) => {
                    if self.user_needs_update(existing, user) {
                        if !self.config.dry_run {
                            self.update_user(existing.id, user).await?;
                        }
                        report.users_updated += 1;
                        debug!(email = %user.email, "Updated user");
                    }
                }
                None => {
                    if !self.config.dry_run {
                        self.create_user(user).await?;
                    }
                    report.users_created += 1;
                    debug!(email = %user.email, "Created user");
                }
            }
        }

        for (idp_subject, existing) in &existing_users {
            if !seen_idp_subjects.contains(idp_subject) {
                if !self.config.dry_run {
                    self.deactivate_user(existing.id).await?;
                }
                report.users_deactivated += 1;
                debug!(email = %existing.email, "Deactivated user");
            }
        }

        Ok(())
    }

    async fn sync_groups_and_memberships(
        &self,
        groups: &[IdpGroup],
        report: &mut SyncReport
    ) -> IdpSyncResult<()> {
        let group_to_team_mapping = self.get_group_to_team_mapping().await?;

        for group in groups {
            let team_id = match group_to_team_mapping.get(&group.id) {
                Some(id) => *id,
                None => {
                    debug!(group_id = %group.id, group_name = %group.name, "No team mapping for group, skipping");
                    continue;
                }
            };

            let members = match self.client.get_group_members(&group.id).await {
                Ok(m) => m,
                Err(e) => {
                    report.add_error("group", &group.id, &e);
                    warn!(group_id = %group.id, error = %e, "Failed to fetch group members");
                    continue;
                }
            };

            let result = self.sync_team_memberships(team_id, &members, report).await;
            if let Err(e) = result {
                report.add_error("group", &group.id, &e);
                warn!(group_id = %group.id, error = %e, "Failed to sync team memberships");
            }

            report.groups_synced += 1;
        }

        Ok(())
    }

    async fn sync_team_memberships(
        &self,
        team_id: Uuid,
        members: &[IdpUser],
        report: &mut SyncReport
    ) -> IdpSyncResult<()> {
        let existing_memberships = self.get_team_memberships(team_id).await?;
        let mut expected_user_ids: HashSet<Uuid> = HashSet::new();

        for member in members {
            if let Some(user_id) = self.get_user_id_by_idp_subject(&member.idp_subject).await? {
                expected_user_ids.insert(user_id);

                if !existing_memberships.contains(&user_id) {
                    if !self.config.dry_run {
                        self.add_team_membership(team_id, user_id, "member").await?;
                    }
                    report.memberships_added += 1;
                }
            }
        }

        for existing_user_id in &existing_memberships {
            if !expected_user_ids.contains(existing_user_id) {
                if !self.config.dry_run {
                    self.remove_team_membership(team_id, *existing_user_id)
                        .await?;
                }
                report.memberships_removed += 1;
            }
        }

        Ok(())
    }

    fn get_provider_name(&self) -> String {
        match &self.config.provider {
            IdpProvider::Okta(_) => "okta".to_string(),
            IdpProvider::AzureAd(_) => "azure_ad".to_string()
        }
    }

    fn user_needs_update(&self, existing: &ExistingUser, idp_user: &IdpUser) -> bool {
        existing.email != idp_user.email
            || existing.first_name != idp_user.first_name
            || existing.last_name != idp_user.last_name
            || existing.display_name != idp_user.display_name
            || (idp_user.status == UserStatus::Active) != existing.is_active
    }

    async fn get_existing_users_by_idp(
        &self,
        provider: &str
    ) -> IdpSyncResult<HashMap<String, ExistingUser>> {
        let rows = sqlx::query_as::<_, ExistingUser>(
            r#"
            SELECT id, email, first_name, last_name, display_name, idp_subject, is_active
            FROM users
            WHERE idp_provider = $1
            "#
        )
        .bind(provider)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|u| (u.idp_subject.clone(), u))
            .collect())
    }

    async fn create_user(&self, user: &IdpUser) -> IdpSyncResult<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, first_name, last_name, display_name, idp_provider, idp_subject, is_active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            "#,
        )
        .bind(id)
        .bind(&user.email)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.display_name)
        .bind(&user.idp_provider)
        .bind(&user.idp_subject)
        .bind(user.status == UserStatus::Active)
        .execute(&self.db_pool)
        .await?;

        Ok(id)
    }

    async fn update_user(&self, user_id: Uuid, user: &IdpUser) -> IdpSyncResult<()> {
        sqlx::query(
            r#"
            UPDATE users
            SET email = $2, first_name = $3, last_name = $4, display_name = $5, is_active = $6, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .bind(&user.email)
        .bind(&user.first_name)
        .bind(&user.last_name)
        .bind(&user.display_name)
        .bind(user.status == UserStatus::Active)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn deactivate_user(&self, user_id: Uuid) -> IdpSyncResult<()> {
        sqlx::query(
            r#"
            UPDATE users SET is_active = false, deactivated_at = NOW(), updated_at = NOW()
            WHERE id = $1
            "#
        )
        .bind(user_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn get_group_to_team_mapping(&self) -> IdpSyncResult<HashMap<String, Uuid>> {
        let rows = sqlx::query_as::<_, (String, Uuid)>(
            r#"
            SELECT idp_group_id, team_id
            FROM idp_group_mappings
            WHERE idp_group_id IS NOT NULL
            "#
        )
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows.into_iter().collect())
    }

    async fn get_team_memberships(&self, team_id: Uuid) -> IdpSyncResult<HashSet<Uuid>> {
        let rows = sqlx::query_as::<_, (Uuid,)>(
            r#"
            SELECT user_id FROM memberships WHERE team_id = $1
            "#
        )
        .bind(team_id)
        .fetch_all(&self.db_pool)
        .await?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    async fn get_user_id_by_idp_subject(&self, idp_subject: &str) -> IdpSyncResult<Option<Uuid>> {
        let row = sqlx::query_as::<_, (Uuid,)>(
            r#"
            SELECT id FROM users WHERE idp_subject = $1
            "#
        )
        .bind(idp_subject)
        .fetch_optional(&self.db_pool)
        .await?;

        Ok(row.map(|(id,)| id))
    }

    async fn add_team_membership(
        &self,
        team_id: Uuid,
        user_id: Uuid,
        role: &str
    ) -> IdpSyncResult<()> {
        sqlx::query(
            r#"
            INSERT INTO memberships (id, team_id, user_id, role, created_at, updated_at)
            VALUES ($1, $2, $3, $4, NOW(), NOW())
            ON CONFLICT (team_id, user_id) DO NOTHING
            "#
        )
        .bind(Uuid::new_v4())
        .bind(team_id)
        .bind(user_id)
        .bind(role)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    async fn remove_team_membership(&self, team_id: Uuid, user_id: Uuid) -> IdpSyncResult<()> {
        sqlx::query(
            r#"
            DELETE FROM memberships WHERE team_id = $1 AND user_id = $2
            "#
        )
        .bind(team_id)
        .bind(user_id)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ExistingUser {
    id: Uuid,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    display_name: Option<String>,
    idp_subject: String,
    is_active: bool
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_report() {
        let mut report = SyncReport::new();
        assert!(report.completed_at.is_none());
        assert!(!report.has_errors());

        report.add_error("user", "123", "test error");
        assert!(report.has_errors());

        report.complete();
        assert!(report.completed_at.is_some());
    }

    #[test]
    fn test_sync_error_serialization() {
        let error = SyncError {
            entity_type: "user".to_string(),
            entity_id: "123".to_string(),
            error: "test".to_string(),
            timestamp: Utc::now()
        };

        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("user"));
        assert!(json.contains("123"));
    }
}
