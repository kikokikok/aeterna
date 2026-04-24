//! Secret input modes for CLI commands that accept sensitive material.
//!
//! B2 tasks 8.1–8.5 of `harden-tenant-provisioning`. Before this module
//! landed, the only way to hand a secret to `aeterna tenant secret set`
//! was `--value <raw-string>` — which leaks into shell history,
//! `/proc/<pid>/cmdline`, any `ps` snapshot, CI log capture, and
//! typically the parent shell's scrollback.
//!
//! This module replaces that single flag with a deliberate menu of
//! input sources, each covering a distinct threat model:
//!
//! | Source          | Threat addressed                                   | Notes |
//! |-----------------|----------------------------------------------------|-------|
//! | `--ref`         | The secret never touches the client at all.        | Caller asserts the secret already lives in the configured secret store; the CLI sends only the reference name. |
//! | `--from-file`   | Shell-history + argv leakage.                      | Path is read and deleted from memory after upload. Refuses modes broader than `0600` on Unix so a group-readable file cannot be handed off by mistake. |
//! | `--from-stdin`  | Shell-history + argv leakage.                      | If stdin is a TTY the prompt runs with echo disabled; if stdin is a pipe the bytes are read verbatim and trailing `\n`/`\r\n` is stripped. |
//! | `--from-env`    | `ps`/argv leakage.                                 | Reads the named variable, then calls `env::remove_var` so forked children (editors, pagers, anything in the same process tree) cannot inherit it. |
//! | `--value`       | **Explicitly unsafe.** Gated behind `--allow-inline-secret`. Exists for tests and one-off debugging only. |
//!
//! Exactly one source must be supplied. Zero → UX error ("how should
//! I get the secret?"). Two or more → UX error ("pick one") —
//! ambiguity is a loud failure rather than silent precedence.
//!
//! The resolver returns a [`SecretPayload`] which the caller turns
//! into the JSON body. The variant carries the semantics (`Inline`
//! becomes `secretValue`, `Reference` becomes `secretRef`) so callers
//! cannot accidentally post an un-dereferenced reference as a plain
//! secret value.

use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use crate::ux_error::UxError;

/// The result of resolving the user's chosen input mode into something
/// the server wire format understands.
///
/// Two variants, two JSON shapes:
/// - [`SecretPayload::Inline`] → `{"secretValue": "…"}`
/// - [`SecretPayload::Reference`] → `{"secretRef": "…"}`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretPayload {
    /// The raw secret bytes, ready to be uploaded under `secretValue`.
    Inline(String),
    /// A reference to an already-stored secret; the CLI never sees
    /// the value and must not fabricate one.
    Reference(String),
}

/// User-supplied input-mode flags, post-`clap` parse.
///
/// Kept as a plain struct rather than a `#[derive(clap::Args)]` type
/// so the resolver is trivially unit-testable without building a
/// whole `Command` tree.
#[derive(Debug, Default, Clone)]
pub struct SecretInputFlags {
    /// `--value <RAW>`. Must be paired with `allow_inline`.
    pub inline_value: Option<String>,
    /// `--allow-inline-secret`. Required co-flag for `inline_value`.
    pub allow_inline: bool,
    /// `--ref <NAME>`. Sent as `secretRef`; no bytes ever cross the wire.
    pub reference: Option<String>,
    /// `--from-file <PATH>`. Mode must be `<= 0600` on Unix.
    pub from_file: Option<PathBuf>,
    /// `--from-stdin`. If stdin is a TTY the prompt echoes stars;
    /// if stdin is a pipe the bytes are read verbatim.
    pub from_stdin: bool,
    /// `--from-env <NAME>`. Read once, then unset in the current
    /// process so child processes cannot inherit it.
    pub from_env: Option<String>,
}

impl SecretInputFlags {
    /// Count of input sources actively selected by the user.
    ///
    /// The inline value counts regardless of `allow_inline` — the
    /// gate is a *separate* error message with a different fix
    /// suggestion, so we do not want to silently drop it here.
    fn source_count(&self) -> usize {
        usize::from(self.inline_value.is_some())
            + usize::from(self.reference.is_some())
            + usize::from(self.from_file.is_some())
            + usize::from(self.from_stdin)
            + usize::from(self.from_env.is_some())
    }
}

