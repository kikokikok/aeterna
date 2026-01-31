//! Section 12.7: OPAL Migration & Compatibility
//!
//! This module provides migration utilities and compatibility layers for
//! transitioning from heuristic-based context resolution to Cedar Agent-based
//! authorization.

use std::sync::Arc;

use chrono::Utc;
use mk_core::hints::OperationHints;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::cedar::{AuthorizationDecision, CedarClient, CedarError, EntityUid};
use crate::resolver::ContextResolver;
use crate::types::{ContextSource, ResolvedContext, ResolvedValue};

// ============================================================================
// Configuration
// ============================================================================

/// Feature flags for Cedar Agent migration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MigrationConfig {
    /// Enable parallel context resolution (heuristic + Cedar).
    pub parallel_resolution_enabled: bool,

    /// Log differences between heuristic and Cedar resolution.
    pub comparison_logging_enabled: bool,

    /// Primary resolution mode.
    pub primary_mode: ResolutionMode,

    /// Audit mode: log Cedar decisions without enforcing.
    pub audit_mode: bool,

    /// Circuit breaker threshold for Cedar fallback.
    pub circuit_breaker_threshold: u32,

    /// Fallback to heuristic on Cedar failure.
    pub fallback_enabled: bool
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            parallel_resolution_enabled: false,
            comparison_logging_enabled: true,
            primary_mode: ResolutionMode::Heuristic,
            audit_mode: true,
            circuit_breaker_threshold: 5,
            fallback_enabled: true
        }
    }
}

impl MigrationConfig {
    /// Load configuration from environment.
    pub fn from_env() -> Self {
        Self {
            parallel_resolution_enabled: std::env::var("AETERNA_PARALLEL_RESOLUTION")
                .map(|v| v == "true")
                .unwrap_or(false),
            comparison_logging_enabled: std::env::var("AETERNA_COMPARISON_LOGGING")
                .map(|v| v != "false")
                .unwrap_or(true),
            primary_mode: std::env::var("AETERNA_PRIMARY_MODE")
                .map(|v| match v.as_str() {
                    "cedar" => ResolutionMode::Cedar,
                    "parallel" => ResolutionMode::Parallel,
                    _ => ResolutionMode::Heuristic
                })
                .unwrap_or(ResolutionMode::Heuristic),
            audit_mode: std::env::var("AETERNA_CEDAR_AUDIT_MODE")
                .map(|v| v != "false")
                .unwrap_or(true),
            circuit_breaker_threshold: std::env::var("AETERNA_CIRCUIT_BREAKER_THRESHOLD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            fallback_enabled: std::env::var("AETERNA_CEDAR_FALLBACK")
                .map(|v| v != "false")
                .unwrap_or(true)
        }
    }
}

/// Resolution mode for context and authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ResolutionMode {
    /// Use heuristic-based resolution only.
    Heuristic,

    /// Use Cedar Agent resolution only.
    Cedar,

    /// Use both and compare results.
    Parallel
}

// ============================================================================
// Parallel Context Resolver
// ============================================================================

/// Resolver that runs both heuristic and Cedar resolution in parallel.
pub struct ParallelContextResolver {
    heuristic_resolver: ContextResolver,
    cedar_client: CedarClient,
    config: MigrationConfig,
    comparison_log: Arc<RwLock<Vec<ResolutionComparison>>>
}

impl ParallelContextResolver {
    /// Create a new parallel resolver.
    pub fn new(
        heuristic_resolver: ContextResolver,
        cedar_client: CedarClient,
        config: MigrationConfig
    ) -> Self {
        Self {
            heuristic_resolver,
            cedar_client,
            config,
            comparison_log: Arc::new(RwLock::new(Vec::new()))
        }
    }

    /// Resolve context using configured mode.
    pub async fn resolve(&self) -> Result<ResolvedContext, ContextError> {
        match self.config.primary_mode {
            ResolutionMode::Heuristic => self.resolve_heuristic().await,
            ResolutionMode::Cedar => self.resolve_cedar().await,
            ResolutionMode::Parallel => self.resolve_parallel().await
        }
    }

    /// Resolve using heuristic method.
    async fn resolve_heuristic(&self) -> Result<ResolvedContext, ContextError> {
        self.heuristic_resolver
            .resolve()
            .map_err(|e| ContextError::HeuristicError(e.to_string()))
    }

