use std::path::Path;

use anyhow::Result;

use super::types::{LlmProvider, SetupConfig};

pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>,
}

pub fn validate_config(config_path: &Path) -> Result<ValidationResult> {
    let mut issues = Vec::new();

    if !config_path.exists() {
        issues.push("Configuration file does not exist".to_string());
        return Ok(ValidationResult {
            is_valid: false,
            issues,
        });
    }

    let content = std::fs::read_to_string(config_path)?;

    if let Err(e) = toml::from_str::<toml::Value>(&content) {
        issues.push(format!("Invalid TOML syntax: {e}"));
    }

    Ok(ValidationResult {
        is_valid: issues.is_empty(),
        issues,
    })
}

pub fn validate_generated_values(yaml: &str) -> std::result::Result<(), Vec<String>> {
    match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(_) => Ok(()),
        Err(e) => Err(vec![format!("invalid YAML: {e}")]),
    }
}

pub fn validate_setup_config(config: &SetupConfig) -> std::result::Result<(), Vec<String>> {
    let mut issues = Vec::new();

    match config.llm_provider {
        LlmProvider::Google => {
            let google = config.google_llm.as_ref();
            if google.is_none() {
                issues.push("google provider requires google_llm configuration".to_string());
            }
            if google.is_none_or(|g| g.project_id.trim().is_empty()) {
                issues.push("google provider requires a non-empty project_id".to_string());
            }
            if google.is_none_or(|g| g.location.trim().is_empty()) {
                issues.push("google provider requires a non-empty location".to_string());
            }
            if google.is_none_or(|g| g.model.trim().is_empty()) {
                issues.push("google provider requires a non-empty model".to_string());
            }
            if google.is_none_or(|g| g.embedding_model.trim().is_empty()) {
                issues.push("google provider requires a non-empty embedding_model".to_string());
            }
        }
        LlmProvider::Bedrock => {
            let bedrock = config.bedrock_llm.as_ref();
            if bedrock.is_none() {
                issues.push("bedrock provider requires bedrock_llm configuration".to_string());
            }
            if bedrock.is_none_or(|b| b.region.trim().is_empty()) {
                issues.push("bedrock provider requires a non-empty region".to_string());
            }
            if bedrock.is_none_or(|b| b.model.trim().is_empty()) {
                issues.push("bedrock provider requires a non-empty model".to_string());
            }
            if bedrock.is_none_or(|b| b.embedding_model.trim().is_empty()) {
                issues.push("bedrock provider requires a non-empty embedding_model".to_string());
            }
        }
        _ => {}
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_generated_values_valid_yaml() {
        let yaml = "key: value\nnested:\n  foo: bar\n";
        assert!(validate_generated_values(yaml).is_ok());
    }

    #[test]
    fn test_validate_generated_values_invalid_yaml() {
        let yaml = ":\n  - bad yaml\n  : : :\n\t\x00";
        let result = validate_generated_values(yaml);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("invalid YAML"));
    }

    #[test]
    fn test_validate_config_missing_file() {
        let result =
            validate_config(Path::new("/tmp/nonexistent_aeterna_test_config.toml")).unwrap();
        assert!(!result.is_valid);
        assert!(result.issues[0].contains("does not exist"));
    }

    #[test]
    fn test_validate_setup_config_google_requires_project_id() {
        let mut config = SetupConfig::default();
        config.llm_provider = LlmProvider::Google;
        config.google_llm = Some(super::super::types::GoogleLlmConfig {
            project_id: String::new(),
            location: "us-central1".into(),
            model: "gemini-2.5-flash".into(),
            embedding_model: "text-embedding-005".into(),
        });

        let errors = validate_setup_config(&config).unwrap_err();
        assert!(errors.iter().any(|error| error.contains("project_id")));
    }

    #[test]
    fn test_validate_setup_config_bedrock_requires_region() {
        let mut config = SetupConfig::default();
        config.llm_provider = LlmProvider::Bedrock;
        config.bedrock_llm = Some(super::super::types::BedrockLlmConfig {
            region: String::new(),
            model: "amazon.nova-micro-v1:0".into(),
            embedding_model: "amazon.titan-embed-text-v2:0".into(),
        });

        let errors = validate_setup_config(&config).unwrap_err();
        assert!(errors.iter().any(|error| error.contains("region")));
    }
}
