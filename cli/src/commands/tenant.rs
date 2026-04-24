use clap::{Args, Subcommand};
use context::ContextResolver;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

use crate::output;
use crate::ux_error;

// ---------------------------------------------------------------------------
// Top-level command
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantCommand {
    #[command(about = "Create a new tenant")]
    Create(TenantCreateArgs),

    #[command(about = "List tenants (platform admin)")]
    List(TenantListArgs),

    #[command(about = "Show tenant details")]
    Show(TenantShowArgs),

    #[command(about = "Update tenant properties")]
    Update(TenantUpdateArgs),

    #[command(about = "Deactivate a tenant")]
    Deactivate(TenantDeactivateArgs),

    #[command(about = "Set default tenant for current context (local .aeterna/context.toml)")]
    Use(TenantUseArgs),

    #[command(
        about = "Switch the server-side default tenant for your user (persists across devices)"
    )]
    Switch(TenantSwitchArgs),

    #[command(about = "Show the currently selected tenant (server preference + local context)")]
    Current(TenantCurrentArgs),

    #[command(
        name = "domain-map",
        about = "Add a verified domain mapping for a tenant"
    )]
    DomainMap(TenantDomainMapArgs),

    #[command(
        name = "repo-binding",
        subcommand,
        about = "Manage tenant repository bindings"
    )]
    RepoBinding(TenantRepoBindingCommand),

    #[command(name = "config", subcommand, about = "Manage tenant configuration")]
    Config(TenantConfigCommand),

    #[command(name = "secret", subcommand, about = "Manage tenant secret entries")]
    Secret(TenantSecretCommand),

    #[command(
        name = "connection",
        subcommand,
        about = "Manage Git provider connection visibility for tenants (PlatformAdmin)"
    )]
    Connection(TenantConnectionCommand),

    #[command(
        name = "validate",
        about = "Validate a tenant manifest against the server (dry-run; no state changes)"
    )]
    Validate(TenantValidateArgs),

    #[command(
        name = "render",
        about = "Render the server's current-state manifest for a tenant"
    )]
    Render(TenantRenderArgs),

    #[command(
        name = "diff",
        about = "Diff an incoming tenant manifest against the server's current state"
    )]
    Diff(TenantDiffArgs),

    #[command(
        name = "apply",
        about = "Apply a tenant manifest (real write; prompts before proceeding unless --yes)"
    )]
    Apply(TenantApplyArgs),

    #[command(
        name = "watch",
        about = "Stream live tenant lifecycle events (SSE; per-step provisioning progress) (B2 §7.5)"
    )]
    Watch(TenantWatchArgs),
}

// ---------------------------------------------------------------------------
// repo-binding sub-commands
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantRepoBindingCommand {
    #[command(about = "Show the repository binding for a tenant")]
    Show(TenantRepoBindingShowArgs),

    #[command(about = "Set the repository binding for a tenant")]
    Set(TenantRepoBindingSetArgs),

    #[command(about = "Validate the repository binding for a tenant")]
    Validate(TenantRepoBindingValidateArgs),
}

#[derive(Subcommand)]
pub enum TenantConfigCommand {
    #[command(about = "Inspect tenant configuration")]
    Inspect(TenantConfigInspectArgs),

    #[command(about = "Upsert tenant configuration from a JSON file")]
    Upsert(TenantConfigUpsertArgs),

    #[command(about = "Validate tenant configuration from a JSON file")]
    Validate(TenantConfigValidateArgs),
}

#[derive(Subcommand)]
pub enum TenantSecretCommand {
    #[command(about = "Set a tenant secret entry")]
    Set(TenantSecretSetArgs),

    #[command(about = "Delete a tenant secret entry")]
    Delete(TenantSecretDeleteArgs),
}

// ---------------------------------------------------------------------------
// Args structs
// ---------------------------------------------------------------------------

#[derive(Args)]
pub struct TenantCreateArgs {
    /// Tenant slug (URL-safe identifier)
    #[arg(long)]
    pub slug: String,

