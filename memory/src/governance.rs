use serde_json::Value;

pub struct GovernanceService {}

impl GovernanceService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn redact_pii(&self, content: &str) -> String {
        utils::redact_pii(content)
    }

    pub fn is_sensitive(&self, metadata: &Value) -> bool {
        if let Some(obj) = metadata.as_object() {
            if let Some(sensitive) = obj.get("sensitive") {
                if let Some(b) = sensitive.as_bool() {
                    if b {
                        return true;
                    }
                }
            }
            if let Some(private) = obj.get("private") {
                if let Some(b) = private.as_bool() {
                    if b {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn can_promote(&self, _content: &str, metadata: &Value) -> bool {
        if self.is_sensitive(metadata) {
            return false;
        }
        true
    }
}

impl Default for GovernanceService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pii_redaction() {
        let service = GovernanceService::new();
        let content = "Contact me at user@example.com for details.";
        let redacted = service.redact_pii(content);
        assert_eq!(redacted, "Contact me at [REDACTED_EMAIL] for details.");
    }

    #[test]
    fn test_sensitivity_check() {
        let service = GovernanceService::new();

        let metadata_sensitive = json!({ "sensitive": true });
        assert!(service.is_sensitive(&metadata_sensitive));

        let metadata_private = json!({ "private": true });
        assert!(service.is_sensitive(&metadata_private));

        let metadata_safe = json!({ "tags": ["rust"] });
        assert!(!service.is_sensitive(&metadata_safe));
    }

    #[test]
    fn test_can_promote() {
        let service = GovernanceService::new();
        let content = "Safe content";
        let metadata = json!({ "sensitive": false });
        assert!(service.can_promote(content, &metadata));

        let metadata_sensitive = json!({ "sensitive": true });
        assert!(!service.can_promote(content, &metadata_sensitive));
    }

    #[test]
    fn test_governance_default() {
        let _ = GovernanceService::default();
    }

    #[test]
    fn test_is_sensitive_non_object() {
        let service = GovernanceService::new();
        assert!(!service.is_sensitive(&json!("not an object")));
        assert!(!service.is_sensitive(&json!(null)));
    }

    #[test]
    fn test_is_sensitive_mixed_types() {
        let service = GovernanceService::new();
        assert!(!service.is_sensitive(&json!({ "sensitive": "true" })));
        assert!(!service.is_sensitive(&json!({ "private": 123 })));
    }
}