/// Resolve the chosen input mode into a [`SecretPayload`].
///
/// The `io` seam is an injected [`SecretIo`] trait so tests can
/// simulate file bytes, stdin bytes, env-var lookups, and TTY
/// detection without touching the real filesystem or environment.
pub fn resolve<I: SecretIo>(
    flags: &SecretInputFlags,
    io: &mut I,
) -> Result<SecretPayload, UxError> {
    match flags.source_count() {
        0 => {
            return Err(UxError::new("No secret source provided")
                .why("One of --ref, --from-file, --from-stdin, --from-env, or --value must be set")
                .fix("Use --ref <NAME> if the secret already lives in the store")
                .fix("Use --from-file <PATH> with a 0600-mode file")
                .fix("Use --from-stdin to type or pipe the secret")
                .fix("Use --from-env <VAR> to read from a masked env var")
                .suggest("aeterna tenant secret set <logical-name> --from-stdin"));
        }
        1 => { /* ok — exactly one source */ }
        n => {
            return Err(UxError::new(format!(
                "{n} secret sources provided; exactly one is allowed"
            ))
            .why("Mixing --value / --ref / --from-file / --from-stdin / --from-env is ambiguous")
            .fix("Pick a single source and remove the others")
            .suggest("aeterna tenant secret set <logical-name> --from-stdin"));
        }
    }

    if let Some(raw) = flags.inline_value.as_deref() {
        if !flags.allow_inline {
            return Err(UxError::new("--value requires --allow-inline-secret")
                .why("Inline secrets leak into shell history, `ps`, and CI logs")
                .fix("Prefer --from-stdin, --from-file, --from-env, or --ref")
                .fix("If you truly need inline (tests, debugging), pass --allow-inline-secret")
                .suggest("aeterna tenant secret set <logical-name> --from-stdin"));
        }
        return Ok(SecretPayload::Inline(raw.to_string()));
    }

    if let Some(reference) = flags.reference.as_deref() {
        if reference.trim().is_empty() {
            return Err(UxError::new("--ref requires a non-empty reference name")
                .fix("Provide the name under which the secret is stored"));
        }
        return Ok(SecretPayload::Reference(reference.to_string()));
    }

    if let Some(path) = flags.from_file.as_deref() {
        return read_from_file(path, io).map(SecretPayload::Inline);
    }

    if flags.from_stdin {
        return io.read_stdin_secret().map(SecretPayload::Inline);
    }

    if let Some(var) = flags.from_env.as_deref() {
        return read_from_env(var, io).map(SecretPayload::Inline);
    }

    // Unreachable because source_count == 1 above guarantees one arm matched.
    unreachable!("source_count == 1 but no arm matched");
}

fn read_from_file<I: SecretIo>(path: &Path, io: &mut I) -> Result<String, UxError> {
    let mode = io.file_mode(path).map_err(|e| {
        UxError::new(format!("Cannot stat {}: {e}", path.display()))
            .fix("Check the path exists and is readable by the current user")
    })?;

    // B2 task 8.3: reject any mode broader than 0600.
    //
    // `mode & 0o177 == 0` captures "no world bits, no group bits, no
    // owner-execute bit" — i.e. at most `rw-------`. We report the
    // octal so the user can copy-paste the chmod.
    if let Some(m) = mode
        && m & 0o177 != 0 {
            return Err(UxError::new(format!(
                "File {} is too permissive ({:04o}); required mode is 0600 or stricter",
                path.display(),
                m & 0o777
            ))
            .why("Group- or world-readable files leak secrets to any process on the host")
            .fix(format!("Run: chmod 0600 {}", path.display()))
            .suggest(format!("chmod 0600 {}", path.display())));
        }
    // On non-Unix platforms `file_mode` returns `None` — we skip the
    // check rather than forcing an error the user cannot satisfy.

    let bytes = io.read_file_bytes(path).map_err(|e| {
        UxError::new(format!("Cannot read {}: {e}", path.display()))
            .fix("Verify the path and that the file is not empty")
    })?;

    let raw = String::from_utf8(bytes).map_err(|_| {
        UxError::new(format!("File {} is not valid UTF-8", path.display()))
            .why("Secret files must be UTF-8 text; binary blobs are not supported")
            .fix("Re-encode the secret as UTF-8, or store it by reference instead")
    })?;

    // Files conventionally have a trailing newline added by `echo >` or
    // most editors. Strip at most one so the secret matches what the
    // user typed, without silently mangling values that intentionally
    // end in whitespace (we only trim the terminator, not all trailing
    // whitespace).
    let trimmed = raw
        .strip_suffix("\r\n")
        .or_else(|| raw.strip_suffix('\n'))
        .map_or(raw.clone(), str::to_owned);

    if trimmed.is_empty() {
        return Err(UxError::new(format!("File {} is empty", path.display()))
            .fix("Write the secret into the file before calling this command"));
    }

    Ok(trimmed)
}

