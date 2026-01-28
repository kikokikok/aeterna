use mk_core::hints::{HintsConfig, OperationHints};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum ContextSource {
    Explicit,
    EnvVar(String),
    ContextToml(PathBuf),
    GitConfig,
    GitRemote,
    CedarAgent,
    OrgDefault,
    SystemDefault
}

impl std::fmt::Display for ContextSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextSource::Explicit => write!(f, "explicit"),
            ContextSource::EnvVar(name) => write!(f, "env:{name}"),
            ContextSource::ContextToml(path) => write!(f, "context.toml:{}", path.display()),
            ContextSource::GitConfig => write!(f, "git-config"),
            ContextSource::GitRemote => write!(f, "git-remote"),
            ContextSource::CedarAgent => write!(f, "cedar-agent"),
            ContextSource::OrgDefault => write!(f, "org-default"),
            ContextSource::SystemDefault => write!(f, "system-default")
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedValue<T> {
    pub value: T,
    pub source: ContextSource
}

impl<T> ResolvedValue<T> {
    pub fn new(value: T, source: ContextSource) -> Self {
        Self { value, source }
    }

    pub fn explicit(value: T) -> Self {
        Self::new(value, ContextSource::Explicit)
    }

    pub fn default(value: T) -> Self {
        Self::new(value, ContextSource::SystemDefault)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedContext {
    pub tenant_id: ResolvedValue<String>,
    pub user_id: ResolvedValue<String>,
    pub org_id: Option<ResolvedValue<String>>,
    pub team_id: Option<ResolvedValue<String>>,
    pub project_id: Option<ResolvedValue<String>>,
    pub agent_id: Option<ResolvedValue<String>>,
    pub session_id: Option<ResolvedValue<String>>,
    pub hints: ResolvedValue<OperationHints>,
    pub context_root: Option<PathBuf>,
    pub git_root: Option<PathBuf>
}

impl Default for ResolvedContext {
    fn default() -> Self {
        Self {
            tenant_id: ResolvedValue::default("default".to_string()),
            user_id: ResolvedValue::default("default".to_string()),
            org_id: None,
            team_id: None,
            project_id: None,
            agent_id: None,
            session_id: None,
            hints: ResolvedValue::default(OperationHints::default()),
            context_root: None,
            git_root: None
        }
    }
}

impl ResolvedContext {
    pub fn to_tenant_context(&self) -> mk_core::TenantContext {
        let tenant_id = mk_core::TenantId::new(self.tenant_id.value.clone())
            .unwrap_or_else(mk_core::TenantId::default);
        let user_id = mk_core::UserId::new(self.user_id.value.clone())
            .unwrap_or_else(mk_core::UserId::default);

        match &self.agent_id {
            Some(agent) => {
                mk_core::TenantContext::with_agent(tenant_id, user_id, agent.value.clone())
            }
            None => mk_core::TenantContext::new(tenant_id, user_id)
        }
    }

    /// Returns the resolved operation hints.
    pub fn to_hints(&self) -> OperationHints {
        self.hints.value.clone()
    }

    pub fn explain(&self) -> Vec<(String, String, String)> {
        let mut explanations = vec![
            (
                "tenant_id".to_string(),
                self.tenant_id.value.clone(),
                self.tenant_id.source.to_string()
            ),
            (
                "user_id".to_string(),
                self.user_id.value.clone(),
                self.user_id.source.to_string()
            ),
        ];

        if let Some(org) = &self.org_id {
            explanations.push((
                "org_id".to_string(),
                org.value.clone(),
                org.source.to_string()
            ));
        }

        if let Some(team) = &self.team_id {
            explanations.push((
                "team_id".to_string(),
                team.value.clone(),
                team.source.to_string()
            ));
        }

        if let Some(project) = &self.project_id {
            explanations.push((
                "project_id".to_string(),
                project.value.clone(),
                project.source.to_string()
            ));
        }

        if let Some(agent) = &self.agent_id {
            explanations.push((
                "agent_id".to_string(),
                agent.value.clone(),
                agent.source.to_string()
            ));
        }

        explanations.push((
            "hints".to_string(),
            format!("{:?}", self.hints.value.preset),
            self.hints.source.to_string()
        ));

        explanations
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContextConfig {
    #[serde(default)]
    pub tenant_id: Option<String>,

    #[serde(default)]
    pub user_id: Option<String>,

    #[serde(default)]
    pub org_id: Option<String>,

    #[serde(default)]
    pub team_id: Option<String>,

    #[serde(default)]
    pub project_id: Option<String>,

    #[serde(default)]
    pub agent_id: Option<String>,

    #[serde(default)]
    pub hints: Option<HintsConfig>,

    #[serde(default)]
    pub server: Option<ServerConfig>,

    #[serde(default)]
    pub storage: Option<StorageConfig>,

    #[serde(default)]
    pub extra: HashMap<String, toml::Value>
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    pub url: Option<String>,

    pub api_key: Option<String>,

    pub timeout_seconds: Option<u64>
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct StorageConfig {
    pub data_dir: Option<PathBuf>,

    pub cache_dir: Option<PathBuf>,

    pub logs_dir: Option<PathBuf>
}

impl ContextConfig {
    pub fn from_toml(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn to_hints(&self) -> OperationHints {
        match &self.hints {
            Some(hints_config) => hints_config.to_operation_hints(),
            None => OperationHints::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mk_core::hints::HintPreset;

    #[test]
    fn test_context_source_display() {
        assert_eq!(ContextSource::Explicit.to_string(), "explicit");
        assert_eq!(
            ContextSource::EnvVar("AETERNA_TENANT".to_string()).to_string(),
            "env:AETERNA_TENANT"
        );
        assert_eq!(ContextSource::GitRemote.to_string(), "git-remote");
    }

    #[test]
    fn test_resolved_value_explicit() {
        let val = ResolvedValue::explicit("test".to_string());
        assert_eq!(val.value, "test");
        assert_eq!(val.source, ContextSource::Explicit);
    }

    #[test]
    fn test_resolved_context_default() {
        let ctx = ResolvedContext::default();
        assert_eq!(ctx.tenant_id.value, "default");
        assert_eq!(ctx.user_id.value, "default");
        assert!(ctx.org_id.is_none());
    }

    #[test]
    fn test_resolved_context_to_tenant_context() {
        let mut ctx = ResolvedContext::default();
        ctx.tenant_id = ResolvedValue::explicit("acme-corp".to_string());
        ctx.user_id = ResolvedValue::explicit("alice".to_string());

        let tenant_ctx = ctx.to_tenant_context();
        assert_eq!(tenant_ctx.tenant_id.as_str(), "acme-corp");
        assert_eq!(tenant_ctx.user_id.as_str(), "alice");
    }

    #[test]
    fn test_context_config_from_toml() {
        let toml_content = r#"
tenant-id = "acme-corp"
user-id = "alice"
org-id = "platform"
project-id = "payments"

[hints]
preset = "fast"
verbose = true

[server]
url = "https://aeterna.example.com"
timeout-seconds = 30
"#;

        let config = ContextConfig::from_toml(toml_content).unwrap();
        assert_eq!(config.tenant_id, Some("acme-corp".to_string()));
        assert_eq!(config.user_id, Some("alice".to_string()));
        assert_eq!(config.org_id, Some("platform".to_string()));
        assert_eq!(config.project_id, Some("payments".to_string()));
        assert!(config.hints.is_some());
        assert!(config.server.is_some());
    }

    #[test]
    fn test_context_config_to_hints() {
        let config = ContextConfig {
            hints: Some(HintsConfig {
                preset: Some(HintPreset::Fast),
                ..Default::default()
            }),
            ..Default::default()
        };

        let hints = config.to_hints();
        assert!(!hints.reasoning);
        assert!(!hints.multi_hop);
    }

    #[test]
    fn test_resolved_context_explain() {
        let ctx = ResolvedContext::default();
        let explanations = ctx.explain();
        assert!(explanations.iter().any(|(name, _, _)| name == "tenant_id"));
        assert!(explanations.iter().any(|(name, _, _)| name == "user_id"));
        assert!(explanations.iter().any(|(name, _, _)| name == "hints"));
    }
}