    /// Human-readable tenant name
    #[arg(long)]
    pub name: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would be created without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantListArgs {
    /// Include inactive tenants in output
    #[arg(long)]
    pub include_inactive: bool,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantShowArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantUpdateArgs {
    /// Tenant slug or ID to update
    pub tenant: String,

    /// New slug value
    #[arg(long)]
    pub new_slug: Option<String>,

    /// New human-readable name
    #[arg(long)]
    pub name: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would change without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantDeactivateArgs {
    /// Tenant slug or ID to deactivate
    pub tenant: String,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantUseArgs {
    /// Tenant slug to set as default context
    pub tenant: String,
}

#[derive(Args, Debug)]
pub struct TenantSwitchArgs {
    /// Tenant slug or UUID to persist as the caller's server-side default.
    ///
    /// When present, overrides any X-Tenant-ID header on subsequent
    /// requests that do not carry an explicit tenant hint. Requires
    /// membership in the target tenant (PlatformAdmin exempt).
    pub tenant: String,

    /// Clear the server-side default instead of setting one. The `tenant`
    /// positional is ignored when this flag is present (use `--clear` with
    /// any dummy value, e.g. `aeterna tenant switch none --clear`).
    #[arg(long)]
    pub clear: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct TenantCurrentArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantDomainMapArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Domain to map (e.g. acme.example.com)
    #[arg(long)]
    pub domain: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingShowArgs {
    /// Tenant slug or ID
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingSetArgs {
    /// Tenant slug or ID
    pub tenant: String,

    #[arg(long)]
    pub kind: String,

    /// Local path (for kind=local)
    #[arg(long)]
    pub local_path: Option<String>,

    /// Remote URL (for kind=remote)
    #[arg(long)]
    pub remote_url: Option<String>,

    /// Branch name
    #[arg(long)]
    pub branch: Option<String>,

    #[arg(long)]
    pub branch_policy: Option<String>,

    #[arg(long)]
    pub credential_kind: Option<String>,

    /// Credential reference (key name in secret store)
    #[arg(long)]
    pub credential_ref: Option<String>,

    /// GitHub organization owner (for kind=github)
    #[arg(long)]
    pub github_owner: Option<String>,

    /// GitHub repository name (for kind=github)
    #[arg(long)]
    pub github_repo: Option<String>,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Dry run – show what would be set without calling the server
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantRepoBindingValidateArgs {
    /// Tenant slug or ID
    pub tenant: String,

    #[arg(long)]
    pub kind: String,

    /// Local path (for kind=local)
    #[arg(long)]
    pub local_path: Option<String>,

    /// Remote URL (for kind=remote)
    #[arg(long)]
    pub remote_url: Option<String>,

    /// Branch name
    #[arg(long)]
    pub branch: Option<String>,

    #[arg(long)]
    pub branch_policy: Option<String>,

    #[arg(long)]
    pub credential_kind: Option<String>,

    /// Credential reference (key name in secret store)
    #[arg(long)]
    pub credential_ref: Option<String>,

    /// GitHub organization owner (for kind=github)
    #[arg(long)]
    pub github_owner: Option<String>,

    /// GitHub repository name (for kind=github)
    #[arg(long)]
    pub github_repo: Option<String>,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConfigInspectArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConfigUpsertArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub file: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct TenantConfigValidateArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    #[arg(long)]
    pub file: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantSecretSetArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    pub logical_name: String,

    /// **UNSAFE.** Inline secret value. Leaks into shell history,
    /// `ps`, and CI logs; only accepted when paired with
    /// `--allow-inline-secret`. Prefer `--from-stdin`, `--from-file`,
    /// `--from-env`, or `--ref` (B2 §8.1/§8.2).
    #[arg(long)]
    pub value: Option<String>,

    /// Opt-in escape hatch that unlocks `--value`. Required for tests
    /// and one-off debugging; never set this in CI scripts.
    #[arg(long)]
    pub allow_inline_secret: bool,

    /// Reference name of an already-stored secret; the CLI sends
    /// only the name (no bytes) to the server (B2 §8.1).
    #[arg(long = "ref", value_name = "NAME")]
    pub reference: Option<String>,

    /// Path to a file whose UTF-8 contents are the secret. File mode
    /// must be `0600` or stricter on Unix (B2 §8.1/§8.3).
    #[arg(long, value_name = "PATH")]
    pub from_file: Option<std::path::PathBuf>,

    /// Read the secret from stdin. On a TTY echo is disabled; when
    /// piped the bytes are read verbatim (B2 §8.1/§8.4).
    #[arg(long)]
    pub from_stdin: bool,

    /// Read the secret from the named environment variable; the
    /// variable is cleared from the current process after read so
    /// child processes cannot inherit it (B2 §8.1/§8.5).
    #[arg(long, value_name = "VAR")]
    pub from_env: Option<String>,

    #[arg(long, default_value = "tenant")]
    pub ownership: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

/// Args for `aeterna tenant validate`.
///
/// Thin wrapper around `POST /api/v1/admin/tenants/provision?dryRun=true`
/// (task §7.1). The command:
///
/// - Reads a `TenantManifest` JSON document from `--file` (use `-` for
///   stdin) and POSTs it with `dryRun=true` so no server state changes.
/// - Renders the `ProvisionPlan` response (status classifier + hash
///   pair + generation + per-section presence flags) when the manifest
///   is valid.
/// - Renders the `validationErrors` array on HTTP 422, one per line,
///   and exits non-zero so CI pipelines can gate on it.
///
/// This is the first CLI consumer of the provision-dry-run surface
/// shipped in B2 §2.3 — follow-ups `tenant plan` (§7.2) and
/// `tenant diff` (§7.3) will build on the same client helper.
#[derive(Args)]
pub struct TenantValidateArgs {
    /// Path to a JSON manifest file. Use `-` to read from stdin.
    #[arg(long)]
    pub file: String,

    /// Emit the raw JSON response (either the dry-run plan or the
    /// validation-errors body) instead of the human table.
    #[arg(long)]
    pub json: bool,
}

/// Args for `aeterna tenant render` (B2 §7.2).
///
/// Surfaces the server-side `GET /admin/tenants/{slug}/manifest`
/// endpoint to operators as a stable CLI entry point. Outputs the
/// rendered JSON manifest either to stdout or to a file.
///
/// Flags mirror the task spec verbatim:
/// - `--slug <slug>` — which tenant to render. Optional; when absent
///   we resolve through the same context-lookup path `tenant show`
///   uses (env → CLI flag → local context.toml → user default).
/// - `--redact` — pass `redact=true` to the server so secret
///   reference *names* are replaced with opaque placeholders and the
///   repository binding's `credentialRef` is elided. Plaintext is
///   never emitted regardless of this flag (the server has no access
///   to unwrapped values); `--redact` only hides operator-chosen
///   logical names from over-the-shoulder readers.
/// - `-o / --output <path>` — write the rendered manifest to a file
///   instead of stdout. Useful for piping into `aeterna tenant diff`
///   (§7.3) once that lands, or into git-tracked snapshots for
///   drift detection.
///
/// This CLI is the second consumer of the manifest-render surface
/// (after the in-server roundtrip in `ManifestGet`). It deliberately
/// has zero transformation logic of its own — the server is the
/// source of truth for the rendered shape, the CLI just serialises
/// it back out.
#[derive(Args)]
pub struct TenantRenderArgs {
    /// Tenant slug or ID to render. Falls back to the active
    /// context's tenant (env / flag / local / server default) when
    /// omitted, matching the resolution order used by `tenant show`.
    #[arg(long)]
    pub slug: Option<String>,

    /// Replace secret-reference *names* with opaque placeholders and
    /// elide the repository binding's `credentialRef`. Plaintext is
    /// never exposed regardless of this flag.
    #[arg(long)]
    pub redact: bool,

    /// Write the rendered manifest to this path instead of stdout.
    /// The file is created (or truncated) with the default umask;
    /// callers that need a mode guarantee should `umask 077`
    /// beforehand.
    #[arg(short = 'o', long = "output")]
    pub output: Option<std::path::PathBuf>,

    /// Target a specific tenant context (PlatformAdmin only — for
    /// cross-tenant operations). Kept for symmetry with `tenant show`
    /// / `tenant validate` so operators have one mental model across
    /// read-shaped commands.
    #[arg(long)]
    pub target_tenant: Option<String>,
}

/// Output format for `aeterna tenant diff` (§7.3).
///
/// `unified` is the default and produces a git-diff-style text view
/// (added/removed leaves colourless, one per line) that a human
/// operator can scan during a manifest review. `json` emits the raw
/// [`TenantDiff`][crate::server::tenant_diff::TenantDiff] wire
/// shape verbatim — useful for piping into `jq` or a CI gate that
/// enforces "no drift" by asserting `operation == "noop"`.
///
/// Kept explicit (no `short = 'o'` collision with `--output` path
/// flags elsewhere) because the format choice is orthogonal to where
/// output goes; diff always writes to stdout.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum TenantDiffFormat {
    /// Git-style unified text: one line per changed leaf with
    /// `+`/`-` prefixes and the dot-notation path.
    Unified,
    /// Raw `TenantDiff` JSON as emitted by the server. Stable wire
    /// shape — safe for scripts.
    Json,
}

/// Args for `aeterna tenant diff` (B2 §7.3).
///
/// Wraps `POST /api/v1/admin/tenants/diff` (B3 §2.4). The server
/// takes the candidate manifest, renders the tenant's current state,
/// walks both JSON trees in lockstep, and returns a `TenantDiff`
/// whose `operation` is `create` / `update` / `noop`. The slug is
/// taken from the manifest body (`manifest.tenant.slug`), not from a
/// CLI flag — mirroring the `tenant validate` contract — so editing
/// the manifest is the only way to change which tenant is targeted.
///
/// Exit codes:
/// - `0` on 200 with `operation == noop` (clean) OR when the diff
///   rendered successfully, regardless of operation. Scripts that
///   want to gate on "no drift" should parse the `operation` field
///   of `-o json` output rather than relying on exit code: a legit
///   `create`/`update` is not a CLI failure.
/// - Non-zero on server errors, HTTP 422 (manifest invalid), or
///   transport failures.
///
/// Rationale for NOT returning non-zero on `update`/`create`: the
/// CLI should be composable. `aeterna tenant diff -f x.json` is the
/// moral equivalent of `diff a b`, and `diff` exits 0 when it
/// succeeds — the *caller* decides whether "differences exist" is a
/// failure by inspecting the output.
#[derive(Args)]
pub struct TenantDiffArgs {
    /// Path to a candidate manifest JSON file. Use `-` to read from
    /// stdin. Identical semantics to `tenant validate --file`; a
    /// `tenant render -o foo.json | tenant diff -f -` pipeline is
    /// the intended drift-check shape.
    #[arg(short = 'f', long)]
    pub file: String,

    /// Output format. `unified` (default) is human-readable; `json`
    /// emits the structured `TenantDiff` response for scripting.
    #[arg(short = 'o', long = "output", value_enum, default_value_t = TenantDiffFormat::Unified)]
    pub output: TenantDiffFormat,

    /// Target a specific tenant context (PlatformAdmin only — for
    /// cross-tenant operations). Kept for symmetry with `tenant
    /// render` / `tenant validate`. The tenant being diffed is
    /// ALWAYS the one named in the manifest body; this flag only
    /// selects which active-tenant context the HTTP client uses
    /// (matters for audit attribution and admin-scope evaluation).
    #[arg(long)]
    pub target_tenant: Option<String>,
}

/// Args for `aeterna tenant apply` (B2 §7.1).
///
/// Real-apply wrapper around `POST /api/v1/admin/tenants/provision`
/// (no `dryRun` flag — this IS the write path). Companion to
/// `tenant validate` (dry-run preview) and `tenant render` /
/// `tenant diff` (read-shaped commands).
///
/// ## Safety model
///
/// `apply` is destructive: it writes to `tenants`, `tenant_configs`,
/// `organizational_units`, `user_roles`, `tenant_secrets`,
/// `tenant_repository_bindings`, and `tenant_domain_mappings` in
/// one transaction. Operator opt-in is enforced as follows:
///
/// 1. **Default (interactive TTY):** a preview is fetched via the
///    dry-run surface, the `ProvisionPlan` is displayed, and the
///    operator must type `yes` at a confirmation prompt before the
///    real apply fires. The prompt blocks on stdin; Ctrl-C aborts
///    cleanly (no partial write — dry-run did not mutate anything).
/// 2. **`--yes`:** the confirmation prompt is skipped. Still runs
///    the preview so the operator's terminal shows the plan, but
///    proceeds immediately afterwards. Required for non-TTY shells
///    (CI, pipes).
/// 3. **`--json` (always requires `--yes`):** fully unattended; no
///    preview text is printed. Renders the raw server response
///    JSON. Intended for CI gates and automation.
///
/// ## Race model
///
/// The preview's `currentGeneration` and the apply's
/// generation-guarded UPDATE are independent checks against
/// `tenants`. A concurrent apply between preview and confirm will
/// be rejected at the UPDATE stage with HTTP 409
/// `generation_conflict` — which we render as an actionable error,
/// not a crash. The preview is advisory; the only source of truth
/// is the guarded write.
///
/// ## `--allow-inline`
///
/// Appends `?allowInline=true` so the server will accept manifests
/// whose `secrets[].secretValue` carry plaintext. Only honoured when
/// the server also has `provisioning.allowInlineSecret = true`, and
/// that flag is permanently off in release builds. The CLI never
/// inspects the manifest for inline plaintext itself — we let the
/// server own that decision — but we expose the toggle here so dev
/// workflows do not have to drop to raw curl.
#[derive(Args)]
pub struct TenantApplyArgs {
    /// Path to a JSON manifest file. Use `-` to read from stdin.
    /// Identical semantics to `tenant validate --file`.
    #[arg(short = 'f', long)]
    pub file: String,

    /// Skip the interactive confirmation prompt. Required when
    /// stdin is not a TTY (CI, pipelines) or when `--json` is set.
    #[arg(long)]
    pub yes: bool,

    /// Emit the raw server JSON response instead of the human
    /// summary. Implies `--yes` must be set; the combination is the
    /// expected script shape. Errors (validation, conflict, partial
    /// apply) are also rendered as JSON so jq / CI gates can parse
    /// every terminal state uniformly.
    #[arg(long)]
    pub json: bool,

    /// Opt in to inline `secrets[].secretValue` plaintext on the
    /// wire. Server-side rejected unless
    /// `provisioning.allowInlineSecret = true` (dev builds only).
    /// Prefer `config.secretReferences` for real deployments.
    #[arg(long)]
    pub allow_inline: bool,

    /// Target a specific tenant context (PlatformAdmin cross-tenant
    /// operation). Does NOT override the manifest slug — the tenant
    /// being written is always the one named in `manifest.tenant.slug`.
    /// This flag only selects the active-tenant context the HTTP
    /// client uses, for audit attribution.
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Stream live lifecycle events (SSE) while the apply is in
    /// flight. Equivalent to running `aeterna tenant watch <slug>`
    /// in a second terminal during the apply, but co-scheduled so
    /// you never miss the opening `provisioning_step` frames.
    ///
    /// Events go to **stderr** (one line per frame, pretty form —
    /// or raw JSON when `--json` is also set). The apply's final
    /// response still goes to **stdout**, so `| jq` on the apply
    /// output keeps working. (B2 §7.6)
    #[arg(long)]
    pub watch: bool,

    /// Abort the apply if no lifecycle event arrives within this
    /// many seconds. Only meaningful together with `--watch`.
    ///
    /// Resets on every received frame — the wall-clock for the
    /// entire apply is unbounded, it's specifically *stalls* that
    /// trigger the bail. Designed for CI pipelines that tolerate a
    /// slow provisioner but want to fail fast when the provisioner
    /// wedges (e.g. the IAM step is waiting on an external service
    /// that's down).
    ///
    /// `0` (the default) means no timeout. Typical values: `30`
    /// for fast tests, `300` for real provisioning flows.
    /// (B2 §7.7)
    #[arg(long, default_value_t = 0, value_name = "SECS")]
    pub watch_timeout: u64,

    /// Continue streaming events **after** the apply HTTP response
    /// arrives, until a lifecycle event of the given kind is
    /// observed. Intended for async reconciliation flows where the
    /// apply merely *enqueues* work (e.g. background IAM sync) and
    /// the caller wants to block until that work completes.
    ///
    /// Accepted kinds: `provisioned`, `updated`, `deactivated`,
    /// `lagged`, or any `provisioning_step` kind name the server
    /// happens to emit. A leading `step:` prefix is also accepted
    /// (e.g. `--watch-until=step:iam_sync_complete`) so future
    /// per-step reconcilers remain nameable without CLI changes.
    ///
    /// Interactions:
    /// * Only meaningful with `--watch` (ignored otherwise).
    /// * Honours `--watch-timeout` — a stall during the post-apply
    ///   wait still aborts.
    /// * Unset (the default) preserves the prior §7.6 behaviour:
    ///   cancel the subscription immediately when the apply
    ///   round-trip returns.
    ///
    /// (B2 §7.8)
    #[arg(long, value_name = "EVENT")]
    pub watch_until: Option<String>,
}

#[derive(Args)]
pub struct TenantSecretDeleteArgs {
    #[arg(long)]
    pub tenant: Option<String>,

    pub logical_name: String,

    #[arg(long)]
    pub target_tenant: Option<String>,

    #[arg(long)]
    pub json: bool,
}

/// Args for `aeterna tenant watch <slug>` (B2 §7.5).
///
/// Thin client over the `/api/v1/admin/tenants/{slug}/events` SSE
/// endpoint. Streams one line per event to stdout and exits 0 when
/// the server closes the stream cleanly (shutdown) or the user sends
/// SIGINT / closes stdout.
#[derive(clap::Args, Debug)]
pub struct TenantWatchArgs {
    /// Slug of the tenant to watch.
    #[arg(value_name = "SLUG")]
    pub slug: String,

    /// Emit raw event JSON (one line per event) instead of the
    /// human-readable pretty form. Useful for piping into `jq`,
    /// feeding a progress bar, or composing with `tenant apply`.
    #[arg(long)]
    pub json: bool,

    /// Override the target-tenant header (matches the convention used
    /// by every other tenant subcommand). Does NOT change which
    /// tenant's events are streamed — that is always `<slug>` — but
    /// some auth paths key off this header.
    #[arg(long)]
    pub target_tenant: Option<String>,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub async fn run(cmd: TenantCommand) -> anyhow::Result<()> {
    match cmd {
        TenantCommand::Create(args) => run_create(args).await,
        TenantCommand::List(args) => run_list(args).await,
        TenantCommand::Show(args) => run_show(args).await,
        TenantCommand::Update(args) => run_update(args).await,
        TenantCommand::Deactivate(args) => run_deactivate(args).await,
        TenantCommand::Use(args) => run_use(args).await,
        TenantCommand::Switch(args) => run_switch(args).await,
        TenantCommand::Current(args) => run_current(args).await,
        TenantCommand::DomainMap(args) => run_domain_map(args).await,
        TenantCommand::RepoBinding(sub) => match sub {
            TenantRepoBindingCommand::Show(args) => run_repo_binding_show(args).await,
            TenantRepoBindingCommand::Set(args) => run_repo_binding_set(args).await,
            TenantRepoBindingCommand::Validate(args) => run_repo_binding_validate(args).await,
        },
        TenantCommand::Config(sub) => match sub {
            TenantConfigCommand::Inspect(args) => run_config_inspect(args).await,
            TenantConfigCommand::Upsert(args) => run_config_upsert(args).await,
            TenantConfigCommand::Validate(args) => run_config_validate(args).await,
        },
        TenantCommand::Secret(sub) => match sub {
            TenantSecretCommand::Set(args) => run_secret_set(args).await,
            TenantSecretCommand::Delete(args) => run_secret_delete(args).await,
        },
        TenantCommand::Connection(sub) => match sub {
            TenantConnectionCommand::List(args) => run_connection_list(args).await,
            TenantConnectionCommand::Grant(args) => run_connection_grant(args).await,
            TenantConnectionCommand::Revoke(args) => run_connection_revoke(args).await,
        },
        TenantCommand::Validate(args) => run_validate(args).await,
        TenantCommand::Render(args) => run_render(args).await,
        TenantCommand::Diff(args) => run_diff(args).await,
        TenantCommand::Apply(args) => run_apply(args).await,
        TenantCommand::Watch(args) => run_watch(args).await,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tenant_server_required(operation: &str, message: &str) -> anyhow::Result<()> {
    ux_error::UxError::new(message)
        .why("This tenant command requires a live control-plane backend")
        .fix("Start the Aeterna server: aeterna serve")
        .fix("Ensure AETERNA_SERVER_URL is set and the server is reachable")
        .suggest("aeterna admin health")
        .display();
    anyhow::bail!("Aeterna server not connected for operation: {operation}")
}

async fn get_live_client() -> Option<crate::client::AeternaClient> {
    get_live_client_for(None).await
}

async fn get_live_client_for(target_tenant: Option<&str>) -> Option<crate::client::AeternaClient> {
    let resolved = crate::profile::load_resolved(None, None);
    if let Ok(ref r) = resolved {
        let client = crate::client::AeternaClient::from_profile(r).await.ok()?;
        if let Some(tenant) = target_tenant {
            Some(client.with_target_tenant(tenant))
        } else {
            Some(client)
        }
    } else {
        None
    }
}

fn repo_binding_body(
    kind: &str,
    local_path: Option<&str>,
    remote_url: Option<&str>,
    branch: Option<&str>,
    branch_policy: Option<&str>,
    credential_kind: Option<&str>,
    credential_ref: Option<&str>,
    github_owner: Option<&str>,
    github_repo: Option<&str>,
) -> serde_json::Value {
    let mut body = json!({ "kind": kind, "sourceOwner": "admin" });
    if let Some(v) = local_path {
        body["localPath"] = json!(v);
    }
    if let Some(v) = remote_url {
        body["remoteUrl"] = json!(v);
    }
    if let Some(v) = branch {
        body["branch"] = json!(v);
    }
    if let Some(v) = branch_policy {
        body["branchPolicy"] = json!(v);
    }
    if let Some(v) = credential_kind {
        body["credentialKind"] = json!(v);
    }
    if let Some(v) = credential_ref {
        body["credentialRef"] = json!(v);
    }
    if let Some(v) = github_owner {
        body["githubOwner"] = json!(v);
    }
    if let Some(v) = github_repo {
        body["githubRepo"] = json!(v);
    }
    body
}

fn redact_secret_values(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map.iter_mut() {
                if key == "secretValue" || key == "secret_value" {
                    *nested = json!("[REDACTED]");
                } else {
                    redact_secret_values(nested);
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact_secret_values(item);
            }
        }
        _ => {}
    }
}

fn redacted_json(mut value: Value) -> Value {
    redact_secret_values(&mut value);
    value
}

fn tenant_config_ownership(ownership: &str) -> anyhow::Result<&'static str> {
    match ownership {
        "tenant" => Ok("tenant"),
        "platform" => Ok("platform"),
        _ => {
            ux_error::UxError::new(format!("Invalid ownership: '{ownership}'"))
                .why("Supported ownership values are: tenant, platform")
                .fix("Use --ownership tenant or --ownership platform")
                .display();
            anyhow::bail!("Invalid tenant config ownership")
        }
    }
}

fn read_json_file(path: &str) -> anyhow::Result<Value> {
    let raw = fs::read_to_string(path)?;
    let payload: Value =
        serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("Invalid JSON in '{path}': {e}"))?;
    Ok(payload)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn run_create(args: TenantCreateArgs) -> anyhow::Result<()> {
    if args.dry_run {
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_create",
                "tenant": { "slug": args.slug, "name": args.name },
                "nextSteps": [
                    "Run without --dry-run to create",
                    "Use 'aeterna tenant use <slug>' to set as default context"
                ]
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Create (Dry Run)");
            println!();
            println!("  Slug: {}", args.slug);
            println!("  Name: {}", args.name);
            println!();
            output::info("Dry run mode – tenant not created.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let body = json!({ "slug": args.slug, "name": args.name });
        let result = client.tenant_create(&body).await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Tenant Created");
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  Slug:   {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:   {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  ID:     {}",
                    t.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status: {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
            output::hint(
                "Use 'aeterna tenant use <slug>' to set this tenant as your default context",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_create"
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_create");
    }
    tenant_server_required(
        "tenant_create",
        "Cannot create tenant: server not connected",
    )
}

async fn run_list(args: TenantListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_list(args.include_inactive)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Tenants");
            println!();
            if let Some(tenants) = result["tenants"].as_array() {
                if tenants.is_empty() {
                    println!("  (no tenants found)");
                } else {
                    for t in tenants {
                        let slug = t["slug"].as_str().unwrap_or("?");
                        let name = t["name"].as_str().unwrap_or("?");
                        let status = t["status"].as_str().unwrap_or("?");
                        println!("  {slug:<24} {name:<32} [{status}]");
                    }
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_list"
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_list");
    }
    tenant_server_required("tenant_list", "Cannot list tenants: server not connected")
}

async fn run_show(args: TenantShowArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client.tenant_show(&args.tenant).await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Tenant: {}", args.tenant));
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  ID:      {}",
                    t.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Slug:    {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:    {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status:  {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Source:  {}",
                    t.get("sourceOwner").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_show",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_show");
    }
    tenant_server_required(
        "tenant_show",
        &format!("Cannot show tenant '{}': server not connected", args.tenant),
    )
}

async fn run_update(args: TenantUpdateArgs) -> anyhow::Result<()> {
    if args.dry_run {
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_update",
                "tenant": args.tenant,
                "changes": {
                    "slug": args.new_slug,
                    "name": args.name
                }
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Update (Dry Run)");
            println!();
            println!("  Tenant: {}", args.tenant);
            if let Some(ref s) = args.new_slug {
                println!("  New Slug: {s}");
            }
            if let Some(ref n) = args.name {
                println!("  New Name: {n}");
            }
            println!();
            output::info("Dry run mode – tenant not updated.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let mut body = json!({});
        if let Some(ref s) = args.new_slug {
            body["slug"] = json!(s);
        }
        if let Some(ref n) = args.name {
            body["name"] = json!(n);
        }
        let result = client
            .tenant_update(&args.tenant, &body)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Tenant Updated");
            println!();
            if let Some(t) = result["tenant"].as_object() {
                println!(
                    "  Slug:   {}",
                    t.get("slug").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Name:   {}",
                    t.get("name").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Status: {}",
                    t.get("status").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_update",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_update");
    }
    tenant_server_required(
        "tenant_update",
        &format!(
            "Cannot update tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_deactivate(args: TenantDeactivateArgs) -> anyhow::Result<()> {
    if !args.yes {
        eprintln!(
            "This will deactivate tenant '{}'. Use --yes to confirm.",
            args.tenant
        );
        eprintln!("Use --yes to skip this confirmation.");
        return Ok(());
    }

    if let Some(client) = get_live_client().await {
        let result = client
            .tenant_deactivate(&args.tenant)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Tenant Deactivated");
            println!();
            println!("  Tenant '{}' has been deactivated.", args.tenant);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_deactivate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_deactivate");
    }
    tenant_server_required(
        "tenant_deactivate",
        &format!(
            "Cannot deactivate tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_use(args: TenantUseArgs) -> anyhow::Result<()> {
    let _resolver = ContextResolver::new();

    let aeterna_dir = Path::new(".aeterna");
    let context_file = aeterna_dir.join("context.toml");

    let mut config = if context_file.exists() {
        let content = fs::read_to_string(&context_file)?;
        toml::from_str::<toml::Value>(&content)
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    if let Some(table) = config.as_table_mut() {
        table.insert(
            "tenant_id".to_string(),
            toml::Value::String(args.tenant.clone()),
        );
    }

    fs::create_dir_all(aeterna_dir)?;
    fs::write(&context_file, toml::to_string_pretty(&config)?)?;

    output::header("Set Default Tenant");
    println!();
    println!("  Setting default tenant: {}", args.tenant);
    println!();
    println!("  ✓ Updated .aeterna/context.toml");
    println!("  tenant_id = \"{}\"", args.tenant);

    Ok(())
}

// ---------------------------------------------------------------------------
// `aeterna tenant switch` / `aeterna tenant current` (#45)
//
// These commands wrap the server-side `/api/v1/user/me/default-tenant`
// endpoints (landed with the RequestContext resolver in #44.b). Unlike
// `tenant use` which is a local-only `.aeterna/context.toml` write, the
// switch/clear round-trip persists the preference in `users.default_tenant_id`
// so it follows the user across devices and sessions.
// ---------------------------------------------------------------------------

async fn run_switch(args: TenantSwitchArgs) -> anyhow::Result<()> {
    let Some(client) = get_live_client().await else {
        return tenant_server_required(
            "tenant switch",
            "The server-side default-tenant preference requires a connected control plane.",
        );
    };

    if args.clear {
        client.user_default_tenant_clear().await.inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .why("Failed to clear the server-side default tenant")
                    .fix("Confirm your session is active: aeterna auth status")
                    .display();
            }
        })?;

        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "success": true,
                    "action": "cleared",
                }))
                .unwrap()
            );
        } else {
            output::header("Default Tenant Cleared");
            println!();
            println!("  ✓ Server-side default preference removed");
            println!("  Subsequent requests will fall back to X-Tenant-ID header or auto-select");
        }
        return Ok(());
    }

    let resp = client
        .user_default_tenant_set(&args.tenant)
        .await
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "tenant": args.tenant, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .why("Failed to set the server-side default tenant")
                    .fix(format!(
                        "Verify you are a member of '{}' with: aeterna tenant list",
                        args.tenant
                    ))
                    .fix("Confirm your session is active: aeterna auth status")
                    .display();
            }
        })?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "success": true,
                "action": "switched",
                "tenant": resp,
            }))
            .unwrap()
        );
    } else {
        let slug = resp
            .get("slug")
            .and_then(Value::as_str)
            .unwrap_or(&args.tenant);
        let name = resp.get("name").and_then(Value::as_str).unwrap_or("");
        let id = resp.get("tenantId").and_then(Value::as_str).unwrap_or("");

        output::header("Switched Default Tenant");
        println!();
        println!("  Tenant: {slug}");
        if !name.is_empty() {
            println!("  Name:   {name}");
        }
        if !id.is_empty() {
            println!("  ID:     {id}");
        }
        println!();
        println!("  ✓ Preference persisted server-side");
        println!("  ✓ Subsequent requests without X-Tenant-ID will target this tenant");
    }

    Ok(())
}

async fn run_current(args: TenantCurrentArgs) -> anyhow::Result<()> {
    // Always read the local context file (best-effort) so we can show it
    // even when the server is unreachable.
    let local = read_local_context_tenant();

    let server_default = match get_live_client().await {
        Some(client) => match client.user_default_tenant_get().await {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::debug!("could not fetch server default tenant: {e}");
                None
            }
        },
        None => None,
    };

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "local": local,
                "server_default": server_default.clone().unwrap_or(None),
            }))
            .unwrap()
        );
        return Ok(());
    }

