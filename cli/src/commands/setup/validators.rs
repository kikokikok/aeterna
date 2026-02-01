use std::path::Path;

use anyhow::Result;

pub struct ValidationResult {
    pub is_valid: bool,
    pub issues: Vec<String>
}

pub fn validate_config(config_path: &Path) -> Result<ValidationResult> {
    let mut issues = Vec::new();

    if !config_path.exists() {
        issues.push("Configuration file does not exist".to_string());
        return Ok(ValidationResult {
            is_valid: false,
            issues
        });
    }

    let content = std::fs::read_to_string(config_path)?;

    if let Err(e) = toml::from_str::<toml::Value>(&content) {
        issues.push(format!("Invalid TOML syntax: {}", e));
    }

    Ok(ValidationResult {
        is_valid: issues.is_empty(),
        issues
    })
}