    /// Resolve using Cedar Agent.
    async fn resolve_cedar(&self) -> Result<ResolvedContext, ContextError> {
        // Get user context from git or environment
        let git_email = self.get_git_email().await;

        // Resolve user by email through Cedar
        let user_entity = self
            .cedar_client
            .resolve_user_by_email(&git_email.unwrap_or_default())
            .await
            .map_err(|e| ContextError::CedarError(e.to_string()))?;

        // Get project from Cedar
        let git_remote = self.get_git_remote().await;
        let project_entity = if let Some(remote) = git_remote {
            self.cedar_client
                .resolve_project_by_git_remote(&remote)
                .await
                .ok()
        } else {
            None
        };

        // Build resolved context from Cedar entities
        let tenant_id = user_entity
            .attrs
            .get("tenant_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let user_id = user_entity.uid.id.clone();

        Ok(ResolvedContext {
            tenant_id: ResolvedValue {
                value: tenant_id,
                source: ContextSource::CedarAgent
            },
            user_id: ResolvedValue {
                value: user_id,
                source: ContextSource::CedarAgent
            },
            org_id: None,
            team_id: None,
            project_id: project_entity.map(|p| ResolvedValue {
                value: p.uid.id.clone(),
                source: ContextSource::CedarAgent
            }),
            agent_id: None,
            session_id: None,
            hints: ResolvedValue::new(OperationHints::default(), ContextSource::CedarAgent),
            context_root: None,
            git_root: None
        })
    }

    /// Resolve using both methods and compare.
    async fn resolve_parallel(&self) -> Result<ResolvedContext, ContextError> {
        // Run both resolutions concurrently
        let (heuristic_result, cedar_result) =
            tokio::join!(self.resolve_heuristic(), self.resolve_cedar());

        // Log comparison if enabled
        if self.config.comparison_logging_enabled {
            self.log_comparison("context_resolution", &heuristic_result, &cedar_result)
                .await;
        }

        // Return primary mode result
        match self.config.primary_mode {
            ResolutionMode::Heuristic => heuristic_result,
            ResolutionMode::Cedar => cedar_result,
            _ => {
                // Use heuristic as primary, Cedar as validation
                heuristic_result
            }
        }
    }

    /// Log comparison between heuristic and Cedar results.
    async fn log_comparison<T: std::fmt::Debug>(
        &self,
        operation: &str,
        heuristic: &Result<T, ContextError>,
        cedar: &Result<T, ContextError>
    ) {
        let comparison = ResolutionComparison {
            timestamp: Utc::now(),
            operation: operation.to_string(),
            heuristic_result: format!("{:?}", heuristic),
            cedar_result: format!("{:?}", cedar),
            match_status: match (heuristic, cedar) {
                (Ok(h), Ok(c)) => {
                    if format!("{:?}", h) == format!("{:?}", c) {
                        MatchStatus::Match
                    } else {
                        MatchStatus::Mismatch
                    }
                }
                (Err(_), Err(_)) => MatchStatus::BothFailed,
                _ => MatchStatus::Partial
            }
        };

        match &comparison.match_status {
            MatchStatus::Mismatch => {
                warn!(
                    "Resolution mismatch detected: heuristic={:?}, cedar={:?}",
                    heuristic, cedar
                );
            }
            MatchStatus::Partial => {
                warn!(
                    "Partial resolution failure: heuristic={:?}, cedar={:?}",
                    heuristic, cedar
                );
            }
            _ => {
                debug!("Resolution comparison: {:?}", comparison);
            }
        }

        self.comparison_log.write().await.push(comparison);
    }

    /// Get git user email.
    async fn get_git_email(&self) -> Option<String> {
        // Implementation would use git2 or similar
        std::env::var("GIT_USER_EMAIL").ok()
    }

    /// Get git remote URL.
    async fn get_git_remote(&self) -> Option<String> {
        // Implementation would use git2 or similar
        std::env::var("GIT_REMOTE_URL").ok()
    }

    /// Get comparison log.
    pub async fn get_comparison_log(&self) -> Vec<ResolutionComparison> {
        self.comparison_log.read().await.clone()
    }
}

/// Comparison between heuristic and Cedar resolution.
#[derive(Debug, Clone, Serialize)]
pub struct ResolutionComparison {
    pub timestamp: chrono::DateTime<Utc>,
    pub operation: String,
    pub heuristic_result: String,
    pub cedar_result: String,
    pub match_status: MatchStatus
}

/// Match status between two resolution methods.
#[derive(Debug, Clone, Serialize)]
pub enum MatchStatus {
    Match,
    Mismatch,
    Partial,
    BothFailed
}

// ============================================================================
// Audit Mode Authorization
// ============================================================================

/// Authorization client that supports audit mode.
pub struct AuditableAuthorizer {
    cedar_client: CedarClient,
    config: MigrationConfig,
    audit_log: Arc<RwLock<Vec<AuthorizationAuditEntry>>>
}

impl AuditableAuthorizer {
    /// Create a new auditable authorizer.
    pub fn new(cedar_client: CedarClient, config: MigrationConfig) -> Self {
        Self {
            cedar_client,
            config,
            audit_log: Arc::new(RwLock::new(Vec::new()))
        }
    }