    output::header("Current Tenant Selection");
    println!();
    match server_default {
        Some(Some(v)) => {
            let slug = v.get("slug").and_then(Value::as_str).unwrap_or("?");
            let name = v.get("name").and_then(Value::as_str).unwrap_or("");
            println!(
                "  Server default: {slug}{}",
                if name.is_empty() {
                    String::new()
                } else {
                    format!(" ({name})")
                }
            );
        }
        Some(None) => {
            println!("  Server default: (none set)");
        }
        None => {
            println!("  Server default: (server unreachable — cannot determine)");
        }
    }
    match local {
        Some(t) => println!("  Local context:  {t}"),
        None => println!("  Local context:  (none set)"),
    }
    println!();
    println!("  Precedence on next request: X-Tenant-ID header > server default > auto-select");

    Ok(())
}

fn read_local_context_tenant() -> Option<String> {
    let path = Path::new(".aeterna").join("context.toml");
    let content = fs::read_to_string(path).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;
    value
        .get("tenant_id")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
}

async fn run_domain_map(args: TenantDomainMapArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let body = json!({ "domain": args.domain });
        let result = client
            .tenant_add_domain_mapping(&args.tenant, &body)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Domain Mapping Added");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Domain: {}", args.domain);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_domain_map",
            "tenant": args.tenant,
            "domain": args.domain
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_domain_map");
    }
    tenant_server_required(
        "tenant_domain_map",
        &format!(
            "Cannot add domain mapping for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_show(args: TenantRepoBindingShowArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_repo_binding_show(&args.tenant)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Repository Binding: {}", args.tenant));
            println!();
            if let Some(b) = result["binding"].as_object() {
                println!(
                    "  Kind:          {}",
                    b.get("kind").and_then(|v| v.as_str()).unwrap_or("?")
                );
                println!(
                    "  Branch:        {}",
                    b.get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(default)")
                );
                println!(
                    "  Branch Policy: {}",
                    b.get("branchPolicy")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                );
                println!(
                    "  Credential:    {}",
                    b.get("credentialKind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                );
            } else {
                println!("  (no binding configured)");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_show",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_show");
    }
    tenant_server_required(
        "tenant_repo_binding_show",
        &format!(
            "Cannot show repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_set(args: TenantRepoBindingSetArgs) -> anyhow::Result<()> {
    let valid_kinds = ["local", "gitRemote", "github"];
    if !valid_kinds.contains(&args.kind.as_str()) {
        ux_error::UxError::new(format!("Invalid repository kind: '{}'", args.kind))
            .why("Supported kinds are: local, gitRemote, github")
            .fix("Use --kind local, --kind gitRemote, or --kind github")
            .display();
        anyhow::bail!("Invalid repository kind");
    }

    if args.dry_run {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_repo_binding_set",
                "tenant": args.tenant,
                "binding": body
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Repository Binding Set (Dry Run)");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Kind:   {}", args.kind);
            if let Some(ref p) = args.local_path {
                println!("  Path:   {p}");
            }
            if let Some(ref u) = args.remote_url {
                println!("  URL:    {u}");
            }
            println!();
            output::info("Dry run mode – binding not set.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        let result = client
            .tenant_repo_binding_set(&args.tenant, &body)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Repository Binding Set");
            println!();
            println!("  Tenant: {}", args.tenant);
            println!("  Kind:   {}", args.kind);
            println!();
            output::hint(
                "Use 'aeterna tenant repo-binding validate <tenant>' to verify the binding",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_set",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_set");
    }
    tenant_server_required(
        "tenant_repo_binding_set",
        &format!(
            "Cannot set repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_repo_binding_validate(args: TenantRepoBindingValidateArgs) -> anyhow::Result<()> {
    let valid_kinds = ["local", "gitRemote", "github"];
    if !valid_kinds.contains(&args.kind.as_str()) {
        ux_error::UxError::new(format!("Invalid repository kind: '{}'", args.kind))
            .why("Supported kinds are: local, gitRemote, github")
            .fix("Use --kind local, --kind gitRemote, or --kind github")
            .display();
        anyhow::bail!("Invalid repository kind");
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let body = repo_binding_body(
            &args.kind,
            args.local_path.as_deref(),
            args.remote_url.as_deref(),
            args.branch.as_deref(),
            args.branch_policy.as_deref(),
            args.credential_kind.as_deref(),
            args.credential_ref.as_deref(),
            args.github_owner.as_deref(),
            args.github_repo.as_deref(),
        );
        let result = client
            .tenant_repo_binding_validate(&args.tenant, &body)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Repository Binding Validation");
            println!();
            println!("  Tenant: {}", args.tenant);
            let valid = result["valid"].as_bool().unwrap_or(false);
            let icon = if valid { "✓" } else { "✗" };
            println!(
                "  Result: {} {}",
                icon,
                if valid { "valid" } else { "invalid" }
            );
            if let Some(msg) = result["message"].as_str()
                && !msg.is_empty()
            {
                println!("  Detail: {msg}");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_repo_binding_validate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_repo_binding_validate");
    }
    tenant_server_required(
        "tenant_repo_binding_validate",
        &format!(
            "Cannot validate repo binding for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_config_inspect(args: TenantConfigInspectArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_inspect(tenant).await
        } else {
            client.my_tenant_config_inspect().await
        }
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            if let Some(config) = redacted["config"].as_object() {
                let field_count = config
                    .get("fields")
                    .and_then(|v| v.as_object())
                    .map_or(0, serde_json::Map::len);
                let secret_ref_count = config
                    .get("secretReferences")
                    .and_then(|v| v.as_object())
                    .map_or(0, serde_json::Map::len);
                println!("  Fields:            {field_count}");
                println!("  Secret References: {secret_ref_count}");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_inspect",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_inspect");
    }
    tenant_server_required(
        "tenant_config_inspect",
        "Cannot inspect tenant config: server not connected",
    )
}

async fn run_config_upsert(args: TenantConfigUpsertArgs) -> anyhow::Result<()> {
    let payload = read_json_file(&args.file)?;

    if args.dry_run {
        let redacted_payload = redacted_json(payload);
        if args.json {
            let out = json!({
                "dryRun": true,
                "operation": "tenant_config_upsert",
                "tenant": args.tenant,
                "payload": redacted_payload,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        } else {
            output::header("Tenant Config Upsert (Dry Run)");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            println!("  File:   {}", args.file);
            println!();
            output::info("Dry run mode – tenant config not updated.");
        }
        return Ok(());
    }

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_upsert(tenant, &payload).await
        } else {
            client.my_tenant_config_upsert(&payload).await
        }
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config Upserted");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            println!("  File:   {}", args.file);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_upsert",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_upsert");
    }
    tenant_server_required(
        "tenant_config_upsert",
        "Cannot upsert tenant config: server not connected",
    )
}

async fn run_config_validate(args: TenantConfigValidateArgs) -> anyhow::Result<()> {
    let payload = read_json_file(&args.file)?;

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client.tenant_config_validate(tenant, &payload).await
        } else {
            client.my_tenant_config_validate(&payload).await
        }
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Config Validation");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant: {tenant}");
            } else {
                println!("  Scope: current tenant context");
            }
            let valid = redacted["valid"].as_bool().unwrap_or(false);
            let icon = if valid { "✓" } else { "✗" };
            println!(
                "  Result: {} {}",
                icon,
                if valid { "valid" } else { "invalid" }
            );
            if let Some(msg) = redacted["message"].as_str()
                && !msg.is_empty()
            {
                println!("  Detail: {msg}");
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_config_validate",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_config_validate");
    }
    tenant_server_required(
        "tenant_config_validate",
        "Cannot validate tenant config: server not connected",
    )
}

async fn run_secret_set(args: TenantSecretSetArgs) -> anyhow::Result<()> {
    let ownership = tenant_config_ownership(args.ownership.as_str())?;

    // B2 §8.1\u2013§8.5: resolve whichever input mode the user picked
    // through the secret-input resolver. The real-IO seam is used in
    // production; unit tests in `secret_input::tests` exercise every
    // branch against a fake IO.
    let flags = crate::secret_input::SecretInputFlags {
        inline_value: args.value.clone(),
        allow_inline: args.allow_inline_secret,
        reference: args.reference.clone(),
        from_file: args.from_file.clone(),
        from_stdin: args.from_stdin,
        from_env: args.from_env.clone(),
    };
    let payload = match crate::secret_input::resolve(&flags, &mut crate::secret_input::RealSecretIo)
    {
        Ok(p) => p,
        Err(e) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.what.clone()})
                    )?
                );
            } else {
                e.display();
            }
            anyhow::bail!("invalid secret input: {}", e.what);
        }
    };

    // `Inline` \u2192 secretValue; `Reference` \u2192 secretRef. The two are
    // mutually exclusive on the wire so the server cannot confuse
    // \"user sent a raw secret named 'vault/foo'\" with \"user sent a
    // reference to 'vault/foo'\".
    let body = match &payload {
        crate::secret_input::SecretPayload::Inline(v) => json!({
            "ownership": ownership,
            "secretValue": v,
        }),
        crate::secret_input::SecretPayload::Reference(r) => json!({
            "ownership": ownership,
            "secretRef": r,
        }),
    };

    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client
                .tenant_secret_set(tenant, &args.logical_name, &body)
                .await
        } else {
            client.my_tenant_secret_set(&args.logical_name, &body).await
        }
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Secret Set");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant:       {tenant}");
            } else {
                println!("  Scope:        current tenant context");
            }
            println!("  Logical Name: {}", args.logical_name);
            println!("  Ownership:    {ownership}");
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_secret_set",
            "tenant": args.tenant,
            "logicalName": args.logical_name,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_secret_set");
    }
    tenant_server_required(
        "tenant_secret_set",
        "Cannot set tenant secret: server not connected",
    )
}

async fn run_secret_delete(args: TenantSecretDeleteArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = if let Some(ref tenant) = args.tenant {
            client
                .tenant_secret_delete(tenant, &args.logical_name)
                .await
        } else {
            client.my_tenant_secret_delete(&args.logical_name).await
        }
        .inspect_err(|e| {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &json!({"success": false, "error": e.to_string()})
                    )
                    .unwrap()
                );
            } else {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            }
        })?;

        let redacted = redacted_json(result);
        if args.json {
            println!("{}", serde_json::to_string_pretty(&redacted)?);
        } else {
            output::header("Tenant Secret Deleted");
            println!();
            if let Some(ref tenant) = args.tenant {
                println!("  Tenant:       {tenant}");
            } else {
                println!("  Scope:        current tenant context");
            }
            println!("  Logical Name: {}", args.logical_name);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_secret_delete",
            "tenant": args.tenant,
            "logicalName": args.logical_name,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_secret_delete");
    }
    tenant_server_required(
        "tenant_secret_delete",
        "Cannot delete tenant secret: server not connected",
    )
}

// ---------------------------------------------------------------------------
// connection sub-commands
// ---------------------------------------------------------------------------

#[derive(Subcommand)]
pub enum TenantConnectionCommand {
    #[command(
        about = "List Git provider connections visible to a tenant (PlatformAdmin or TenantAdmin)"
    )]
    List(TenantConnectionListArgs),

    #[command(about = "Grant a tenant visibility of a Git provider connection (PlatformAdmin)")]
    Grant(TenantConnectionGrantArgs),

    #[command(about = "Revoke a tenant's visibility of a Git provider connection (PlatformAdmin)")]
    Revoke(TenantConnectionRevokeArgs),
}

#[derive(Args)]
pub struct TenantConnectionListArgs {
    /// Tenant slug to list connections for
    pub tenant: String,

    /// Target a specific tenant context (PlatformAdmin only — for cross-tenant operations)
    #[arg(long)]
    pub target_tenant: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConnectionGrantArgs {
    /// Tenant slug to grant visibility to
    pub tenant: String,

    /// Git provider connection ID to grant
    #[arg(long)]
    pub connection: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct TenantConnectionRevokeArgs {
    /// Tenant slug to revoke visibility from
    pub tenant: String,

    /// Git provider connection ID to revoke
    #[arg(long)]
    pub connection: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

// ---------------------------------------------------------------------------
// connection handlers
// ---------------------------------------------------------------------------

async fn run_connection_list(args: TenantConnectionListArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await {
        let result = client
            .tenant_git_provider_connections_list(&args.tenant)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header(&format!("Git Provider Connections: {}", args.tenant));
            println!();
            if let Some(connections) = result["connections"].as_array() {
                if connections.is_empty() {
                    println!("  (no connections visible to this tenant)");
                } else {
                    for c in connections {
                        let id = c["id"].as_str().unwrap_or("?");
                        let name = c["name"].as_str().unwrap_or("?");
                        let kind = c["providerKind"].as_str().unwrap_or("?");
                        println!("  {id:<32} {name:<32} [{kind}]");
                    }
                }
            }
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_list",
            "tenant": args.tenant
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_list");
    }
    tenant_server_required(
        "connection_list",
        &format!(
            "Cannot list connections for tenant '{}': server not connected",
            args.tenant
        ),
    )
}

async fn run_connection_grant(args: TenantConnectionGrantArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .git_provider_connection_grant_tenant(&args.connection, &args.tenant)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Connection Granted");
            println!();
            println!("  Tenant:     {}", args.tenant);
            println!("  Connection: {}", args.connection);
            println!();
            output::hint(
                "Use 'aeterna tenant connection list <tenant>' to verify the connection is visible",
            );
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_grant",
            "tenant": args.tenant,
            "connection": args.connection
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_grant");
    }
    tenant_server_required(
        "connection_grant",
        "Cannot grant connection: server not connected",
    )
}

async fn run_connection_revoke(args: TenantConnectionRevokeArgs) -> anyhow::Result<()> {
    if let Some(client) = get_live_client().await {
        let result = client
            .git_provider_connection_revoke_tenant(&args.connection, &args.tenant)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"success": false, "error": e.to_string()})
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;

        if args.json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else {
            output::header("Connection Revoked");
            println!();
            println!("  Tenant:     {}", args.tenant);
            println!("  Connection: {}", args.connection);
            println!();
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "connection_revoke",
            "tenant": args.tenant,
            "connection": args.connection
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: connection_revoke");
    }
    tenant_server_required(
        "connection_revoke",
        "Cannot revoke connection: server not connected",
    )
}

// ---------------------------------------------------------------------------
// tenant validate (§7.1)
// ---------------------------------------------------------------------------

/// Read the manifest body either from a path or from stdin when `file == "-"`.
///
/// Stdin support exists so operators can pipe `cat manifest.json | aeterna
/// tenant validate --file -` (matching how `kubectl apply -f -` works);
/// CI pipelines that compose manifests in-memory would otherwise have
/// to materialise a temp file just to feed the CLI.
fn read_manifest_input(file: &str) -> anyhow::Result<Value> {
    if file == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| anyhow::anyhow!("Failed to read manifest from stdin: {e}"))?;
        let payload: Value = serde_json::from_str(&buf)
            .map_err(|e| anyhow::anyhow!("Invalid JSON on stdin: {e}"))?;
        Ok(payload)
    } else {
        read_json_file(file)
    }
}

