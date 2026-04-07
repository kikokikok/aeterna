## 1. CLI auth and profile foundation

- [x] 1.1 Add CLI auth command surface (`login`, `logout`, `status`) under the `aeterna` binary.
- [x] 1.2 Implement a shared authenticated CLI client that resolves target profile, server URL, and credentials.
- [x] 1.3 Implement secure credential persistence and documented fallback storage behavior.
- [ ] 1.4 Add token refresh/session continuation support for CLI-authenticated usage.

## 2. Configuration and environment management

- [x] 2.1 Define canonical user-level and project-level CLI config file locations.
- [x] 2.2 Add CLI config/profile management commands for show, set/update, validate, and default-profile selection.
  - [x] 2.2.1 `aeterna config show/set/validate/default-profile` commands (config.rs)
  - [x] 2.2.2 `aeterna profile add` — interactive wizard, `--from-file` manifest (TOML), CLI flags
  - [x] 2.2.3 `aeterna profile update` — merge flags/file into existing profile
  - [x] 2.2.4 `aeterna profile remove` — with confirmation prompt and `--yes` flag
  - [x] 2.2.5 `aeterna profile list` — tabular display with default marker
  - [x] 2.2.6 `aeterna profile default` — set default profile
  - [x] 2.2.7 Register Auth, Config, Profile commands in Commands enum and main.rs dispatch
  - [x] 2.2.8 Add `github_client_id` field to Profile struct for device flow auth
  - [x] 2.2.9 Add `delete_profile`, `list_profiles`, `default_profile_name` helpers to profile.rs
  - [x] 2.2.10 Unit tests for profile commands (18 tests passing)
- [ ] 2.3 Align configuration loading code and docs on the `AETERNA_*` namespace and canonical precedence rules.
- [ ] 2.4 Update errors and status output to reference the canonical config/profile model.

## 3. Convert stubbed command groups to real backend-backed behavior

- [ ] 3.1 Wire memory commands to real backend requests or explicit unsupported errors.
- [ ] 3.2 Wire knowledge commands to real backend requests or explicit unsupported errors.
- [ ] 3.3 Wire sync commands and offline/deferred behavior through the shared client layer.
- [ ] 3.4 Replace simulated governance command behavior with real backend-backed execution or explicit unsupported errors.
- [ ] 3.5 Replace stubbed user/org/team/agent/admin flows with real backend-backed execution where supported.
- [ ] 3.6 Make `admin health` and related status commands consume real runtime endpoints instead of fabricated results.

## 4. Code-search and control-plane UX cleanup

- [ ] 4.1 Replace the dead legacy `code-search` shell behavior with a supported backend path or explicit unsupported contract.
- [ ] 4.2 Integrate the existing offline client infrastructure into the supported CLI control-plane behavior.
- [ ] 4.3 Ensure all backend-facing commands use consistent target/profile/auth output and exit semantics.

## 5. Packaging, docs, and end-to-end scenarios

- [ ] 5.1 Define and implement supported release artifacts/package-manager paths for macOS and Linux CLI installation.
- [ ] 5.2 Update user-facing docs to cover install, login, target selection, config management, and command usage end to end.
- [ ] 5.3 Add end-to-end scenario coverage for first-time user onboarding, daily authenticated usage, and operator/admin flows.
- [ ] 5.4 Validate that CLI docs, release outputs, and command behavior stay consistent across supported environments.