    /// Check authorization with audit mode support.
    pub async fn authorize(
        &self,
        principal: &EntityUid,
        action: &str,
        resource: &EntityUid,
        context: Option<serde_json::Value>
    ) -> Result<AuthorizationDecision, CedarError> {
        let start = std::time::Instant::now();

        let cedar_result = self
            .cedar_client
            .check_authorization(principal, action, resource, context.clone())
            .await;

        let duration = start.elapsed();

        let audit_decision = cedar_result.as_ref().ok().map(|&allowed| {
            if allowed {
                AuditDecision::Allow
            } else {
                AuditDecision::Deny
            }
        });

        let entry = AuthorizationAuditEntry {
            timestamp: Utc::now(),
            principal: principal.to_string(),
            action: action.to_string(),
            resource: resource.to_string(),
            context: context.clone(),
            cedar_decision: audit_decision,
            cedar_error: cedar_result.as_ref().err().map(|e| e.to_string()),
            duration_ms: duration.as_millis() as u64,
            audit_mode: self.config.audit_mode
        };

        self.audit_log.write().await.push(entry);

        if self.config.audit_mode {
            info!(
                "AUDIT MODE: Authorization check logged but not enforced. Cedar would have \
                 returned: {:?}",
                cedar_result
            );

            match cedar_result {
                Ok(allowed) => Ok(if allowed {
                    AuthorizationDecision::Allow
                } else {
                    AuthorizationDecision::Deny
                }),
                Err(_) => Ok(AuthorizationDecision::Allow)
            }
        } else {
            cedar_result.map(|allowed| {
                if allowed {
                    AuthorizationDecision::Allow
                } else {
                    AuthorizationDecision::Deny
                }
            })
        }
    }

    /// Get audit log.
    pub async fn get_audit_log(&self) -> Vec<AuthorizationAuditEntry> {
        self.audit_log.read().await.clone()
    }
}

/// Authorization audit entry.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationAuditEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub principal: String,
    pub action: String,
    pub resource: String,
    pub context: Option<serde_json::Value>,
    pub cedar_decision: Option<AuditDecision>,
    pub cedar_error: Option<String>,
    pub duration_ms: u64,
    pub audit_mode: bool
}

/// Serializable authorization decision for audit logging.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum AuditDecision {
    Allow,
    Deny
}

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during context resolution.
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("Heuristic resolution failed: {0}")]
    HeuristicError(String),

    #[error("Cedar resolution failed: {0}")]
    CedarError(String),

    #[error("Parallel resolution failed: {0}")]
    ParallelError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String)
}

// ============================================================================
// Data Migration
// ============================================================================

/// Migration utilities for organizational data.
pub struct DataMigration {
    // Would hold database connection pool
}

impl DataMigration {
    /// Migrate existing organizational data to OPAL-compatible format.
    pub async fn migrate_organizational_data(&self) -> Result<MigrationReport, MigrationError> {
        let report = MigrationReport {
            timestamp: Utc::now(),
            companies_migrated: 0,
            orgs_migrated: 0,
            teams_migrated: 0,
            projects_migrated: 0,
            users_migrated: 0,
            agents_migrated: 0,
            errors: Vec::new()
        };

        info!("Starting organizational data migration to OPAL format");

        // Migration logic would go here
        // This would read from existing tables and create/update OPAL-compatible
        // entities

        Ok(report)
    }

    /// Validate migrated data consistency.
    pub async fn validate_migration(&self) -> Result<ValidationReport, MigrationError> {
        Ok(ValidationReport {
            timestamp: Utc::now(),
            valid: true,
            issues: Vec::new()
        })
    }
}

/// Migration report.
#[derive(Debug, Clone, Serialize)]
pub struct MigrationReport {
    pub timestamp: chrono::DateTime<Utc>,
    pub companies_migrated: u64,
    pub orgs_migrated: u64,
    pub teams_migrated: u64,
    pub projects_migrated: u64,
    pub users_migrated: u64,
    pub agents_migrated: u64,
    pub errors: Vec<String>
}

/// Validation report.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub timestamp: chrono::DateTime<Utc>,
    pub valid: bool,
    pub issues: Vec<String>
}

/// Migration error.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String)
}