/// Render a successful dry-run `ProvisionPlan` body as a human-readable
/// table. Kept non-async and Value-typed so the render logic stays
/// trivially unit-testable without standing up a server.
fn render_provision_plan(plan: &Value) {
    output::header("Tenant Manifest Validation");
    println!();
    println!("  Result: ✓ valid");
    if let Some(slug) = plan.get("slug").and_then(|v| v.as_str()) {
        println!("  Slug:   {slug}");
    }
    if let Some(status) = plan.get("status").and_then(|v| v.as_str()) {
        // `status` is one of `unchanged` / `create` / `update` — all
        // three are legitimate validate outcomes; we surface which
        // pipeline a non-dry-run apply WOULD take so the operator
        // knows whether they are editing an existing tenant or
        // creating a new one.
        println!("  Action: {status} (what a real apply would do)");
    }
    if let Some(incoming) = plan.get("incomingHash").and_then(|v| v.as_str()) {
        println!("  Incoming hash: {incoming}");
    }
    match plan.get("currentHash") {
        Some(v) if v.is_null() => println!("  Current hash:  (none — first apply)"),
        Some(v) => {
            if let Some(s) = v.as_str() {
                println!("  Current hash:  {s}");
            }
        }
        None => {}
    }
    if let (Some(cur), Some(next)) = (
        plan.get("currentGeneration").and_then(|v| v.as_i64()),
        plan.get("nextGeneration").and_then(|v| v.as_i64()),
    ) {
        println!("  Generation:    {cur} → {next}");
    }
    println!();
    println!("  Sections present:");
    let section = |key: &str, label: &str| {
        let v = plan.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        let icon = if v { "✓" } else { "·" };
        println!("    {icon} {label}");
    };
    section("hasRepositoryBinding", "repositoryBinding");
    section("hasDomainMappings", "domainMappings");
    section("hasHierarchy", "hierarchy");
    section("hasRoles", "roles");
    section("hasProviders", "providers");
    if let Some(fields) = plan.get("configFieldCount").and_then(|v| v.as_u64()) {
        println!("    · config.fields: {fields}");
    }
    if let Some(refs) = plan.get("secretReferenceCount").and_then(|v| v.as_u64()) {
        println!("    · config.secretReferences: {refs}");
    }
    println!();
    output::hint("Re-run without --dry-run (via `aeterna tenant apply`) once available to apply.");
}

/// Render an HTTP 422 `manifest_validation_failed` body by listing every
/// string in `validationErrors`. Returns `true` when errors were
/// rendered so the caller can propagate a non-zero exit code.
fn render_validation_errors(body: &Value) -> bool {
    let errors: Vec<&str> = body
        .get("validationErrors")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    output::header("Tenant Manifest Validation");
    println!();
    println!("  Result: ✗ invalid");
    if errors.is_empty() {
        if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
            println!("  Error: {err}");
        }
        println!();
        return true;
    }
    println!("  {} error(s):", errors.len());
    for e in &errors {
        println!("    • {e}");
    }
    println!();
    true
}