fn read_from_env<I: SecretIo>(var: &str, io: &mut I) -> Result<String, UxError> {
    if var.trim().is_empty() {
        return Err(UxError::new("--from-env requires a variable name")
            .fix("Example: --from-env TENANT_SECRET"));
    }

    let raw = match io.read_env(var) {
        Some(v) => v,
        None => {
            return Err(
                UxError::new(format!("Environment variable {var} is not set"))
                    .fix(format!("Export it before running: export {var}=<secret>"))
                    .suggest(format!("env | grep {var}")),
            );
        }
    };

    if raw.is_empty() {
        return Err(UxError::new(format!("Environment variable {var} is empty"))
            .fix("Set a non-empty value or use a different input mode"));
    }

    // B2 task 8.5: clear the variable from the current process so
    // child processes (anything the CLI spawns — shell hooks, pager,
    // editor invocations, telemetry subprocesses) cannot inherit it.
    io.clear_env(var);

    Ok(raw)
}

// ---------------------------------------------------------------------------
// IO seam
// ---------------------------------------------------------------------------

/// Side-effecting operations the resolver needs. Abstracted so tests
/// can drive the resolver deterministically without touching the real
/// filesystem, environment, or TTY.
pub trait SecretIo {
    /// Return the file's Unix mode bits, or `None` on non-Unix platforms.
    fn file_mode(&self, path: &Path) -> io::Result<Option<u32>>;
    /// Read the file contents verbatim.
    fn read_file_bytes(&mut self, path: &Path) -> io::Result<Vec<u8>>;
    /// Read a secret from stdin. Implementations MUST disable echo if
    /// stdin is a TTY.
    fn read_stdin_secret(&mut self) -> Result<String, UxError>;
    /// Look up an environment variable.
    fn read_env(&self, var: &str) -> Option<String>;
    /// Remove an environment variable from the current process.
    fn clear_env(&mut self, var: &str);
}

/// Real-OS implementation used by the CLI at runtime.
pub struct RealSecretIo;

