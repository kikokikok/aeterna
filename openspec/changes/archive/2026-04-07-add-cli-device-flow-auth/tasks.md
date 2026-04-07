## 1. Wire Auth into CLI

- [x] 1.1 Add `pub mod auth;` to `cli/src/commands/mod.rs`
- [x] 1.2 Add `Auth(auth::AuthCommand)` variant to the `Commands` enum in `cli/src/commands/mod.rs`
- [x] 1.3 Add match arm for `Commands::Auth(cmd) => auth::run(cmd).await` in the CLI dispatch (main.rs or equivalent)
- [x] 1.4 Verify `aeterna auth --help` shows login/logout/status

## 2. GitHub Device Flow Implementation

- [x] 2.1 Add `github_client_id` field to `Profile` struct in `cli/src/profile.rs`
- [x] 2.2 Add `AETERNA_GITHUB_CLIENT_ID` env var reading — env takes precedence over profile config
- [x] 2.3 Implement `device_flow_login()` in `cli/src/client.rs`:
  - POST `https://github.com/login/device/code` with `client_id` and `scope=read:user,user:email`
  - Parse response: `device_code`, `user_code`, `verification_uri`, `expires_in`, `interval`
- [x] 2.4 Implement `poll_device_authorization()` in `cli/src/client.rs`:
  - POST `https://github.com/login/oauth/access_token` with `client_id`, `device_code`, `grant_type=urn:ietf:params:oauth:grant-type:device_code`
  - Handle: `authorization_pending` (continue), `slow_down` (increase interval), `expired_token`, `access_denied`
  - Return GitHub access token on success
- [x] 2.5 Update `run_login()` in `cli/src/commands/auth.rs`:
  - If `--github-token` provided → use PAT exchange (existing path)
  - Otherwise → call `device_flow_login()`, display user code + URL, call `poll_device_authorization()`, exchange result via `bootstrap_github()`
- [x] 2.6 Add terminal UX: display verification URL and user code clearly, show spinner while polling, open browser if possible (`open` on macOS / `xdg-open` on Linux)
- [x] 2.7 Write unit tests for device flow response parsing and polling state machine

## 3. Automatic Token Refresh

- [x] 3.1 Add `ensure_valid_token()` method to credential/profile resolution that checks expiry and auto-refreshes
- [x] 3.2 Integrate auto-refresh into `AeternaClient::new()` or a request middleware so all authenticated calls benefit
- [x] 3.3 On refresh failure, return a clear error with `aeterna auth login` hint
- [x] 3.4 Write tests for refresh logic (expired → refresh success, expired → refresh failure)

## 4. Build and Verify

- [x] 4.1 Run `cargo build -p aeterna` to verify compilation
- [x] 4.2 Run `cargo test -p aeterna` to verify tests pass
- [x] 4.3 Manual test: run `aeterna auth login --server-url https://[REDACTED_HOST]` and complete device flow
- [x] 4.4 Manual test: run `aeterna auth status` to verify stored credentials
- [ ] 4.5 Manual test: verify auto-refresh works by letting token expire and running an authenticated command
