//! Deployment environment classification.
//!
//! [`Environment`] models *where* the running process believes it is —
//! development workstation, CI, staging, or production. It feeds the
//! production safety gates (e.g. KMS provider selection in
//! `storage::secret_backend`) that refuse non-production-grade backends in
//! a production deployment.
//!
//! The single source of truth is the `AETERNA_ENV` environment variable.
//! When unset, [`Environment::from_env`] defaults to
//! [`Environment::Development`] so local builds and unit tests work without
//! extra setup. The production gate only fires when an operator explicitly
//! opts into `AETERNA_ENV=production`, which is exactly when the loudest
//! protection is wanted.

use std::str::FromStr;

/// The deployment environment the binary was started in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Environment {
    /// Local developer workstation. The default when `AETERNA_ENV` is unset.
    Development,
    /// Continuous integration runs.
    Ci,
    /// A pre-production staging environment.
    Staging,
    /// Production. Enables strict guardrails — non-production-grade
    /// secret backends, KMS providers, and similar are rejected at startup.
    Production,
}

impl Environment {
    /// Read the environment from the `AETERNA_ENV` variable.
    ///
    /// Recognised values (case-insensitive): `development` / `dev`,
    /// `ci`, `staging` / `stage`, `production` / `prod`. Unset or
    /// unrecognised values default to [`Environment::Development`].
    pub fn from_env() -> Self {
        std::env::var("AETERNA_ENV")
            .ok()
            .as_deref()
            .and_then(|v| Self::from_str(v).ok())
            .unwrap_or(Self::Development)
    }

    /// `true` only for [`Environment::Production`].
    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

impl FromStr for Environment {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "ci" => Ok(Self::Ci),
            "staging" | "stage" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Development => "development",
            Self::Ci => "ci",
            Self::Staging => "staging",
            Self::Production => "production",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_aliases_case_insensitive() {
        for (raw, expected) in [
            ("development", Environment::Development),
            ("DEV", Environment::Development),
            ("Ci", Environment::Ci),
            ("staging", Environment::Staging),
            ("STAGE", Environment::Staging),
            ("production", Environment::Production),
            ("Prod", Environment::Production),
            ("  production  ", Environment::Production),
        ] {
            assert_eq!(Environment::from_str(raw).unwrap(), expected, "raw={raw}");
        }
    }

    #[test]
    fn unknown_values_error() {
        assert!(Environment::from_str("preprod").is_err());
        assert!(Environment::from_str("").is_err());
        assert!(Environment::from_str("test").is_err());
    }

    #[test]
    fn is_production_only_true_for_production() {
        assert!(!Environment::Development.is_production());
        assert!(!Environment::Ci.is_production());
        assert!(!Environment::Staging.is_production());
        assert!(Environment::Production.is_production());
    }

    #[test]
    fn display_round_trips_via_from_str() {
        for env in [
            Environment::Development,
            Environment::Ci,
            Environment::Staging,
            Environment::Production,
        ] {
            let s = env.to_string();
            assert_eq!(Environment::from_str(&s).unwrap(), env);
        }
    }
}
