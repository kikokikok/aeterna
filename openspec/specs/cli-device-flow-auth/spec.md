# cli-device-flow-auth Specification

## Purpose
TBD - created by archiving change add-cli-device-flow-auth. Update Purpose after archive.
## Requirements
### Requirement: CLI GitHub Device Flow Authentication
The CLI SHALL provide an interactive GitHub Device Flow login as the default authentication method when `aeterna auth login` is invoked without a `--github-token` flag.

The CLI SHALL request a device code from GitHub's device authorization endpoint, display the verification URL and user code to the terminal, and poll for authorization completion. Upon successful authorization, the CLI SHALL exchange the resulting GitHub access token with the Aeterna server's bootstrap endpoint for Aeterna-issued credentials.

#### Scenario: Interactive device flow login
- **WHEN** a user runs `aeterna auth login` without providing `--github-token`
- **THEN** the CLI SHALL initiate a GitHub Device Flow request
- **AND** the CLI SHALL display the verification URL and user code in the terminal
- **AND** the CLI SHALL poll GitHub for authorization completion at the specified interval
- **AND** upon user authorization, the CLI SHALL exchange the GitHub access token with the Aeterna bootstrap endpoint
- **AND** the CLI SHALL persist the resulting credentials to the user's credential store

#### Scenario: Device flow with explicit server URL
- **WHEN** a user runs `aeterna auth login --server-url https://aeterna.example.com`
- **THEN** the CLI SHALL use the provided server URL for the bootstrap token exchange
- **AND** the CLI SHALL save the server URL in the user's profile configuration

#### Scenario: PAT fallback login
- **WHEN** a user runs `aeterna auth login --github-token <PAT>`
- **THEN** the CLI SHALL skip the device flow and directly exchange the PAT with the Aeterna bootstrap endpoint
- **AND** the CLI SHALL persist the resulting credentials identically to the device flow path

#### Scenario: Device flow timeout
- **WHEN** the user does not complete authorization within the GitHub-specified expiry window
- **THEN** the CLI SHALL display an error indicating the device code expired
- **AND** the CLI SHALL suggest re-running `aeterna auth login`

#### Scenario: Device flow denied
- **WHEN** the user explicitly denies authorization in the browser
- **THEN** the CLI SHALL display an error indicating authorization was denied
- **AND** the CLI SHALL exit with a non-zero exit code

### Requirement: CLI Auth Command Registration
The CLI SHALL expose `aeterna auth` as a top-level subcommand with `login`, `logout`, and `status` subcommands.

#### Scenario: Auth subcommand is accessible
- **WHEN** a user runs `aeterna auth --help`
- **THEN** the CLI SHALL display help for `login`, `logout`, and `status` subcommands

#### Scenario: Auth status shows current session
- **WHEN** a user runs `aeterna auth status`
- **THEN** the CLI SHALL display the current authentication state including profile, server URL, GitHub login, and token expiry

### Requirement: CLI Automatic Token Refresh
The CLI SHALL automatically refresh expired Aeterna credentials before making authenticated API calls when a valid refresh token is available.

#### Scenario: Transparent refresh on expired access token
- **WHEN** the CLI detects that the stored access token has expired and a refresh token is available
- **THEN** the CLI SHALL call the Aeterna refresh endpoint to obtain new credentials
- **AND** the CLI SHALL persist the refreshed credentials
- **AND** the original API call SHALL proceed with the new access token

#### Scenario: Re-login required when refresh fails
- **WHEN** token refresh fails because the refresh token is revoked, expired, or invalid
- **THEN** the CLI SHALL display an error indicating re-authentication is needed
- **AND** the CLI SHALL suggest running `aeterna auth login`

### Requirement: CLI GitHub Client ID Configuration
The CLI SHALL support configuring the GitHub App client_id used for device flow authentication via profile configuration or environment variable.

#### Scenario: Client ID from environment
- **WHEN** the environment variable `AETERNA_GITHUB_CLIENT_ID` is set
- **THEN** the CLI SHALL use that value as the GitHub client_id for device flow requests

#### Scenario: Client ID from profile config
- **WHEN** the profile configuration includes a `github_client_id` field
- **THEN** the CLI SHALL use that value for device flow requests
- **AND** the environment variable SHALL take precedence over the profile config

