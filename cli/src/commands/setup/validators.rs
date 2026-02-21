use std::path::Path;

use anyhow::Result;

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
        issues.push(format!("Invalid TOML syntax: {}", e));
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
}