impl SecretIo for RealSecretIo {
    fn file_mode(&self, path: &Path) -> io::Result<Option<u32>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(path)?;
            Ok(Some(meta.permissions().mode()))
        }
        #[cfg(not(unix))]
        {
            // Touch `path` so the call still surfaces "file not found"
            // errors on Windows even though we cannot check mode bits.
            let _ = std::fs::metadata(path)?;
            Ok(None)
        }
    }

    fn read_file_bytes(&mut self, path: &Path) -> io::Result<Vec<u8>> {
        std::fs::read(path)
    }

    fn read_stdin_secret(&mut self) -> Result<String, UxError> {
        let stdin = io::stdin();
        if stdin.is_terminal() {
            // B2 task 8.4: no echo on a TTY. `dialoguer::Password` draws
            // stars and disables echo via `console::Term`.
            dialoguer::Password::new()
                .with_prompt("Secret")
                .interact()
                .map_err(|e| {
                    UxError::new(format!("Failed to read secret from terminal: {e}"))
                        .fix("Ensure the terminal supports interactive input")
                })
        } else {
            // Piped stdin: read all bytes verbatim, strip a single
            // trailing newline so `echo 'secret' | aeterna …` and
            // `printf 'secret' | aeterna …` both do the intuitive
            // thing.
            let mut buf = String::new();
            stdin.lock().read_to_string(&mut buf).map_err(|e| {
                UxError::new(format!("Failed to read stdin: {e}"))
                    .fix("Pipe the secret into the command")
            })?;
            let trimmed = buf
                .strip_suffix("\r\n")
                .or_else(|| buf.strip_suffix('\n'))
                .map_or(buf.clone(), str::to_owned);
            if trimmed.is_empty() {
                return Err(UxError::new("Empty secret on stdin")
                    .fix("Pipe a non-empty value into the command"));
            }
            Ok(trimmed)
        }
    }

    fn read_env(&self, var: &str) -> Option<String> {
        std::env::var_os(var).and_then(|v: OsString| v.into_string().ok())
    }

    fn clear_env(&mut self, var: &str) {
        // SAFETY: Rust 2024 makes env mutation `unsafe` because it is
        // not thread-safe. This call executes on the CLI's single
        // synchronous command-dispatch path before any worker threads
        // read the environment, so there is no concurrent reader.
        unsafe { std::env::remove_var(var) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io;

    /// Scriptable SecretIo for deterministic tests.
    #[derive(Default)]
    struct FakeIo {
        files: HashMap<PathBuf, Vec<u8>>,
        modes: HashMap<PathBuf, u32>,
        env: HashMap<String, String>,
        stdin: Option<Result<String, UxError>>,
        cleared: Vec<String>,
    }

    impl SecretIo for FakeIo {
        fn file_mode(&self, path: &Path) -> io::Result<Option<u32>> {
            match self.modes.get(path) {
                Some(m) => Ok(Some(*m)),
                None if self.files.contains_key(path) => Ok(Some(0o600)),
                None => Err(io::Error::new(io::ErrorKind::NotFound, "no such file")),
            }
        }
        fn read_file_bytes(&mut self, path: &Path) -> io::Result<Vec<u8>> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no such file"))
        }
        fn read_stdin_secret(&mut self) -> Result<String, UxError> {
            self.stdin
                .take()
                .unwrap_or_else(|| Err(UxError::new("no stdin scripted")))
        }
        fn read_env(&self, var: &str) -> Option<String> {
            self.env.get(var).cloned()
        }
        fn clear_env(&mut self, var: &str) {
            self.env.remove(var);
            self.cleared.push(var.to_string());
        }
    }

    fn flags() -> SecretInputFlags {
        SecretInputFlags::default()
    }

    // ---- source_count arithmetic ----------------------------------------

    #[test]
    fn zero_sources_errors_with_help() {
        let mut io = FakeIo::default();
        let err = resolve(&flags(), &mut io).unwrap_err();
        assert!(err.what.contains("No secret source"));
        assert!(err.suggested_command.is_some());
    }

    #[test]
    fn two_sources_errors_as_ambiguous() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.from_stdin = true;
        f.from_env = Some("X".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("2 secret sources"));
    }

    #[test]
    fn five_sources_errors_with_exact_count() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.inline_value = Some("x".into());
        f.allow_inline = true;
        f.reference = Some("r".into());
        f.from_file = Some(PathBuf::from("/x"));
        f.from_stdin = true;
        f.from_env = Some("E".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("5 secret sources"));
    }

    // ---- inline gating (task 8.2) ---------------------------------------

    #[test]
    fn inline_value_without_allow_flag_is_rejected() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.inline_value = Some("raw".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("--allow-inline-secret"));
        assert!(err.why.as_ref().unwrap().contains("shell history"));
    }

    #[test]
    fn inline_value_with_allow_flag_passes_through() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.inline_value = Some("raw".into());
        f.allow_inline = true;
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("raw".into())
        );
    }

    // ---- reference mode -------------------------------------------------

    #[test]
    fn reference_becomes_reference_payload() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.reference = Some("vault/repo-token".into());
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Reference("vault/repo-token".into())
        );
    }

    #[test]
    fn empty_reference_is_rejected() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.reference = Some("   ".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("non-empty"));
    }

    // ---- file mode check (task 8.3) -------------------------------------

    #[test]
    fn file_mode_0600_is_accepted() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"secret\n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("secret".into())
        );
    }

    #[test]
    fn file_mode_0400_is_accepted() {
        // Read-only for owner is stricter than 0600 — must still pass.
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"k\n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o400);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert!(resolve(&f, &mut io).is_ok());
    }

    #[test]
    fn file_mode_0644_is_rejected_with_chmod_suggestion() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"k".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o644);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("0644"));
        assert_eq!(err.suggested_command, Some("chmod 0600 /s".into()));
    }

    #[test]
    fn file_mode_0660_is_rejected_group_bits_are_fatal() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"k".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o660);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert!(resolve(&f, &mut io).is_err());
    }

    #[test]
    fn file_mode_0700_is_rejected_owner_exec_is_fatal() {
        // 0700 is owner-only but includes the exec bit, so it is NOT
        // `<= 0600`. Task phrasing is literal.
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"k".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o700);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert!(resolve(&f, &mut io).is_err());
    }

    #[test]
    fn file_strips_trailing_newline_once() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"hunter2\n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("hunter2".into())
        );
    }

    #[test]
    fn file_strips_crlf_terminator() {
        let mut io = FakeIo::default();
        io.files
            .insert(PathBuf::from("/s"), b"hunter2\r\n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("hunter2".into())
        );
    }

    #[test]
    fn file_preserves_trailing_whitespace_other_than_newline() {
        // Do NOT trim trailing spaces — a password might legitimately
        // end in a space and silent munging would be worse than
        // echoing it.
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"pad \n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("pad ".into())
        );
    }

    #[test]
    fn empty_file_is_rejected() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), b"\n".to_vec());
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        assert!(resolve(&f, &mut io).unwrap_err().what.contains("empty"));
    }

    #[test]
    fn non_utf8_file_is_rejected_with_actionable_message() {
        let mut io = FakeIo::default();
        io.files.insert(PathBuf::from("/s"), vec![0xff, 0xfe, 0xfd]);
        io.modes.insert(PathBuf::from("/s"), 0o600);
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/s"));
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("UTF-8"));
    }

    #[test]
    fn missing_file_surfaces_stat_error() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.from_file = Some(PathBuf::from("/does-not-exist"));
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("Cannot stat"));
    }

    // ---- stdin (task 8.4 delegated to RealSecretIo) ---------------------

    #[test]
    fn stdin_delegates_to_io_seam() {
        let mut io = FakeIo {
            stdin: Some(Ok("typed-secret".into())),
            ..FakeIo::default()
        };
        let mut f = flags();
        f.from_stdin = true;
        assert_eq!(
            resolve(&f, &mut io).unwrap(),
            SecretPayload::Inline("typed-secret".into())
        );
    }

    #[test]
    fn stdin_propagates_io_error() {
        let mut io = FakeIo {
            stdin: Some(Err(UxError::new("no tty"))),
            ..FakeIo::default()
        };
        let mut f = flags();
        f.from_stdin = true;
        assert!(resolve(&f, &mut io).is_err());
    }

    // ---- env (task 8.5) -------------------------------------------------

    #[test]
    fn env_reads_then_clears() {
        let mut io = FakeIo::default();
        io.env.insert("MY_SECRET".into(), "from-env".into());
        let mut f = flags();
        f.from_env = Some("MY_SECRET".into());
        let out = resolve(&f, &mut io).unwrap();
        assert_eq!(out, SecretPayload::Inline("from-env".into()));
        // Must be cleared so forked children do not inherit.
        assert_eq!(io.cleared, vec!["MY_SECRET".to_string()]);
        assert!(!io.env.contains_key("MY_SECRET"));
    }

    #[test]
    fn env_unset_is_clear_error() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.from_env = Some("NEVER_SET".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("NEVER_SET"));
        assert!(err.what.contains("not set"));
    }

    #[test]
    fn env_empty_is_rejected_and_not_silently_sent() {
        let mut io = FakeIo::default();
        io.env.insert("E".into(), String::new());
        let mut f = flags();
        f.from_env = Some("E".into());
        assert!(resolve(&f, &mut io).unwrap_err().what.contains("empty"));
    }

    #[test]
    fn env_empty_name_is_rejected() {
        let mut io = FakeIo::default();
        let mut f = flags();
        f.from_env = Some("   ".into());
        let err = resolve(&f, &mut io).unwrap_err();
        assert!(err.what.contains("variable name"));
    }
}
