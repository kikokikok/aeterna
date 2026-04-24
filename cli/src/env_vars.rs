pub const AETERNA_TENANT_ID: &str = "AETERNA_TENANT_ID";
pub const AETERNA_K8S_NAMESPACE: &str = "AETERNA_K8S_NAMESPACE";
pub const AETERNA_AUTH_BACKEND: &str = "AETERNA_AUTH_BACKEND";
pub const AETERNA_DEFAULT_TENANT_ID: &str = "AETERNA_DEFAULT_TENANT_ID";

/// CI / scripted bearer token override (B2 §10.6).
///
/// When set, the value is treated as an opaque bearer token and used
/// verbatim in the `Authorization` header; the credentials-file flow
/// is bypassed entirely (no refresh attempt, no expiry check — the
/// server is the source of truth and will 401 on a stale token).
/// Intended for CI gates and unattended scripts where the operator
/// provisioned a service token via `aeterna auth token create` out
/// of band and exported it as an env var.
///
/// Takes precedence over `~/.config/aeterna/credentials.toml`. The
/// deprecated `--token` CLI flag is explicitly rejected in favour
/// of this variable; see `main::reject_legacy_token_flag`.
pub const AETERNA_API_TOKEN: &str = "AETERNA_API_TOKEN";
