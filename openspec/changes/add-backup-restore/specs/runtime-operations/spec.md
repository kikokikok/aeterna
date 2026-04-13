## MODIFIED Requirements

### Requirement: Honest Runtime Command Behavior
The system SHALL ensure user-facing CLI and service operations either execute real backend behavior or fail explicitly as unsupported.

#### Scenario: Export command executes real backend-backed operation
- **WHEN** a user invokes `aeterna admin export` with a reachable configured server
- **THEN** the command SHALL initiate a real server-side export job
- **AND** the command SHALL poll for job completion and download the resulting archive
- **AND** the command SHALL NOT return simulated export data or placeholder success

#### Scenario: Import command executes real backend-backed operation
- **WHEN** a user invokes `aeterna admin import` with a reachable configured server
- **THEN** the command SHALL upload the archive and initiate a real server-side import job
- **AND** the command SHALL display the actual validation or import report from the server
- **AND** the command SHALL NOT return simulated conflict analysis or placeholder import results

## ADDED Requirements

### Requirement: Backup Validation Command
The system SHALL provide an offline archive validation command that does not require a server connection.

#### Scenario: Offline archive validation
- **WHEN** a user runs `aeterna admin backup validate <archive>`
- **THEN** the command SHALL verify archive structure, manifest schema, and per-file checksums locally
- **AND** the command SHALL report integrity violations without requiring a server connection
- **AND** the command SHALL exit with a non-zero status code if any validation fails

### Requirement: Export and Import Progress Display
The system SHALL display real-time progress for long-running export and import operations.

#### Scenario: Export progress display
- **WHEN** a user runs `aeterna admin export` and the job is running
- **THEN** the CLI SHALL poll the server for job status and display progress including percentage complete, entity counts per type, and elapsed time

#### Scenario: Import progress display
- **WHEN** a user runs `aeterna admin import` and the job is running
- **THEN** the CLI SHALL poll the server for job status and display progress including percentage complete, entities processed, and conflicts detected