async fn run_validate(args: TenantValidateArgs) -> anyhow::Result<()> {
    let manifest = read_manifest_input(&args.file)?;

    if let Some(client) = get_live_client().await {
        let body = client
            .tenant_provision_dry_run(&manifest)
            .await
            .inspect_err(|e| {
                if args.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({ "success": false, "error": e.to_string() })
                        )
                        .unwrap()
                    );
                } else {
                    ux_error::UxError::new(e.to_string())
                        .fix("Run: aeterna auth login")
                        .display();
                }
            })?;

        // `tenant_provision_dry_run` returns Ok on both 200 (plan) and
        // 422 (validation errors). The two cases are distinguished by
        // the top-level `success` field the server always sets.
        let is_valid = body
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if args.json {
            println!("{}", serde_json::to_string_pretty(&body)?);
        } else if is_valid {
            render_provision_plan(&body);
        } else {
            render_validation_errors(&body);
        }

        if !is_valid {
            // Non-zero exit so CI gates on validation.
            anyhow::bail!("tenant manifest is invalid");
        }
        return Ok(());
    }

    if args.json {
        let out = json!({
            "success": false,
            "error": "server_not_connected",
            "operation": "tenant_validate",
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
        anyhow::bail!("Aeterna server not connected for operation: tenant_validate");
    }
    tenant_server_required(
        "tenant_validate",
        "Cannot validate tenant manifest: server not connected",
    )
}

// ---------------------------------------------------------------------------
// tenant render (§7.2)
// ---------------------------------------------------------------------------

/// Resolve the tenant slug to render against.
///
/// `--slug` is the explicit override and wins unconditionally. When
/// absent we fall back to the CLI's active context — `get_live_client`
/// has already consulted env / `.aeterna/context.toml` / server
/// defaults, so we ask the user. This intentionally mirrors the
/// resolution order `tenant show` uses so the two commands feel
/// identical from an operator standpoint.
///
/// Returns `None` when neither `--slug` nor an active context tenant
/// is available; the caller surfaces this as a user-facing error
/// rather than silently rendering nothing.
fn resolve_render_slug(args_slug: Option<&str>) -> Option<String> {
    if let Some(s) = args_slug {
        return Some(s.to_string());
    }
    // Active-context lookup — identical shape to other read commands
    // (`get_live_client_for` runs the same `load_resolved(None, None)`
    // call, so reading `tenant_id` off the resolved config picks up
    // the same value the HTTP client uses).
    crate::profile::load_resolved(None, None)
        .ok()
        .and_then(|cfg| cfg.tenant_id)
}

/// Serialise the rendered manifest `Value` as pretty JSON. Factored
/// out so unit tests can lock the byte shape without spinning up an
/// HTTP server.
fn serialize_rendered_manifest(manifest: &serde_json::Value) -> anyhow::Result<String> {
    // `to_string_pretty` emits LF-only line endings and no trailing
    // newline. We add a trailing newline so the file is POSIX-text-
    // file-compliant — `cat | diff` and `git` both prefer trailing
    // newlines, and the cost is one byte.
    let mut s = serde_json::to_string_pretty(manifest)
        .map_err(|e| anyhow::anyhow!("Failed to serialize manifest: {e}"))?;
    s.push('\n');
    Ok(s)
}

async fn run_render(args: TenantRenderArgs) -> anyhow::Result<()> {
    // §7.2 slug resolution — explicit flag wins, then fall back to
    // active context. Fail loudly when neither is available rather
    // than rendering against an arbitrary default.
    let slug = resolve_render_slug(args.slug.as_deref()).ok_or_else(|| {
        anyhow::anyhow!(
            "No tenant specified. Pass --slug <slug> or set an active tenant with `aeterna tenant use <slug>`."
        )
    })?;

    let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await else {
        // `get_live_client_for` returns None only when the user is not
        // logged in; the helper has already surfaced a UX-friendly
        // error. Propagate a non-zero exit code so CI pipelines fail.
        anyhow::bail!("Not logged in — run `aeterna auth login` first.");
    };

    let manifest = client
        .tenant_manifest(&slug, args.redact)
        .await
        .inspect_err(|e| {
            // We deliberately do not emit JSON on failure here — unlike
            // `tenant validate`, the render command's happy path is
            // always pure-JSON output (to stdout or to `-o`), so an
            // error dressed as JSON would be indistinguishable from a
            // successful render at the shell level. Keep errors on
            // stderr via the UxError renderer.
            ux_error::UxError::new(e.to_string())
                .fix("Run: aeterna auth login")
                .display();
        })?;

    let rendered = serialize_rendered_manifest(&manifest)?;

    match args.output.as_deref() {
        Some(path) => {
            // Write atomically — create (or truncate) the target path.
            // We do NOT stage-via-temp-and-rename here because the
            // render endpoint is idempotent and safe to re-run; a
            // partial write is recoverable by rerunning the command.
            std::fs::write(path, &rendered).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to write rendered manifest to {}: {e}",
                    path.display()
                )
            })?;
            output::success(&format!(
                "Rendered manifest for tenant '{slug}' → {}",
                path.display()
            ));
        }
        None => {
            // Raw JSON to stdout — `print!` (no trailing newline
            // injection) because `serialize_rendered_manifest` already
            // added one. This keeps the byte output identical to a
            // file written via `-o`, so `aeterna tenant render --slug X
            // > x.json` and `aeterna tenant render --slug X -o x.json`
            // produce identical files.
            print!("{rendered}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// tenant apply (§7.1)
// ---------------------------------------------------------------------------

/// Terminal classification of a tenant apply response.
///
/// Derived from the server's `status` string + HTTP code. Used by the
/// renderer to pick icons / colours and by `run_apply` to pick the
/// exit code. Isolated as an enum (rather than branching on strings
/// inline) so unit tests can cover the classifier independently.
#[derive(Debug, PartialEq, Eq)]
enum ApplyOutcome {
    /// `status == "applied"`, every step OK. HTTP 200.
    Applied,
    /// `status == "unchanged"`, no-op re-apply. HTTP 200, steps=[].
    Unchanged,
    /// `status == "partial"`, HTTP 207 Multi-Status. Some steps
    /// failed; tenant row exists but downstream state is half-applied.
    Partial,
    /// HTTP 409 `generation_conflict` — strict-monotonic gate rejected
    /// the caller's `metadata.generation`.
    GenerationConflict,
    /// HTTP 422 `manifest_validation_failed` — `validate_manifest`
    /// returned errors before any write.
    ValidationFailed,
    /// HTTP 422 `inline_secret_not_allowed` — the server or caller
    /// does not permit inline plaintext.
    InlineSecretRejected,
    /// Anything the classifier does not recognise. Renders the raw
    /// body so operators see what they got rather than an opaque
    /// "unknown" line.
    Other,
}

fn classify_apply_response(body: &Value) -> ApplyOutcome {
    let success = body
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = body.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let error = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
    match (success, status, error) {
        (true, "applied", _) => ApplyOutcome::Applied,
        (true, "unchanged", _) => ApplyOutcome::Unchanged,
        (false, "partial", _) => ApplyOutcome::Partial,
        (false, _, "generation_conflict") => ApplyOutcome::GenerationConflict,
        (false, _, "manifest_validation_failed") => ApplyOutcome::ValidationFailed,
        (false, _, "manifest_parse_failed") => ApplyOutcome::ValidationFailed,
        (false, _, "inline_secret_not_allowed") => ApplyOutcome::InlineSecretRejected,
        _ => ApplyOutcome::Other,
    }
}

/// Render an applied / unchanged / partial response as a human
/// summary. Factored out of `run_apply` so the byte shape is
/// unit-testable without a live server.
fn render_apply_result(body: &Value, outcome: &ApplyOutcome) -> String {
    let mut out = String::new();
    let slug = body.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
    let hash = body.get("hash").and_then(|v| v.as_str()).unwrap_or("?");
    let generation = body
        .get("generation")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);

    let (icon, label) = match outcome {
        ApplyOutcome::Applied => ("✓", "applied"),
        ApplyOutcome::Unchanged => ("·", "unchanged (no-op)"),
        ApplyOutcome::Partial => ("⚠", "partial"),
        _ => ("?", "?"),
    };
    out.push_str(&format!("Tenant apply: {slug}\n"));
    out.push_str(&format!("Result:       {icon} {label}\n"));
    out.push_str(&format!("Hash:         {hash}\n"));
    if generation >= 0 {
        out.push_str(&format!("Generation:   {generation}\n"));
    }

    let empty = Vec::new();
    let steps = body
        .get("steps")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    if !steps.is_empty() {
        out.push('\n');
        out.push_str("Steps:\n");
        for step in steps {
            let name = step.get("step").and_then(|v| v.as_str()).unwrap_or("?");
            let ok = step.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            let detail = step.get("detail").and_then(|v| v.as_str());
            let err = step.get("error").and_then(|v| v.as_str());
            let icon = if ok { "✓" } else { "✗" };
            match (ok, detail, err) {
                (true, Some(d), _) => {
                    out.push_str(&format!("  {icon} {name}: {d}\n"));
                }
                (true, None, _) => {
                    out.push_str(&format!("  {icon} {name}\n"));
                }
                (false, _, Some(e)) => {
                    out.push_str(&format!("  {icon} {name}: {e}\n"));
                }
                (false, _, None) => {
                    out.push_str(&format!("  {icon} {name}: (no error message)\n"));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// tenant diff (§7.3)
// ---------------------------------------------------------------------------

/// Render a 200 `TenantDiff` response as git-style unified text.
///
/// Format:
///
/// ```text
/// Tenant diff: <slug>
/// Operation:   create|update|noop
/// Summary:     +<A> -<R> ~<M> (sections: a, b, c)
///
/// - <path>: <before-value>
/// + <path>: <after-value>
/// ~ <path>: <before> → <after>
/// ```
///
/// Factored out of `run_diff` so unit tests can lock the byte shape
/// without a live server. Kept `Value`-typed because the function
/// does not need the typed [`TenantDiff`] struct — walking the JSON
/// tree keeps the CLI forward-compatible with additive server
/// fields (new `changeKind` variants would render as an unknown
/// prefix rather than failing to deserialise).
fn render_diff_unified(diff: &Value) -> String {
    let mut out = String::new();
    let slug = diff.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
    let op = diff
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    out.push_str(&format!("Tenant diff: {slug}\n"));
    out.push_str(&format!("Operation:   {op}\n"));

    let summary = diff.get("summary");
    let added = summary
        .and_then(|s| s.get("added"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let removed = summary
        .and_then(|s| s.get("removed"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let modified = summary
        .and_then(|s| s.get("modified"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let sections: Vec<&str> = summary
        .and_then(|s| s.get("changedSections"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let sections_str = if sections.is_empty() {
        "none".to_string()
    } else {
        sections.join(", ")
    };
    out.push_str(&format!(
        "Summary:     +{added} -{removed} ~{modified} (sections: {sections_str})\n"
    ));

    if op == "noop" {
        out.push_str("\n  (no changes — a re-apply would be a no-op)\n");
        return out;
    }

    out.push('\n');
    let empty = Vec::new();
    let changes = diff
        .get("changes")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    for change in changes {
        let path = change.get("path").and_then(|v| v.as_str()).unwrap_or("?");
        let kind = change.get("kind").and_then(|v| v.as_str()).unwrap_or("?");
        // `compact_value` keeps the line single-row for primitives
        // and short arrays/objects; multi-line JSON gets folded onto
        // one line with spacing collapsed. Operators reviewing diffs
        // scan vertically by path — wrapping blobs across lines
        // defeats that pattern.
        let before = change.get("before").map(compact_value);
        let after = change.get("after").map(compact_value);
        match kind {
            "added" => {
                out.push_str(&format!(
                    "+ {path}: {}\n",
                    after.as_deref().unwrap_or("(null)")
                ));
            }
            "removed" => {
                out.push_str(&format!(
                    "- {path}: {}\n",
                    before.as_deref().unwrap_or("(null)")
                ));
            }
            "modified" => {
                out.push_str(&format!(
                    "~ {path}: {} → {}\n",
                    before.as_deref().unwrap_or("(null)"),
                    after.as_deref().unwrap_or("(null)"),
                ));
            }
            other => {
                // Forward-compat: unknown kind → show both sides.
                out.push_str(&format!(
                    "? {path} [{other}]: before={} after={}\n",
                    before.as_deref().unwrap_or("(null)"),
                    after.as_deref().unwrap_or("(null)"),
                ));
            }
        }
    }
    out
}

/// Render a 409 `generation_conflict` body as an actionable error.
fn render_generation_conflict(body: &Value) -> String {
    let current = body
        .get("currentGeneration")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    let submitted = body
        .get("submittedGeneration")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    let hint = body.get("hint").and_then(|v| v.as_str()).unwrap_or("");
    let mut out = String::from("Tenant apply: ✗ generation_conflict\n");
    out.push_str(&format!("  current:    {current}\n"));
    out.push_str(&format!("  submitted:  {submitted}\n"));
    if !hint.is_empty() {
        out.push_str(&format!("  hint:       {hint}\n"));
    }
    out
}

/// Render the `inline_secret_not_allowed` error body.
fn render_inline_secret_rejected(body: &Value) -> String {
    let mut out = String::from("Tenant apply: ✗ inline_secret_not_allowed\n");
    let empty = Vec::new();
    let offending = body
        .get("offendingSecrets")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    if !offending.is_empty() {
        out.push_str("  Offending secrets (logical names):\n");
        for name in offending {
            if let Some(s) = name.as_str() {
                out.push_str(&format!("    • {s}\n"));
            }
        }
    }
    if let Some(msg) = body.get("message").and_then(|v| v.as_str()) {
        out.push('\n');
        out.push_str(&format!("  {msg}\n"));
    }
    out
}

/// Prompt the operator on stdin. Returns `true` if the operator
/// typed `y` or `yes` (case-insensitive). Any other input — including
/// EOF / Ctrl-D — returns `false`. On non-TTY stdin we refuse to
/// prompt and force the caller to pass `--yes`; the caller-side check
/// happens in `run_apply` before we reach this function.
fn prompt_yes_no(question: &str) -> bool {
    use std::io::{BufRead, Write};
    print!("{question} [y/N]: ");
    // Best-effort flush; if stdout is detached the prompt is lost but
    // we still read stdin, so we degrade to "no" on whitespace.
    let _ = std::io::stdout().flush();
    let stdin = std::io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(0) => false, // EOF → no
        Ok(_) => {
            let trimmed = line.trim().to_ascii_lowercase();
            trimmed == "y" || trimmed == "yes"
        }
        Err(_) => false,
    }
}

async fn run_apply(args: TenantApplyArgs) -> anyhow::Result<()> {
    // `--json` is a script-shape flag — forcing `--yes` alongside
    // keeps the CLI from ever prompting in JSON mode, which would
    // interleave prompt text with machine-readable output.
    if args.json && !args.yes {
        anyhow::bail!("--json requires --yes (pass both to run unattended)");
    }

    let manifest = read_manifest_input(&args.file)?;

    let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await else {
        anyhow::bail!("Not logged in — run `aeterna auth login` first.");
    };

    // Step 1: preview via dry-run unless the caller asked for raw
    // JSON. The preview is advisory — an operator staring at their
    // terminal wants to see "this would UPDATE acme, gen 5→6" before
    // typing y. In JSON mode the operator is a script that does not
    // care about the preview, so we skip the extra round-trip.
    if !args.json {
        let preview = client
            .tenant_provision_dry_run(&manifest)
            .await
            .inspect_err(|e| {
                ux_error::UxError::new(e.to_string())
                    .fix("Run: aeterna auth login")
                    .display();
            })?;
        let preview_ok = preview
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !preview_ok {
            // Invalid manifest — render the validation errors and
            // bail before the real apply even ships. No prompt.
            render_validation_errors(&preview);
            anyhow::bail!("tenant manifest is invalid (preview rejected)");
        }
        render_provision_plan(&preview);

        // Short-circuit: dry-run says nothing would change. No need
        // to prompt or apply — the write would audit a
        // `tenant_provision_unchanged` event and nothing else.
        if preview.get("status").and_then(|v| v.as_str()) == Some("unchanged") {
            output::success("Nothing to apply — manifest is already in effect.");
            return Ok(());
        }

        if !args.yes {
            let slug = preview
                .get("slug")
                .and_then(|v| v.as_str())
                .unwrap_or("(unknown)");
            let action = preview
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("apply");
            let q = format!("Proceed with {action} for tenant '{slug}'?");
            if !prompt_yes_no(&q) {
                output::hint("Aborted — no changes were made.");
                anyhow::bail!("aborted by user");
            }
        }
    }

    // Step 2: real apply.
    //
    // When `--watch` is set, open an SSE subscription to the tenant
    // before the write call so we don't miss the opening
    // `provisioning_step` frames. The subscriber drains to stderr in
    // the background and is cancelled once the apply round-trip
    // returns (success or error), letting the final stdout render
    // remain the single source of scriptable truth.
    let watch_handle = if args.watch {
        let slug = manifest
            .get("tenant")
            .and_then(|t| t.get("slug"))
            .and_then(|s| s.as_str())
            .map(std::string::ToString::to_string);
        match slug {
            Some(slug) => {
                let client_clone = client.clone();
                let json_mode = args.json;
                let stall_timeout = if args.watch_timeout > 0 {
                    Some(std::time::Duration::from_secs(args.watch_timeout))
                } else {
                    None
                };
                let until_event = args.watch_until.clone();
                let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
                let task = tokio::spawn(async move {
                    // Non-fatal at the subscription layer: if the
                    // stream fails to open (server returned 404
                    // because the tenant doesn't exist yet on first
                    // create, or the caller lacks the read scope),
                    // log and move on — the apply itself still runs
                    // and the operator gets the final response the
                    // usual way.
                    //
                    // EXCEPT: a `watch_stall` error (from the stall
                    // timeout arm) is re-raised to the foreground
                    // as a fatal — the whole point of `--watch-timeout`
                    // is to abort the apply when the stream wedges.
                    // We signal this by returning the error, which
                    // the main task inspects after cancellation.
                    match stream_tenant_events(
                        &client_clone,
                        &slug,
                        json_mode,
                        FrameSink::Stderr,
                        cancel_rx,
                        stall_timeout,
                        until_event.as_deref(),
                    )
                    .await
                    {
                        Ok(()) => None,
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.starts_with("watch_stall:") {
                                eprintln!("{msg}");
                                Some(e)
                            } else {
                                eprintln!("warning: watch stream ended: {e}");
                                None
                            }
                        }
                    }
                });
                // Give the server a tick to register the subscriber
                // on the broadcast channel. The tenant_pubsub layer
                // buffers up to ~16 events per subscriber, so a
                // missed window is unlikely — but this 250ms pause
                // makes the race impossible on a loaded machine.
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                Some((cancel_tx, task))
            }
            None => {
                // Shouldn't happen — preview would have caught a
                // missing slug — but be defensive so `--watch`
                // never panics on a malformed manifest.
                eprintln!(
                    "warning: --watch skipped: manifest has no `tenant.slug` to subscribe to"
                );
                None
            }
        }
    } else {
        None
    };

    // Race the apply against the watch task. Semantics:
    //
    // * Apply finishes first  → cancel the watch task, drain it
    //                           briefly to flush buffered frames,
    //                           proceed with the normal result path.
    // * Watch task returns    → either the stream EOF'd (Ok(None))
    //                           or the stall-timeout fired
    //                           (Ok(Some(err))).  On EOF keep
    //                           waiting on apply; on stall, drop
    //                           the apply future (tokio::select!
    //                           drops the losing arm) and bail with
    //                           the stall error.
    //
    // When `--watch` is not set, `watch_handle` is `None` and we
    // just await the apply straight — the existing 957-test path.
    // Semantics of the race:
    //
    // * Apply finishes first → behaviour forks on `--watch-until`:
    //     - Unset: cancel the watch task, 200 ms flush, continue.
    //       (§7.6 behaviour, preserved exactly.)
    //     - Set:   DO NOT cancel — instead await the task, which
    //              now returns on target-event match, stall, or EOF.
    //              Target match = success; stall = fatal.
    //       (§7.8 behaviour, new.)
    // * Watch task returns first →
    //     - Stall (`Some(err)`): abort the apply with the stall
    //       error (tokio::select! drops the losing arm).
    //     - EOF (`None`) mid-apply: unusual but non-fatal; await
    //       the apply normally and surface its result.
    //     - JoinError (panic): warn + await apply anyway.
    let apply_result = if let Some((cancel_tx, task)) = watch_handle {
        let apply_fut = client.tenant_apply(&manifest, args.allow_inline);
        tokio::pin!(apply_fut);
        tokio::pin!(task);

        tokio::select! {
            res = &mut apply_fut => {
                if args.watch_until.is_some() {
                    // §7.8 — keep streaming until the target event
                    // arrives (or the stall-timer fires, or the
                    // server closes the stream). The apply HTTP
                    // response is already in hand; we're just
                    // waiting for the reconciler to settle.
                    //
                    // Note on JSON mode: the final apply response
                    // still needs to hit stdout eventually. We
                    // defer that until AFTER the task finishes so
                    // the SSE trail on stderr precedes the stdout
                    // summary — matching what a human reader
                    // expects.
                    match (&mut task).await {
                        Ok(Some(stall_err)) => return Err(stall_err),
                        Ok(None) => { /* target matched or EOF */ }
                        Err(je) => {
                            eprintln!("warning: watch task panicked: {je}");
                        }
                    }
                    // cancel_tx drops here — harmless, task is done.
                    drop(cancel_tx);
                } else {
                    // §7.6 — cancel + 200 ms flush.
                    let _ = cancel_tx.send(());
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_millis(200),
                        task,
                    ).await;
                }
                res
            }
            task_res = &mut task => {
                match task_res {
                    Ok(Some(stall_err)) => return Err(stall_err),
                    Ok(None) => {
                        drop(cancel_tx);
                        apply_fut.await
                    }
                    Err(je) => {
                        eprintln!("warning: watch task panicked: {je}");
                        drop(cancel_tx);
                        apply_fut.await
                    }
                }
            }
        }
    } else {
        client.tenant_apply(&manifest, args.allow_inline).await
    };

    let body = apply_result.inspect_err(|e| {
        if args.json {
            // Surface transport / auth errors as JSON too so the
            // scripted consumer sees one shape regardless of
            // where the failure occurred.
            let out = json!({
                "success": false,
                "error": "transport_failure",
                "details": e.to_string(),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        } else {
            ux_error::UxError::new(e.to_string())
                .fix("Run: aeterna auth login")
                .display();
        }
    })?;

    let outcome = classify_apply_response(&body);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&body)?);
        return match outcome {
            ApplyOutcome::Applied | ApplyOutcome::Unchanged => Ok(()),
            _ => anyhow::bail!("tenant apply did not succeed: {:?}", outcome),
        };
    }

    match outcome {
        ApplyOutcome::Applied | ApplyOutcome::Unchanged => {
            print!("{}", render_apply_result(&body, &outcome));
            Ok(())
        }
        ApplyOutcome::Partial => {
            print!("{}", render_apply_result(&body, &outcome));
            anyhow::bail!("tenant apply completed with step failures — see output")
        }
        ApplyOutcome::GenerationConflict => {
            print!("{}", render_generation_conflict(&body));
            anyhow::bail!("tenant apply rejected: generation_conflict")
        }
        ApplyOutcome::ValidationFailed => {
            render_validation_errors(&body);
            anyhow::bail!("tenant manifest is invalid")
        }
        ApplyOutcome::InlineSecretRejected => {
            print!("{}", render_inline_secret_rejected(&body));
            anyhow::bail!("tenant apply rejected: inline_secret_not_allowed")
        }
        ApplyOutcome::Other => {
            // Surface the raw body so the operator isn't left in the
            // dark when the server responds with a shape we don't
            // recognise (e.g. a future `status` string added
            // server-side before the CLI is updated).
            println!("{}", serde_json::to_string_pretty(&body)?);
            anyhow::bail!("tenant apply returned an unrecognised response shape")
        }
    }
}

/// Render a JSON `Value` on a single line, using compact separators.
/// Strings lose their surrounding quotes for readability in the
/// unified view (the path column already disambiguates — a path
/// ending in a list index obviously carries a non-string leaf).
fn compact_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "(null)".to_string(),
        _ => serde_json::to_string(v).unwrap_or_else(|_| "<?>".to_string()),
    }
}

async fn run_diff(args: TenantDiffArgs) -> anyhow::Result<()> {
    let manifest = read_manifest_input(&args.file)?;

    let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await else {
        anyhow::bail!("Not logged in — run `aeterna auth login` first.");
    };

    let body = client.tenant_diff(&manifest).await.inspect_err(|e| {
        ux_error::UxError::new(e.to_string())
            .fix("Run: aeterna auth login")
            .display();
    })?;

    // 200 = TenantDiff (no top-level `success` field); 422 =
    // validation-errors envelope with `success: false`. Same split
    // as `tenant_provision_dry_run`.
    let is_validation_error = body
        .get("success")
        .and_then(|v| v.as_bool())
        .map(|b| !b)
        .unwrap_or(false);

    if is_validation_error {
        match args.output {
            TenantDiffFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&body)?);
            }
            TenantDiffFormat::Unified => {
                render_validation_errors(&body);
            }
        }
        anyhow::bail!("tenant manifest is invalid");
    }

    match args.output {
        TenantDiffFormat::Json => {
            // Pretty-print for human readability; scripts that care
            // about byte stability should pipe through `jq -c`. We
            // deliberately do NOT emit compact JSON here — matches
            // the `tenant render` convention (pretty + trailing NL).
            let mut s = serde_json::to_string_pretty(&body)?;
            s.push('\n');
            print!("{s}");
        }
        TenantDiffFormat::Unified => {
            print!("{}", render_diff_unified(&body));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// `tenant watch` — SSE consumer (B2 §7.5)
// ---------------------------------------------------------------------------

/// One parsed SSE frame. We keep only the fields the endpoint emits;
/// `id`/`retry` are parsed out of the wire stream into nothing
/// because the server does not emit them (no replay semantics), but
/// the parser tolerates them gracefully.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct SseFrame {
    /// `event:` field. `None` → SSE spec default (`"message"`).
    event: Option<String>,
    /// Concatenated `data:` fields, joined by `\n` per the spec.
    data: String,
}

impl SseFrame {
    /// Event name with SSE default folded in — callers can match on a
    /// single `&str` without threading `Option` logic.
    fn event_name(&self) -> &str {
        self.event.as_deref().unwrap_or("message")
    }
}

/// Incremental SSE parser.
///
/// Fed arbitrary byte chunks (reqwest's `bytes_stream` does not
/// guarantee frame alignment) and emits complete frames as lines
/// cross the blank-line boundary. The SSE wire format is:
///
/// ```text
/// event: provisioning_step\n
/// data: {"slug":"acme",...}\n
/// \n    ← empty line terminates frame
/// ```
///
/// Multi-line `data:` fields concatenate with `\n`. Lines starting
/// with `:` are comments (used by `KeepAlive::default()` on the
/// server) — we ignore them. Malformed lines (no colon) are ignored
/// per the SSE spec's robustness rule.
struct SseParser {
    /// Carry-over bytes that did not end with a newline in the
    /// previous chunk. Owned so we can hand it back to the next
    /// `feed` call cheaply.
    buf: String,
    /// Frame under construction — reset on terminator.
    current: SseFrame,
}

impl SseParser {
    fn new() -> Self {
        Self {
            buf: String::new(),
            current: SseFrame::default(),
        }
    }

    /// Append a chunk. Returns any frames completed by this chunk.
    /// Allocates one `Vec` per call; events come in bursts of ≤ 10 so
    /// this is not a hot path worth pooling.
    fn feed(&mut self, chunk: &str) -> Vec<SseFrame> {
        self.buf.push_str(chunk);
        let mut out = Vec::new();

        // Drain whole lines from `buf`. A "line" is everything up to
        // (but not including) the next `\n`; `\r\n` is normalised by
        // trimming trailing `\r`. Anything after the last `\n` stays
        // in `buf` for the next call.
        loop {
            let Some(nl) = self.buf.find('\n') else {
                break;
            };
            // `drain(..=nl)` removes the newline too; strip the `\r`
            // before measuring length so CRLF survives untouched.
            let line: String = {
                let drained: String = self.buf.drain(..=nl).collect();
                let trimmed = drained.trim_end_matches('\n').trim_end_matches('\r');
                trimmed.to_string()
            };

            if line.is_empty() {
                // Frame boundary — only emit if we accumulated
                // something. A lone blank line (SSE heartbeat before
                // any data) is a legal no-op.
                if self.current.event.is_some() || !self.current.data.is_empty() {
                    out.push(std::mem::take(&mut self.current));
                }
                continue;
            }

            if let Some(rest) = line.strip_prefix(':') {
                // Comment line — servers use `:` as a keep-alive
                // ping (`KeepAlive::default()` sends `:\n\n`). Drop
                // it but don't log; it would spam stderr every 15 s.
                let _ = rest;
                continue;
            }

            // Field lines are `<name>: <value>` (space after colon is
            // optional per spec). Missing colon → treat whole line as
            // name with empty value (spec rule 9.2.6).
            let (name, value) = match line.split_once(':') {
                Some((n, v)) => (n, v.strip_prefix(' ').unwrap_or(v)),
                None => (line.as_str(), ""),
            };

            match name {
                "event" => self.current.event = Some(value.to_string()),
                "data" => {
                    if !self.current.data.is_empty() {
                        self.current.data.push('\n');
                    }
                    self.current.data.push_str(value);
                }
                // `id` / `retry` are not emitted by our server; we
                // tolerate them for forward-compat with a future
                // version that adds replay support.
                _ => {}
            }
        }
        out
    }
}

/// `aeterna tenant watch <slug>` entrypoint.
///
/// Opens a long-lived `GET /api/v1/admin/tenants/{slug}/events` and
/// prints one line per event to stdout. Exits:
/// * `0` when the stream closes (server shutdown or network close).
/// * non-zero when auth fails or the URL cannot be reached.
///
/// The command is intentionally tolerant of any event kind — it
/// renders `provisioning_step` and the three lifecycle kinds
/// prettily, and falls back to a verbatim dump for `unknown` and
/// future kinds so a newer server never produces blank output on an
/// older CLI.
async fn run_watch(args: TenantWatchArgs) -> anyhow::Result<()> {
    let Some(client) = get_live_client_for(args.target_tenant.as_deref()).await else {
        anyhow::bail!("Not logged in — run `aeterna auth login` first.");
    };

    if !args.json {
        eprintln!("Watching tenant '{}' — Ctrl-C to stop.", args.slug);
    }

    // `_cancel_tx` is intentionally dropped when the function
    // returns. Dropping fires the oneshot's recv() with an Err(_),
    // which the select! arm also treats as cancellation. But in
    // practice `run_watch` ends on stream EOF, not cancellation.
    let (_cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    stream_tenant_events(
        &client,
        &args.slug,
        args.json,
        FrameSink::Stdout,
        cancel_rx,
        None, // `tenant watch` has no stall timeout — it's an interactive tail
        None, // and no target event — it runs until Ctrl-C / EOF
    )
    .await
}

/// Target stream for rendered frames.
///
/// `run_watch` writes to stdout (that's the command's only output).
/// `run_apply --watch` writes to stderr so the final apply response
/// stays the single thing on stdout — preserving `| jq` pipelines.
#[derive(Clone, Copy)]
enum FrameSink {
    Stdout,
    Stderr,
}

impl FrameSink {
    fn emit(self, line: &str) {
        match self {
            Self::Stdout => println!("{line}"),
            Self::Stderr => eprintln!("{line}"),
        }
    }

    fn emit_warn(self, line: &str) {
        // warnings always go to stderr regardless of sink — they're
        // out-of-band and must never pollute stdout JSON.
        let _ = self;
        eprintln!("{line}");
    }
}

/// Render one parsed frame to stdout, honouring `--json`.
fn render_watch_frame(frame: &SseFrame, json_mode: bool) {
    render_watch_frame_to(frame, json_mode, FrameSink::Stdout);
}

/// Render one parsed frame to the given sink, honouring `--json`.
///
/// Extracted so `run_apply --watch` can reuse the exact same
/// formatting while routing frames to stderr.
fn render_watch_frame_to(frame: &SseFrame, json_mode: bool, sink: FrameSink) {
    if json_mode {
        // Raw mode: emit the `data:` payload verbatim, one JSON
        // object per line. Downstream tools like `jq -c` can consume
        // this without reconstructing frame boundaries.
        if !frame.data.is_empty() {
            sink.emit(&frame.data);
        }
        return;
    }

    // Pretty mode: parse the data as a TenantChangeEvent-shaped JSON
    // and render a human line. Failures fall through to a verbatim
    // dump so a newer server variant never produces blank output.
    let parsed: Option<serde_json::Value> = serde_json::from_str(&frame.data).ok();
    let slug = parsed
        .as_ref()
        .and_then(|v| v.get("slug"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    match frame.event_name() {
        "provisioned" => sink.emit(&format!("✓ {slug} provisioned")),
        "updated" => sink.emit(&format!("✓ {slug} updated")),
        "deactivated" => sink.emit(&format!("✗ {slug} deactivated")),
        "provisioning_step" => {
            // `kind` is the struct variant
            // `{"provisioning_step": {"step": "...", "status": "...", "detail": "..."}}`
            let step_obj = parsed
                .as_ref()
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.get("provisioning_step"));
            let step = step_obj
                .and_then(|v| v.get("step"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let status = step_obj
                .and_then(|v| v.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let detail = step_obj
                .and_then(|v| v.get("detail"))
                .and_then(|v| v.as_str());
            let marker = match status {
                "started" => "→",
                "ok" => "✓",
                "failed" => "✗",
                _ => "·",
            };
            match detail {
                Some(d) => sink.emit(&format!("  {marker} {step:<16} {status:<8} {d}")),
                None => sink.emit(&format!("  {marker} {step:<16} {status}")),
            }
        }
        "lagged" => {
            // Synthetic frame emitted by the SSE endpoint when a
            // subscriber fell behind. Surface it so operators realise
            // they missed events and know to reconnect.
            let skipped = parsed
                .as_ref()
                .and_then(|v| v.get("skipped"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            sink.emit_warn(&format!(
                "warning: stream lagged — {skipped} event(s) dropped. \
                 Reconnect to re-sync."
            ));
        }
        other => {
            // Unknown / forward-compat kinds — print verbatim so a
            // newer server never produces blank output.
            sink.emit(&format!("· [{other}] {}", frame.data));
        }
    }
}

/// Decide whether an SSE frame satisfies a `--watch-until=<target>`
/// predicate.
///
/// Matching rules (in order):
/// * bare kind (`provisioned`, `updated`, `deactivated`, `lagged`,
///   `provisioning_step`) → matches iff `frame.event_name()` equals
///   the target;
/// * `step:<name>` → matches iff `event_name() == "provisioning_step"`
///   AND the parsed JSON payload carries
///   `kind.provisioning_step.step == <name>` AND
///   `kind.provisioning_step.status == "ok"` (we always wait for the
///   *completion* of a step, never its `started`). This gives the
///   CLI a stable way to block on "step X finished successfully"
///   without needing a CLI release each time the server adds a
///   new step name.
fn frame_matches_target(frame: &SseFrame, target: &str) -> bool {
    if let Some(step_name) = target.strip_prefix("step:") {
        if frame.event_name() != "provisioning_step" {
            return false;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&frame.data) else {
            return false;
        };
        let step_obj = v.get("kind").and_then(|k| k.get("provisioning_step"));
        let name_matches = step_obj
            .and_then(|s| s.get("step"))
            .and_then(|s| s.as_str())
            == Some(step_name);
        let status_ok = step_obj
            .and_then(|s| s.get("status"))
            .and_then(|s| s.as_str())
            == Some("ok");
        return name_matches && status_ok;
    }
    frame.event_name() == target
}

/// Open an SSE subscription for `slug` and forward frames to `sink`
/// until either the server closes the stream or `cancel` is fired.
///
/// Used by `run_watch` (sink = stdout, cancel = never, no stall
/// timeout) and by `run_apply --watch [--watch-timeout=N]`
/// (sink = stderr, cancel = fired after the apply HTTP round-trip
/// returns, stall timeout resets on every incoming byte chunk).
///
/// `stall_timeout`: when `Some(dur)`, bails with a `watch_stall`
/// error if no chunk arrives within `dur`. Resets on every chunk
/// (not every parsed frame — a batched chunk is one reset, which
/// is the correct signal: the server is still live). When `None`,
/// the stream runs untimed. (B2 §7.7)
///
/// `until_event`: when `Some(name)`, returns `Ok(())` as soon as
/// a frame with `event_name() == name` is parsed (and rendered).
/// Accepts a bare kind (`provisioned`, `updated`, `deactivated`,
/// `lagged`, `provisioning_step`) or a `step:<name>` form — the
/// latter matches any `provisioning_step` frame whose parsed
/// `kind.provisioning_step.step` field equals `<name>`. When
/// `None`, the stream runs until EOF / cancel / stall. (B2 §7.8)
async fn stream_tenant_events(
    client: &crate::client::AeternaClient,
    slug: &str,
    json_mode: bool,
    sink: FrameSink,
    mut cancel: tokio::sync::oneshot::Receiver<()>,
    stall_timeout: Option<std::time::Duration>,
    until_event: Option<&str>,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;

    let path = format!("/api/v1/admin/tenants/{slug}/events");
    let resp = client.get(&path).await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Server rejected stream open ({}): {}", status, body.trim());
    }

    if let Some(ct) = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        && !ct.contains("text/event-stream")
    {
        sink.emit_warn(&format!(
            "warning: server Content-Type is `{ct}` (expected text/event-stream) — \
             a buffering proxy may hold events back"
        ));
    }

    let mut stream = resp.bytes_stream();
    let mut parser = SseParser::new();

    loop {
        // Build a stall-timeout future each iteration. When the flag
        // is off, substitute a future that never resolves so the
        // select! below treats it as a permanently pending arm.
        let stall_fut: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
            match stall_timeout {
                Some(dur) => Box::pin(tokio::time::sleep(dur)),
                None => Box::pin(std::future::pending::<()>()),
            };

        tokio::select! {
            // Cancellation wins if both arms are ready, so a late
            // frame arriving concurrently with the cancel signal
            // still gets rendered only if the select picks the
            // stream arm. That's fine — apply-watch bounds the
            // "trailing frame" window to whatever's already buffered
            // in reqwest's chunk queue.
            _ = &mut cancel => return Ok(()),
            () = stall_fut => {
                anyhow::bail!(
                    "watch_stall: no event received from tenant '{slug}' within {}s \
                     (server may be wedged — check `aeterna admin health`)",
                    stall_timeout.map(|d| d.as_secs()).unwrap_or(0),
                );
            }
            maybe_chunk = stream.next() => {
                let Some(chunk) = maybe_chunk else { return Ok(()) };
                let chunk = chunk.map_err(|e| anyhow::anyhow!("stream read failed: {e}"))?;
                let text = String::from_utf8_lossy(&chunk);
                for frame in parser.feed(&text) {
                    render_watch_frame_to(&frame, json_mode, sink);
                    // --watch-until match check — done AFTER the
                    // render so the matching frame is visible in
                    // the user's event trail (consistent with how
                    // `kubectl wait` prints the state that made it
                    // satisfy the condition).
                    if let Some(target) = until_event
                        && frame_matches_target(&frame, target)
                    {
                        return Ok(());
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_create_args_dry_run_fields() {
        let args = TenantCreateArgs {
            slug: "acme".to_string(),
            name: "Acme Corp".to_string(),
            json: false,
            dry_run: true,
        };
        assert_eq!(args.slug, "acme");
        assert_eq!(args.name, "Acme Corp");
        assert!(args.dry_run);
        assert!(!args.json);
    }

    #[test]
    fn test_tenant_list_args_defaults() {
        let args = TenantListArgs {
            include_inactive: false,
            target_tenant: None,
            json: false,
        };
        assert!(!args.include_inactive);
        assert!(!args.json);
    }

    #[test]
    fn test_tenant_list_args_include_inactive() {
        let args = TenantListArgs {
            include_inactive: true,
            target_tenant: None,
            json: true,
        };
        assert!(args.include_inactive);
        assert!(args.json);
    }

    #[test]
    fn test_tenant_show_args() {
        let args = TenantShowArgs {
            tenant: "acme".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant, "acme");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_update_args_partial() {
        let args = TenantUpdateArgs {
            tenant: "acme".to_string(),
            new_slug: None,
            name: Some("Acme Corporation".to_string()),
            json: false,
            dry_run: false,
        };
        assert!(args.new_slug.is_none());
        assert_eq!(args.name, Some("Acme Corporation".to_string()));
    }

    #[test]
    fn test_tenant_update_args_dry_run() {
        let args = TenantUpdateArgs {
            tenant: "acme".to_string(),
            new_slug: Some("acme-corp".to_string()),
            name: None,
            json: false,
            dry_run: true,
        };
        assert!(args.dry_run);
        assert_eq!(args.new_slug, Some("acme-corp".to_string()));
    }

    #[test]
    fn test_tenant_deactivate_args_requires_yes() {
        let args = TenantDeactivateArgs {
            tenant: "acme".to_string(),
            yes: false,
            json: false,
        };
        assert!(!args.yes);
    }

    #[test]
    fn test_tenant_deactivate_args_confirmed() {
        let args = TenantDeactivateArgs {
            tenant: "acme".to_string(),
            yes: true,
            json: true,
        };
        assert!(args.yes);
        assert!(args.json);
    }

    #[test]
    fn test_tenant_use_args() {
        let args = TenantUseArgs {
            tenant: "acme".to_string(),
        };
        assert_eq!(args.tenant, "acme");
    }

    #[test]
    fn test_tenant_domain_map_args() {
        let args = TenantDomainMapArgs {
            tenant: "acme".to_string(),
            domain: "acme.example.com".to_string(),
            json: false,
        };
        assert_eq!(args.domain, "acme.example.com");
    }

    #[test]
    fn test_repo_binding_body_local() {
        let body = repo_binding_body(
            "local",
            Some("/repos/acme"),
            None,
            Some("main"),
            Some("directCommit"),
            None,
            None,
            None,
            None,
        );
        assert_eq!(body["kind"], "local");
        assert_eq!(body["localPath"], "/repos/acme");
        assert_eq!(body["branch"], "main");
        assert_eq!(body["branchPolicy"], "directCommit");
        assert_eq!(body["sourceOwner"], "admin");
    }

    #[test]
    fn test_repo_binding_body_github() {
        let body = repo_binding_body(
            "github",
            None,
            None,
            Some("main"),
            Some("directCommit"),
            Some("githubApp"),
            Some("my-app-cred"),
            Some("acme-org"),
            Some("knowledge-repo"),
        );
        assert_eq!(body["kind"], "github");
        assert_eq!(body["githubOwner"], "acme-org");
        assert_eq!(body["githubRepo"], "knowledge-repo");
        assert_eq!(body["credentialKind"], "githubApp");
        assert_eq!(body["credentialRef"], "my-app-cred");
    }

    #[test]
    fn test_repo_binding_body_remote() {
        let body = repo_binding_body(
            "gitRemote",
            None,
            Some("https://github.com/acme/knowledge.git"),
            None,
            None,
            Some("sshKey"),
            Some("acme-deploy-key"),
            None,
            None,
        );
        assert_eq!(body["kind"], "gitRemote");
        assert_eq!(body["remoteUrl"], "https://github.com/acme/knowledge.git");
        assert_eq!(body["credentialKind"], "sshKey");
    }

    #[test]
    fn test_repo_binding_body_minimal() {
        let body = repo_binding_body("local", None, None, None, None, None, None, None, None);
        assert_eq!(body["kind"], "local");
        assert_eq!(body["sourceOwner"], "admin");
        assert!(body.get("localPath").is_none());
        assert!(body.get("remoteUrl").is_none());
    }

    #[test]
    fn test_tenant_repo_binding_show_args() {
        let args = TenantRepoBindingShowArgs {
            tenant: "acme".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant, "acme");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_repo_binding_set_args() {
        let args = TenantRepoBindingSetArgs {
            tenant: "acme".to_string(),
            kind: "github".to_string(),
            local_path: None,
            remote_url: None,
            branch: Some("main".to_string()),
            branch_policy: Some("directCommit".to_string()),
            credential_kind: Some("githubApp".to_string()),
            credential_ref: Some("my-cred".to_string()),
            github_owner: Some("acme-org".to_string()),
            github_repo: Some("knowledge-repo".to_string()),
            target_tenant: None,
            json: false,
            dry_run: false,
        };
        assert_eq!(args.kind, "github");
        assert_eq!(args.github_owner, Some("acme-org".to_string()));
        assert!(!args.dry_run);
    }

    #[test]
    fn test_tenant_repo_binding_validate_args() {
        let args = TenantRepoBindingValidateArgs {
            tenant: "acme".to_string(),
            kind: "local".to_string(),
            local_path: Some("/repos/acme".to_string()),
            remote_url: None,
            branch: None,
            branch_policy: None,
            credential_kind: None,
            credential_ref: None,
            github_owner: None,
            github_repo: None,
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.kind, "local");
        assert!(args.json);
    }

    #[test]
    fn test_tenant_config_inspect_args() {
        let args = TenantConfigInspectArgs {
            tenant: Some("acme".to_string()),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.tenant.as_deref(), Some("acme"));
        assert!(args.json);
    }

    #[test]
    fn test_tenant_config_upsert_args_dry_run() {
        let args = TenantConfigUpsertArgs {
            tenant: None,
            file: "config.json".to_string(),
            target_tenant: Some("acme".to_string()),
            json: false,
            dry_run: true,
        };
        assert!(args.dry_run);
        assert_eq!(args.file, "config.json");
        assert_eq!(args.target_tenant.as_deref(), Some("acme"));
    }

    #[test]
    fn test_tenant_secret_set_args() {
        let args = TenantSecretSetArgs {
            tenant: Some("acme".to_string()),
            logical_name: "repo.token".to_string(),
            value: Some("s3cr3t".to_string()),
            allow_inline_secret: true,
            reference: None,
            from_file: None,
            from_stdin: false,
            from_env: None,
            ownership: "tenant".to_string(),
            target_tenant: None,
            json: true,
        };
        assert_eq!(args.logical_name, "repo.token");
        assert_eq!(args.ownership, "tenant");
        assert!(args.json);
        assert!(args.allow_inline_secret);
    }

    #[test]
    fn test_tenant_secret_delete_args() {
        let args = TenantSecretDeleteArgs {
            tenant: None,
            logical_name: "repo.token".to_string(),
            target_tenant: Some("acme".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("acme"));
        assert_eq!(args.logical_name, "repo.token");
    }

    #[test]
    fn test_redact_secret_values() {
        let mut payload = json!({
            "secretValue": "raw-secret",
            "nested": {
                "secret_value": "also-raw"
            }
        });
        redact_secret_values(&mut payload);
        assert_eq!(payload["secretValue"], "[REDACTED]");
        assert_eq!(payload["nested"]["secret_value"], "[REDACTED]");
    }

    #[test]
    fn test_tenant_config_ownership_validation() {
        assert_eq!(tenant_config_ownership("tenant").unwrap(), "tenant");
        assert_eq!(tenant_config_ownership("platform").unwrap(), "platform");
        assert!(tenant_config_ownership("invalid").is_err());
    }

    #[test]
    fn test_tenant_list_args_target_tenant() {
        let args = TenantListArgs {
            include_inactive: false,
            target_tenant: Some("platform-tenant".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("platform-tenant"));
    }

    #[test]
    fn test_tenant_show_args_target_tenant() {
        let args = TenantShowArgs {
            tenant: "acme".to_string(),
            target_tenant: Some("parent-tenant".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("parent-tenant"));
    }

    #[test]
    fn test_tenant_repo_binding_show_args_target_tenant() {
        let args = TenantRepoBindingShowArgs {
            tenant: "acme".to_string(),
            target_tenant: Some("admin-context".to_string()),
            json: false,
        };
        assert_eq!(args.target_tenant.as_deref(), Some("admin-context"));
    }

    // -----------------------------------------------------------------------
    // #45: server-backed switch / current
    // -----------------------------------------------------------------------

    #[test]
    fn test_tenant_switch_args_basic() {
        let args = TenantSwitchArgs {
            tenant: "acme".to_string(),
            clear: false,
            json: false,
        };
        assert_eq!(args.tenant, "acme");
        assert!(!args.clear);
    }

    #[test]
    fn test_tenant_switch_args_clear_flag() {
        let args = TenantSwitchArgs {
            tenant: "ignored".to_string(),
            clear: true,
            json: true,
        };
        assert!(args.clear);
        assert!(args.json);
    }

    #[test]
    fn test_tenant_current_args_json() {
        let args = TenantCurrentArgs { json: true };
        assert!(args.json);
    }

    // `test_read_local_context_tenant_{absent,present}` both mutate
    // the process-wide cwd (`std::env::set_current_dir`), which Rust
    // runs tests in parallel against by default. Without a shared
    // mutex, a second test can observe cwd mid-switch from a sibling
    // and panic on a stale relative path. A static `Mutex` wrapping
    // the critical section makes them `#[serial]`-equivalent without
    // pulling in a new dev-dep.
    fn cwd_guard() -> &'static std::sync::Mutex<()> {
        use std::sync::{Mutex, OnceLock};
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn test_read_local_context_tenant_absent() {
        // Runs in a temp dir with no .aeterna/context.toml.
        // We guard on `cwd_guard()` because `set_current_dir` is a
        // process-global side effect and `cargo test` parallelises.
        let _g = cwd_guard().lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = read_local_context_tenant();
        std::env::set_current_dir(cwd).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_local_context_tenant_present() {
        // Same cwd-mutex contract as the `_absent` sibling above.
        let _g = cwd_guard().lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let aeterna = tmp.path().join(".aeterna");
        std::fs::create_dir_all(&aeterna).unwrap();
        std::fs::write(aeterna.join("context.toml"), "tenant_id = \"acme\"\n").unwrap();

        let cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let result = read_local_context_tenant();
        std::env::set_current_dir(cwd).unwrap();

        assert_eq!(result.as_deref(), Some("acme"));
    }

    // ---------------------------------------------------------------
    // tenant validate (§7.1)
    // ---------------------------------------------------------------

    #[test]
    fn test_tenant_validate_args_roundtrip() {
        // No derived parse — we just assert the shape matches what the
        // dispatcher wires up. If these field names drift, downstream
        // doc/examples and shell scripts break, so the test is the
        // canary for rename regressions.
        let args = TenantValidateArgs {
            file: "manifest.json".to_string(),
            json: true,
        };
        assert_eq!(args.file, "manifest.json");
        assert!(args.json);
    }

    #[test]
    fn test_read_manifest_input_from_file() {
        // A well-formed JSON file round-trips through read_manifest_input
        // without mutation (no normalization, no re-serialization).
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("m.json");
        std::fs::write(
            &path,
            r#"{"tenant":{"slug":"acme","name":"Acme"},"config":{}}"#,
        )
        .unwrap();
        let v = read_manifest_input(path.to_str().unwrap()).unwrap();
        assert_eq!(v["tenant"]["slug"], "acme");
        assert_eq!(v["tenant"]["name"], "Acme");
    }

    #[test]
    fn test_read_manifest_input_rejects_invalid_json() {
        // Malformed JSON surfaces a clear error mentioning the path.
        // This is the primary failure mode for CI pipelines that
        // generate manifests from templates — if the template outputs
        // a trailing comma, we want the error to be immediate and
        // pointed, not a cryptic server-side 400.
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        std::fs::write(&path, "{not valid json").unwrap();
        let err = read_manifest_input(path.to_str().unwrap()).expect_err("expected parse error");
        let msg = err.to_string();
        assert!(
            msg.contains("Invalid JSON") || msg.contains("invalid") || msg.contains("expected"),
            "unexpected error message: {msg}"
        );
    }

    // -----------------------------------------------------------------
    // B2 §7.5 — `tenant watch` SSE parser
    // -----------------------------------------------------------------

    #[test]
    fn sse_parser_single_frame_lf() {
        // Happy path: a complete frame arrives in one chunk with LF
        // line endings (Axum's Sse writes LF, not CRLF).
        let mut p = SseParser::new();
        let frames = p.feed("event: provisioned\ndata: {\"slug\":\"acme\"}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_name(), "provisioned");
        assert_eq!(frames[0].data, r#"{"slug":"acme"}"#);
    }

    #[test]
    fn sse_parser_handles_crlf() {
        // Some middleboxes (Azure Front Door, older nginx) rewrite LF
        // to CRLF. The parser must normalise.
        let mut p = SseParser::new();
        let frames = p.feed("event: updated\r\ndata: {\"x\":1}\r\n\r\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_name(), "updated");
        assert_eq!(frames[0].data, r#"{"x":1}"#);
    }

    #[test]
    fn sse_parser_splits_across_chunks() {
        // reqwest's `bytes_stream` makes no framing guarantees — a
        // single logical frame may arrive as a dozen tiny chunks.
        // The parser must hold state across `feed` calls.
        let mut p = SseParser::new();
        let mut frames = Vec::new();
        for chunk in [
            "event: provi",
            "sioning_step\n",
            "data: {\"slug\"",
            ":\"acme\"}\n",
            "\n",
        ] {
            frames.extend(p.feed(chunk));
        }
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_name(), "provisioning_step");
        assert_eq!(frames[0].data, r#"{"slug":"acme"}"#);
    }

    #[test]
    fn sse_parser_ignores_comment_lines() {
        // `KeepAlive::default()` on the server emits `:\n\n` every
        // 15 s. These are not events — parser must swallow them and
        // not synthesise a phantom empty frame.
        let mut p = SseParser::new();
        let frames = p.feed(":\n\n:ping\n\n");
        assert!(
            frames.is_empty(),
            "keep-alive pings must not produce frames: {frames:?}"
        );
    }

    #[test]
    fn sse_parser_concatenates_multiline_data() {
        // SSE spec: multiple `data:` fields in one frame concat with
        // `\n`. Prevents silently dropping continuation lines.
        let mut p = SseParser::new();
        let frames = p.feed("data: line1\ndata: line2\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "line1\nline2");
        assert_eq!(frames[0].event_name(), "message"); // SSE default
    }

    #[test]
    fn sse_parser_strips_leading_space_after_colon() {
        // Spec: optional space after colon. "data:foo" and
        // "data: foo" must both yield "foo".
        let mut p = SseParser::new();
        let f1 = p.feed("data:no-space\n\n");
        assert_eq!(f1[0].data, "no-space");
        let f2 = p.feed("data: with-space\n\n");
        assert_eq!(f2[0].data, "with-space");
    }

    #[test]
    fn sse_parser_survives_stream_break_mid_field() {
        // Partial line on one side of a feed boundary must survive to
        // the next call without corrupting the next event.
        let mut p = SseParser::new();
        assert!(
            p.feed("event: prov").is_empty(),
            "partial line must not emit"
        );
        let frames = p.feed("isioned\ndata: {}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event_name(), "provisioned");
    }

    #[test]
    fn sse_frame_default_event_name_is_message() {
        // Matches the SSE spec default. Callers downstream branch on
        // this string verbatim so the default must be exact.
        let f = SseFrame::default();
        assert_eq!(f.event_name(), "message");
    }

    #[test]
    fn tenant_watch_args_minimal_shape() {
        // Smoke: the Args struct can be constructed with the minimal
        // fields and preserves them. Same pattern as every other
        // `*Args` test in this module.
        let args = TenantWatchArgs {
            slug: "acme".into(),
            json: false,
            target_tenant: None,
        };
        assert_eq!(args.slug, "acme");
        assert!(!args.json);
        assert!(args.target_tenant.is_none());
    }

    #[test]
    fn test_render_validation_errors_returns_true_with_errors() {
        // Validation-error renderer: returns true (caller gates exit
        // code on it) and tolerates empty/missing arrays.
        let body = json!({
            "success": false,
            "error": "manifest_validation_failed",
            "validationErrors": [
                "tenant.slug must not be empty",
                "config.fields.foo: invalid UTF-8",
            ],
        });
        assert!(render_validation_errors(&body));
    }

    #[test]
    fn test_render_validation_errors_handles_missing_array() {
        // Defensive: if the server returns 422 without a
        // validationErrors array (unlikely but possible for
        // future error codes), we still print something sensible
        // and return true so the caller exits non-zero.
        let body = json!({
            "success": false,
            "error": "something_else",
        });
        assert!(render_validation_errors(&body));
    }

    // ── §7.2 tenant render unit tests ────────────────────────────────────

    #[test]
    fn render_slug_explicit_flag_wins_over_context() {
        // When `--slug` is passed it must be used verbatim, regardless
        // of whatever the active context might have set. Without this
        // guarantee a CI operator setting AETERNA_TENANT_ID in env
        // would accidentally override an explicit `--slug prod`.
        let got = resolve_render_slug(Some("explicit-slug"));
        assert_eq!(got.as_deref(), Some("explicit-slug"));
    }

    // NOTE: the "slug=None → fall back to active context" path is
    // covered by integration tests that pin the cwd; unit-testing it
    // here would race with `test_read_local_context_tenant_present`
    // which calls `std::env::set_current_dir` (`load_resolved` reads
    // `current_dir()` and would pick up that test's temp dir).

    #[test]
    fn serialize_rendered_manifest_emits_pretty_json_with_trailing_newline() {
        // Lock the byte shape: 2-space indent (serde_json default),
        // LF line endings, exactly one trailing newline so `git` /
        // `diff` / `cat` treat the output as a well-formed text file.
        let v = json!({
            "apiVersion": "aeterna.io/v1",
            "kind": "TenantManifest",
            "tenant": {"slug": "acme"},
        });
        let s = serialize_rendered_manifest(&v).unwrap();
        assert!(
            s.ends_with('\n'),
            "output must end with exactly one newline: {s:?}"
        );
        assert!(!s.ends_with("\n\n"), "no double newline: {s:?}");
        // Indent check — `to_string_pretty` uses two spaces.
        assert!(
            s.contains("  \"apiVersion\""),
            "expected 2-space indent: {s}"
        );
        // Round-trips back to the same JSON.
        let v2: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn serialize_rendered_manifest_handles_nested_structures() {
        // Regression guard for a real rendered manifest shape —
        // nested objects, arrays, nulls, numbers. The serializer has
        // no business-logic awareness, but this test locks that we
        // don't accidentally reach for a BTreeMap or re-sort keys
        // somewhere down the line and silently change the wire order.
        let v = json!({
            "apiVersion": "aeterna.io/v1",
            "metadata": {"generation": 7, "manifestHash": null},
            "tenant": {
                "id": "t-1",
                "domainMappings": ["acme.com", "beta.acme.com"],
            },
            "roles": [
                {"userId": "u-1", "role": "admin"},
                {"userId": "u-2", "role": "member", "unit": "Platform"},
            ],
            "notRendered": [],
        });
        let s = serialize_rendered_manifest(&v).unwrap();
        assert!(s.ends_with('\n'));
        // Preserves insertion order in the serialized form.
        let api_idx = s.find("\"apiVersion\"").expect("apiVersion present");
        let roles_idx = s.find("\"roles\"").expect("roles present");
        assert!(api_idx < roles_idx, "apiVersion must render before roles");
    }

    #[test]
    fn tenant_render_args_defaults() {
        let args = TenantRenderArgs {
            slug: None,
            redact: false,
            output: None,
            target_tenant: None,
        };
        assert!(args.slug.is_none());
        assert!(!args.redact);
        assert!(args.output.is_none());
    }

    #[test]
    fn tenant_render_args_with_all_flags() {
        let args = TenantRenderArgs {
            slug: Some("acme".into()),
            redact: true,
            output: Some(std::path::PathBuf::from("/tmp/acme.json")),
            target_tenant: Some("prod".into()),
        };
        assert_eq!(args.slug.as_deref(), Some("acme"));
        assert!(args.redact);
        assert_eq!(
            args.output.as_deref(),
            Some(std::path::Path::new("/tmp/acme.json"))
        );
    }

    // -----------------------------------------------------------------
    // §7.1 tenant apply — outcome classifier + renderer byte shape
    // -----------------------------------------------------------------

    /// Canonical 200 `{"success": true, "status": "applied", ...}`
    /// server response. Locks the exact wire shape the CLI renderer
    /// is fed; a server-side rename (e.g. `status` → `result`) would
    /// break these tests and catch the drift at CI rather than in
    /// production operator tooling.
    fn sample_applied_body() -> Value {
        json!({
            "success": true,
            "status": "applied",
            "tenantId": "01J8Z9K...",
            "slug": "acme",
            "hash": "a".repeat(64),
            "generation": 6,
            "steps": [
                { "step": "tenant", "ok": true, "detail": "slug=acme name=Acme" },
                { "step": "config", "ok": true, "detail": "5 fields upserted" },
                { "step": "secrets", "ok": true, "detail": "2 references bound" },
            ]
        })
    }

    // -----------------------------------------------------------------
    // §7.3 tenant diff — unified-output byte shape locks
    // -----------------------------------------------------------------

    /// Canonical 200 `TenantDiff` shape used across the unified
    /// renderer tests. Mirrors the server's
    /// `cli/src/server/tenant_diff.rs::TenantDiff` wire format
    /// (camelCase, lowercase enums). Kept as a hand-written `json!`
    /// literal rather than deserialising from the typed struct so
    /// the CLI renderer is exercised against the actual wire bytes
    /// it will see in production.
    fn sample_update_diff() -> Value {
        json!({
            "slug": "acme",
            "operation": "update",
            "changes": [
                { "path": "tenant.name", "kind": "modified",
                  "before": "Acme", "after": "Acme Corp" },
                { "path": "providers.llm.kind", "kind": "added",
                  "after": "openai" },
                { "path": "hierarchy.0.slug", "kind": "removed",
                  "before": "legacy-org" },
            ],
            "summary": {
                "added": 1,
                "removed": 1,
                "modified": 1,
                "changedSections": ["hierarchy", "providers", "tenant"]
            }
        })
    }

    #[test]
    fn test_classify_applied() {
        let body = sample_applied_body();
        assert_eq!(classify_apply_response(&body), ApplyOutcome::Applied);
    }

    #[test]
    fn test_classify_unchanged() {
        let body = json!({
            "success": true,
            "status": "unchanged",
            "slug": "acme",
            "hash": "a".repeat(64),
            "generation": 6,
            "steps": []
        });
        assert_eq!(classify_apply_response(&body), ApplyOutcome::Unchanged);
    }

    #[test]
    fn test_classify_partial() {
        // 207 Multi-Status — tenant row created, but a downstream step
        // failed. CLI must render the failing step and bail non-zero.
        let body = json!({
            "success": false,
            "status": "partial",
            "slug": "acme",
            "hash": "a".repeat(64),
            "generation": 6,
            "steps": [
                { "step": "tenant", "ok": true, "detail": "slug=acme" },
                { "step": "repository", "ok": false,
                  "error": "invalid_credential_ref: missing provider" },
            ]
        });
        assert_eq!(classify_apply_response(&body), ApplyOutcome::Partial);
    }

    #[test]
    fn test_classify_generation_conflict() {
        let body = json!({
            "success": false,
            "error": "generation_conflict",
            "slug": "acme",
            "currentGeneration": 7,
            "submittedGeneration": 6,
            "hint": "metadata.generation must be strictly greater than the current generation"
        });
        assert_eq!(
            classify_apply_response(&body),
            ApplyOutcome::GenerationConflict
        );
    }

    #[test]
    fn test_classify_validation_failed() {
        let body = json!({
            "success": false,
            "error": "manifest_validation_failed",
            "validationErrors": [
                { "path": "tenant.slug", "message": "must match [a-z0-9-]+" }
            ]
        });
        assert_eq!(
            classify_apply_response(&body),
            ApplyOutcome::ValidationFailed
        );
    }

    #[test]
    fn test_classify_parse_failed_as_validation() {
        // `manifest_parse_failed` (HTTP 400) and
        // `manifest_validation_failed` (HTTP 422) are both "operator
        // wrote a bad manifest"; grouped under the same outcome so
        // the CLI renders them with the same affordance (validate
        // errors section), rather than the opaque "unrecognised
        // response shape" branch.
        let body = json!({
            "success": false,
            "error": "manifest_parse_failed",
            "details": "missing field `tenant.slug`"
        });
        assert_eq!(
            classify_apply_response(&body),
            ApplyOutcome::ValidationFailed
        );
    }

    #[test]
    fn test_classify_inline_secret_rejected() {
        let body = json!({
            "success": false,
            "error": "inline_secret_not_allowed",
            "offendingSecrets": ["openai-key", "stripe-key"]
        });
        assert_eq!(
            classify_apply_response(&body),
            ApplyOutcome::InlineSecretRejected
        );
    }

    #[test]
    fn test_classify_unknown_shape() {
        // Server returns a shape the CLI does not know about —
        // forward-compat catch-all. Must not crash; must be
        // classifiable so the caller can render the raw body.
        let body = json!({
            "success": true,
            "status": "queued",
            "slug": "acme",
            "jobId": "j-01"
        });
        assert_eq!(classify_apply_response(&body), ApplyOutcome::Other);
    }

    #[test]
    fn test_render_apply_result_applied_shape() {
        let body = sample_applied_body();
        let out = render_apply_result(&body, &ApplyOutcome::Applied);
        assert!(out.starts_with("Tenant apply: acme\n"));
        assert!(out.contains("Result:       ✓ applied\n"));
        assert!(out.contains("Generation:   6\n"));
        assert!(out.contains("Steps:\n"));
        assert!(out.contains("  ✓ tenant: slug=acme name=Acme\n"));
        assert!(out.contains("  ✓ config: 5 fields upserted\n"));
    }

    #[test]
    fn test_render_apply_result_unchanged_shape() {
        let body = json!({
            "success": true,
            "status": "unchanged",
            "slug": "acme",
            "hash": "a".repeat(64),
            "generation": 6,
            "steps": []
        });
        let out = render_apply_result(&body, &ApplyOutcome::Unchanged);
        assert!(out.contains("Result:       · unchanged (no-op)\n"));
        // No `Steps:` section when the array is empty — the renderer
        // suppresses it to keep unchanged output compact.
        assert!(!out.contains("Steps:\n"));
    }

    #[test]
    fn test_render_apply_result_partial_shows_failures() {
        let body = json!({
            "success": false,
            "status": "partial",
            "slug": "acme",
            "hash": "a".repeat(64),
            "generation": 6,
            "steps": [
                { "step": "tenant", "ok": true, "detail": "slug=acme" },
                { "step": "repository", "ok": false,
                  "error": "invalid_credential_ref: missing provider" },
            ]
        });
        let out = render_apply_result(&body, &ApplyOutcome::Partial);
        assert!(out.contains("Result:       ⚠ partial\n"));
        assert!(out.contains("  ✓ tenant: slug=acme\n"));
        assert!(
            out.contains("  ✗ repository: invalid_credential_ref: missing provider\n"),
            "failing step must show error message; got:\n{out}"
        );
    }

    #[test]
    fn test_render_generation_conflict_is_actionable() {
        let body = json!({
            "success": false,
            "error": "generation_conflict",
            "currentGeneration": 7,
            "submittedGeneration": 6,
            "hint": "metadata.generation must be strictly greater than the current generation"
        });
        let out = render_generation_conflict(&body);
        assert!(out.contains("generation_conflict"));
        assert!(out.contains("  current:    7\n"));
        assert!(out.contains("  submitted:  6\n"));
        assert!(out.contains("hint:"));
    }

    #[test]
    fn test_render_inline_secret_rejected_lists_offenders() {
        let body = json!({
            "success": false,
            "error": "inline_secret_not_allowed",
            "offendingSecrets": ["openai-key", "stripe-key"],
            "message": "Inline plaintext is disabled on this server."
        });
        let out = render_inline_secret_rejected(&body);
        assert!(out.contains("inline_secret_not_allowed"));
        assert!(out.contains("    • openai-key\n"));
        assert!(out.contains("    • stripe-key\n"));
        assert!(out.contains("Inline plaintext is disabled"));
    }

    #[test]
    fn test_render_diff_unified_update_shape() {
        let out = render_diff_unified(&sample_update_diff());
        // Header lines, exact byte-positions matter for CI grep
        // patterns; lock them here.
        assert!(out.starts_with("Tenant diff: acme\nOperation:   update\n"));
        assert!(out.contains("Summary:     +1 -1 ~1 (sections: hierarchy, providers, tenant)\n"));
        // One line per change, in the order the server emitted.
        assert!(out.contains("~ tenant.name: Acme → Acme Corp\n"));
        assert!(out.contains("+ providers.llm.kind: openai\n"));
        assert!(out.contains("- hierarchy.0.slug: legacy-org\n"));
    }

    #[test]
    fn test_render_diff_unified_noop_shape() {
        let diff = json!({
            "slug": "acme",
            "operation": "noop",
            "changes": [],
            "summary": {
                "added": 0, "removed": 0, "modified": 0,
                "changedSections": []
            }
        });
        let out = render_diff_unified(&diff);
        assert!(out.contains("Operation:   noop\n"));
        assert!(out.contains("Summary:     +0 -0 ~0 (sections: none)\n"));
        // NoOp short-circuits — no per-change lines, just the hint.
        assert!(out.contains("(no changes — a re-apply would be a no-op)\n"));
        assert!(!out.contains("\n+ "));
        assert!(!out.contains("\n- "));
        assert!(!out.contains("\n~ "));
    }

    #[test]
    fn test_render_diff_unified_create_shape() {
        // First-apply: operation=create, every field appears as Added.
        let diff = json!({
            "slug": "fresh",
            "operation": "create",
            "changes": [
                { "path": "tenant.slug", "kind": "added", "after": "fresh" },
                { "path": "tenant.name", "kind": "added", "after": "Fresh Co" },
            ],
            "summary": {
                "added": 2, "removed": 0, "modified": 0,
                "changedSections": ["tenant"]
            }
        });
        let out = render_diff_unified(&diff);
        assert!(out.contains("Operation:   create\n"));
        assert!(out.contains("+ tenant.slug: fresh\n"));
        assert!(out.contains("+ tenant.name: Fresh Co\n"));
        // create is NOT a noop — the hint must not appear.
        assert!(!out.contains("no-op"));
    }

    #[test]
    fn test_render_diff_unified_complex_values() {
        // Non-string leaves render as compact JSON; null renders as
        // `(null)` so grep-on-operator output never shows bare `null`
        // ambiguously against a real string value of that name.
        let diff = json!({
            "slug": "acme",
            "operation": "update",
            "changes": [
                { "path": "providers.memoryLayers.semantic.ttl",
                  "kind": "modified", "before": 3600, "after": 7200 },
                { "path": "domainMappings",
                  "kind": "added",
                  "after": [{"domain": "acme.test", "verified": true}] },
                { "path": "providers.memoryLayers.episodic",
                  "kind": "removed", "before": null },
            ],
            "summary": {
                "added": 1, "removed": 1, "modified": 1,
                "changedSections": ["domainMappings", "providers"]
            }
        });
        let out = render_diff_unified(&diff);
        assert!(out.contains("~ providers.memoryLayers.semantic.ttl: 3600 → 7200\n"));
        assert!(out.contains("+ domainMappings: [{\"domain\":\"acme.test\",\"verified\":true}]\n"));
        assert!(out.contains("- providers.memoryLayers.episodic: (null)\n"));
    }

    #[test]
    fn test_render_diff_unified_unknown_kind_forward_compat() {
        // A server that adds a new `ChangeKind` variant (e.g.
        // `moved`) must not crash older CLIs. Render a fallback
        // `? path [kind]: ...` line instead.
        let diff = json!({
            "slug": "acme",
            "operation": "update",
            "changes": [
                { "path": "tenant.slug", "kind": "moved",
                  "before": "old", "after": "new" },
            ],
            "summary": {
                "added": 0, "removed": 0, "modified": 0,
                "changedSections": ["tenant"]
            }
        });
        let out = render_diff_unified(&diff);
        assert!(out.contains("? tenant.slug [moved]: before=old after=new\n"));
    }

    #[test]
    fn test_tenant_apply_args_defaults() {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: TenantCommand,
        }

        let parsed = Wrap::try_parse_from(["prog", "apply", "-f", "m.json"]).unwrap();
        match parsed.cmd {
            TenantCommand::Apply(args) => {
                assert_eq!(args.file, "m.json");
                assert!(!args.yes);
                assert!(!args.json);
                assert!(!args.allow_inline);
                assert!(args.target_tenant.is_none());
                assert!(!args.watch);
            }
            _ => panic!("expected Apply variant"),
        }
    }

    #[test]
    fn test_frame_matches_target_bare_kinds() {
        // B2 §7.8 — bare-kind targets must match on `event:` name.
        let frame = SseFrame {
            event: Some("provisioned".into()),
            data: r#"{"slug":"acme","kind":"provisioned"}"#.into(),
        };
        assert!(frame_matches_target(&frame, "provisioned"));
        assert!(!frame_matches_target(&frame, "updated"));
        assert!(!frame_matches_target(&frame, "deactivated"));

        // Default event name (no explicit `event:` field) is
        // `message` — which should NOT match "provisioned".
        let msg_frame = SseFrame {
            event: None,
            data: "{}".into(),
        };
        assert!(!frame_matches_target(&msg_frame, "provisioned"));
    }

    #[test]
    fn test_frame_matches_target_step_prefix_requires_ok() {
        // B2 §7.8 — `step:<name>` must require status==ok. A
        // `started` or `failed` frame for the same step must NOT
        // satisfy the target; otherwise `--watch-until=step:iam`
        // would trip on the first half-millisecond.
        let started = SseFrame {
            event: Some("provisioning_step".into()),
            data:
                r#"{"slug":"acme","kind":{"provisioning_step":{"step":"iam","status":"started"}}}"#
                    .into(),
        };
        assert!(!frame_matches_target(&started, "step:iam"));

        let ok = SseFrame {
            event: Some("provisioning_step".into()),
            data: r#"{"slug":"acme","kind":{"provisioning_step":{"step":"iam","status":"ok"}}}"#
                .into(),
        };
        assert!(frame_matches_target(&ok, "step:iam"));

        let failed = SseFrame {
            event: Some("provisioning_step".into()),
            data:
                r#"{"slug":"acme","kind":{"provisioning_step":{"step":"iam","status":"failed"}}}"#
                    .into(),
        };
        assert!(!frame_matches_target(&failed, "step:iam"));

        // Wrong step name must not match.
        let other_step = SseFrame {
            event: Some("provisioning_step".into()),
            data: r#"{"slug":"acme","kind":{"provisioning_step":{"step":"dns","status":"ok"}}}"#
                .into(),
        };
        assert!(!frame_matches_target(&other_step, "step:iam"));

        // `step:` prefix on a non-step event must not match.
        let provisioned = SseFrame {
            event: Some("provisioned".into()),
            data: r#"{"slug":"acme"}"#.into(),
        };
        assert!(!frame_matches_target(&provisioned, "step:iam"));
    }

    #[test]
    fn test_tenant_apply_watch_until_parses() {
        // B2 §7.8 — `--watch-until=<kind>` must parse as
        // `Option<String>` and default to None. Also verifies it
        // composes with `--watch --watch-timeout` (all three flags
        // together are the expected CI shape for reconciliation
        // flows).
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: TenantCommand,
        }

        // Default: None.
        let parsed = Wrap::try_parse_from(["prog", "apply", "-f", "m.json"]).unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert_eq!(args.watch_until, None);

        // Full combo.
        let parsed = Wrap::try_parse_from([
            "prog",
            "apply",
            "-f",
            "m.json",
            "--yes",
            "--json",
            "--watch",
            "--watch-timeout",
            "60",
            "--watch-until",
            "provisioned",
        ])
        .unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert!(args.watch);
        assert_eq!(args.watch_timeout, 60);
        assert_eq!(args.watch_until.as_deref(), Some("provisioned"));

        // step:<name> form must parse verbatim — no splitting, no
        // special-case in clap.
        let parsed = Wrap::try_parse_from([
            "prog",
            "apply",
            "-f",
            "m.json",
            "--watch",
            "--watch-until",
            "step:iam_sync_complete",
        ])
        .unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert_eq!(args.watch_until.as_deref(), Some("step:iam_sync_complete"));
    }

    #[test]
    fn test_tenant_apply_watch_timeout_parses() {
        // B2 §7.7 — `--watch-timeout=30` must parse as an integer
        // seconds count (NOT a humantime string; we deliberately
        // avoid that dep). Also verifies default is 0 (disabled)
        // and that `--watch-timeout` composes with `--watch`.
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: TenantCommand,
        }

        // Default: 0 means "no stall timeout".
        let parsed = Wrap::try_parse_from(["prog", "apply", "-f", "m.json"]).unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert_eq!(args.watch_timeout, 0);

        // With both flags.
        let parsed = Wrap::try_parse_from([
            "prog",
            "apply",
            "-f",
            "m.json",
            "--yes",
            "--watch",
            "--watch-timeout",
            "45",
        ])
        .unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert!(args.watch);
        assert_eq!(args.watch_timeout, 45);

        // `--watch-timeout` without `--watch` is accepted by clap
        // (we treat it as a no-op at runtime — the flag is only
        // meaningful together with `--watch`). Keeps the clap
        // surface simple.
        let parsed =
            Wrap::try_parse_from(["prog", "apply", "-f", "m.json", "--watch-timeout", "10"])
                .unwrap();
        let TenantCommand::Apply(args) = parsed.cmd else {
            panic!("expected Apply variant");
        };
        assert!(!args.watch);
        assert_eq!(args.watch_timeout, 10);
    }

    #[test]
    fn test_tenant_apply_watch_flag_parses() {
        // B2 §7.6 — `apply --watch` MUST be accepted together with
        // `--yes --json` (the unattended + machine-readable combo a
        // CI pipeline uses to tail lifecycle events while the write
        // is in flight). Guard against regressions that would
        // accidentally make the flags mutually exclusive or reorder
        // them in the clap definition.
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: TenantCommand,
        }

        let parsed = Wrap::try_parse_from([
            "prog",
            "apply",
            "-f",
            "manifest.json",
            "--yes",
            "--json",
            "--watch",
        ])
        .unwrap();
        match parsed.cmd {
            TenantCommand::Apply(args) => {
                assert!(args.watch);
                assert!(args.yes);
                assert!(args.json);
                assert_eq!(args.file, "manifest.json");
            }
            _ => panic!("expected Apply variant"),
        }
    }

    #[test]
    fn test_tenant_apply_and_diff_args_full_shape() {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(subcommand)]
            cmd: TenantCommand,
        }

        let parsed = Wrap::try_parse_from([
            "prog",
            "apply",
            "-f",
            "-",
            "--yes",
            "--json",
            "--allow-inline",
            "--target-tenant",
            "prod",
        ])
        .unwrap();
        match parsed.cmd {
            TenantCommand::Apply(args) => {
                assert_eq!(args.file, "-");
                assert!(args.yes);
                assert!(args.json);
                assert!(args.allow_inline);
                assert_eq!(args.target_tenant.as_deref(), Some("prod"));
            }
            _ => panic!("expected Apply variant"),
        }

        // `--file` is required.
        assert!(Wrap::try_parse_from(["prog", "apply"]).is_err());
        let parsed = Wrap::try_parse_from(["prog", "diff", "-f", "m.json"]).unwrap();
        match parsed.cmd {
            TenantCommand::Diff(args) => {
                assert_eq!(args.file, "m.json");
                assert!(matches!(args.output, TenantDiffFormat::Unified));
                assert!(args.target_tenant.is_none());
            }
            _ => panic!("expected Diff variant"),
        }

        let parsed = Wrap::try_parse_from(["prog", "diff", "-f", "-", "-o", "json"]).unwrap();
        match parsed.cmd {
            TenantCommand::Diff(args) => {
                assert_eq!(args.file, "-");
                assert!(matches!(args.output, TenantDiffFormat::Json));
            }
            _ => panic!("expected Diff variant"),
        }

        // `-f` is required — omitting it is a clap error.
        assert!(Wrap::try_parse_from(["prog", "diff"]).is_err());
    }
}
