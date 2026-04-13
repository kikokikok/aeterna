---
title: Backup and Restore Specification
status: draft
version: 0.1.0
created: 2026-04-11
authors:
  - AI Systems Architecture Team
related:
  - memory-system
  - knowledge-repository
  - storage
  - deployment
  - runtime-operations
---

## Purpose

The Backup and Restore capability provides enterprise-grade data portability, disaster recovery, and tenant migration for Aeterna deployments. It enables operators to create portable, integrity-verified, point-in-time archives of all persistent Aeterna data and restore them into the same or a different instance — while the server remains online and serving live traffic.

## Requirements

### Requirement: Portable Archive Format
The system SHALL produce self-describing, integrity-verified archives in a documented portable format.

#### Scenario: Archive structure
- **WHEN** an export job completes successfully
- **THEN** the system SHALL produce a gzip-compressed tar archive containing a `manifest.json`, one NDJSON file per exported entity type, and a `checksums.sha256` file
- **AND** the `manifest.json` SHALL include the archive schema version, creation timestamp, export scope, source instance identifier, entity counts per file, SHA256 checksums per file, embedding model metadata, and per-backend snapshot timestamps

#### Scenario: NDJSON entity serialization
- **WHEN** the system serializes entity data for export
- **THEN** each entity type (memories, knowledge, graph nodes, graph edges, policies, org units, roles, promotions, relations, governance events) SHALL be written as one JSON object per line in its respective NDJSON file
- **AND** MemoryEntry records SHALL include the embedding vector as a JSON array of floats when present
- **AND** each line SHALL be a self-contained valid JSON object

#### Scenario: Archive integrity verification
- **WHEN** the system opens an archive for import
- **THEN** it SHALL verify every file's SHA256 checksum against the value in `checksums.sha256`
- **AND** it SHALL reject the archive if any checksum does not match
- **AND** it SHALL reject the archive if the `manifest.json` is missing or unparseable

#### Scenario: Schema version compatibility
- **WHEN** the system reads an archive manifest
- **THEN** it SHALL verify the archive schema version is compatible with the current system version
- **AND** it SHALL reject archives with incompatible schema versions with a clear error identifying the version mismatch

### Requirement: Server-Side Export Lifecycle
The system SHALL support asynchronous server-side export jobs that produce downloadable archives.

#### Scenario: Initiate export job
- **WHEN** an authorized user submits an export request with scope, target entities, and optional filters
- **THEN** the system SHALL create a background export job and return a job identifier
- **AND** the job SHALL begin reading data from the configured storage backends

#### Scenario: Poll export job status
- **WHEN** a user queries the status of an export job by its identifier
- **THEN** the system SHALL return the current job state (pending, running, completed, failed), progress percentage, entity counts processed so far, and per-backend snapshot timestamps

#### Scenario: Download completed export
- **WHEN** a user requests the archive for a completed export job
- **THEN** the system SHALL stream the archive file to the client
- **AND** the system SHALL NOT serve archives for jobs that have not completed successfully

#### Scenario: Cancel export job
- **WHEN** a user cancels a running export job
- **THEN** the system SHALL stop all read operations, release database connections back to the export pool, delete the Qdrant snapshot if one was created, clean up any temporary archive files, and transition the job to cancelled state

#### Scenario: Export job failure
- **WHEN** an export job encounters an unrecoverable error
- **THEN** the job status SHALL transition to failed with a descriptive error message
- **AND** partial archive data SHALL NOT be served as a completed export
- **AND** the system SHALL clean up temporary files and release all resources

### Requirement: Server-Side Import Lifecycle
The system SHALL support asynchronous server-side import jobs with validation, dry-run, and confirmation stages.

#### Scenario: Upload and initiate import
- **WHEN** an authorized user uploads an archive and specifies import mode (merge, replace, or skip-existing)
- **THEN** the system SHALL validate the archive integrity and schema version
- **AND** the system SHALL create a background import job and return a job identifier

#### Scenario: Dry-run validation
- **WHEN** an import job is initiated with dry-run enabled
- **THEN** the system SHALL validate the archive, detect conflicts against existing data, and produce a detailed validation report
- **AND** the system SHALL NOT write any data to storage backends during dry-run
- **AND** the report SHALL list each conflict with entity type, identifier, and reason

#### Scenario: Confirm import after dry-run
- **WHEN** a user confirms an import job that was previously run in dry-run mode
- **THEN** the system SHALL execute the import with the same parameters and write data to storage backends
- **AND** the system SHALL apply the specified conflict resolution mode

#### Scenario: Import conflict resolution in merge mode
- **WHEN** an import in merge mode encounters an entity that already exists in the target
- **THEN** the system SHALL keep the record with the more recent `updated_at` timestamp
- **AND** the system SHALL log the resolution decision in the import report

#### Scenario: Import conflict resolution in replace mode
- **WHEN** an import in replace mode encounters an entity that already exists in the target
- **THEN** the system SHALL overwrite the existing entity with the imported version
- **AND** replace mode SHALL require the `--force` flag or prior dry-run confirmation

#### Scenario: Import conflict resolution in skip-existing mode
- **WHEN** an import in skip-existing mode encounters an entity that already exists in the target
- **THEN** the system SHALL skip the imported entity and retain the existing version
- **AND** the system SHALL log the skipped entity in the import report

#### Scenario: Import failure and rollback
- **WHEN** an import job encounters an unrecoverable error during data writing
- **THEN** the system SHALL roll back all changes made by the current import job where the storage backend supports transactions
- **AND** the system SHALL report the failure with the error details and the count of entities that were rolled back

### Requirement: Online Export with Minimal Live Impact
The system SHALL execute export operations while the server is running and serving live traffic, with bounded resource consumption.

#### Scenario: Dedicated export connection pool
- **WHEN** the server initializes the backup subsystem
- **THEN** the system SHALL create a dedicated PostgreSQL connection pool for export operations, separate from the main application pool
- **AND** the export pool size SHALL be configurable with a default of 2 connections

#### Scenario: Streaming cursor-based reads
- **WHEN** an export job reads data from PostgreSQL
- **THEN** the system SHALL use `DECLARE CURSOR ... FETCH N` with a configurable fetch size (default 1000 rows) to read data in bounded batches
- **AND** the system SHALL NOT load the entire result set into memory at once

#### Scenario: Non-blocking Qdrant export
- **WHEN** an export job reads vector data from Qdrant
- **THEN** the system SHALL create a Qdrant collection snapshot without blocking live write operations
- **AND** the system SHALL use the scroll API with pagination (configurable batch size, default 100 points) to read from the snapshot

#### Scenario: Resource throttling
- **WHEN** an export job is running
- **THEN** the system SHALL respect configurable concurrency limits for concurrent backend reads
- **AND** the system SHALL bound peak memory usage proportional to batch size, not dataset size

### Requirement: Multi-Backend Snapshot Consistency
The system SHALL coordinate export reads across storage backends to produce practically consistent snapshots.

#### Scenario: Coordinated export read
- **WHEN** an export job reads data from PostgreSQL, Qdrant, and DuckDB
- **THEN** the system SHALL open a PostgreSQL REPEATABLE READ transaction for the duration of PG reads
- **AND** the system SHALL capture a Qdrant collection snapshot at the start of the export job
- **AND** the system SHALL read DuckDB graph data within the same job context
- **AND** the manifest SHALL record the snapshot timestamps for each backend

### Requirement: Tenant-Scoped Export and Import
The system SHALL enforce tenant isolation during export and import operations.

#### Scenario: Tenant-scoped export
- **WHEN** a TenantAdmin user initiates an export
- **THEN** the system SHALL export only data belonging to the caller's tenant
- **AND** the system SHALL use PostgreSQL Row-Level Security to enforce tenant boundaries
- **AND** the archive manifest SHALL record the tenant scope

#### Scenario: Full-instance export
- **WHEN** a PlatformAdmin user initiates a full-instance export
- **THEN** the system SHALL export data across all tenants
- **AND** the archive SHALL preserve tenant identifiers on each record

#### Scenario: Cross-tenant export attempt
- **WHEN** a user without PlatformAdmin role attempts a full-instance or cross-tenant export
- **THEN** the system SHALL reject the request with an authorization error
- **AND** the system SHALL NOT export any data from other tenants

### Requirement: Incremental Export
The system SHALL support exporting only data created or modified since a specified point in time.

#### Scenario: Since-timestamp export
- **WHEN** an export request includes a `since` timestamp
- **THEN** the system SHALL export only entities with `created_at` or `updated_at` greater than the specified timestamp
- **AND** the manifest SHALL record the since-timestamp and indicate the export is incremental

#### Scenario: Layer-scoped export
- **WHEN** an export request includes a layer filter
- **THEN** the system SHALL export only entities belonging to the specified MemoryLayer or KnowledgeLayer
- **AND** the manifest SHALL record the layer scope

### Requirement: Identity Remapping
The system SHALL support mapping identifiers when importing archives from a different Aeterna instance.

#### Scenario: Strict identity mode
- **WHEN** an import is executed without a remap file
- **THEN** the system SHALL verify that all tenant IDs, user IDs, and org unit IDs in the archive exist in the target instance
- **AND** the system SHALL fail the import if any referenced identifier does not exist

#### Scenario: Remap identity mode
- **WHEN** an import is executed with a remap file
- **THEN** the system SHALL translate all tenant IDs, user IDs, and org unit IDs in the archive according to the mapping file before writing
- **AND** the system SHALL fail if the remap file does not cover all identifiers found in the archive

### Requirement: Embedding Handling on Import
The system SHALL support importing vector embeddings directly or re-computing them.

#### Scenario: Direct embedding import
- **WHEN** an import writes MemoryEntry records with non-null embeddings
- **THEN** the system SHALL write the embedding vectors directly to the vector store without re-computation

#### Scenario: Re-embed on import
- **WHEN** an import is executed with the re-embed option enabled
- **THEN** the system SHALL discard the archived embedding vectors and re-compute them using the target instance's embedding provider
- **AND** the system SHALL log when the source and target embedding models differ

### Requirement: CLI Export and Import Commands
The system SHALL provide CLI commands for export and import that execute real backend-backed operations.

#### Scenario: CLI export invokes server-backed job
- **WHEN** a user runs `aeterna admin export` with a configured and reachable server
- **THEN** the CLI SHALL initiate a server-side export job, poll for completion with progress display, and download the resulting archive
- **AND** the CLI SHALL display progress during the export including entity counts and percentage

#### Scenario: CLI import invokes server-backed job
- **WHEN** a user runs `aeterna admin import <archive>` with a configured and reachable server
- **THEN** the CLI SHALL upload the archive, initiate a server-side import job, and display the validation or import report
- **AND** dry-run mode SHALL display the conflict report without executing the import

#### Scenario: CLI export without server connection
- **WHEN** a user runs `aeterna admin export` without a reachable server
- **THEN** the CLI SHALL return an explicit connectivity error identifying the target server
- **AND** the CLI SHALL NOT return simulated export data or placeholder success

### Requirement: Archive Validation Command
The system SHALL provide an offline archive validation command that does not require a server connection.

#### Scenario: Validate archive integrity offline
- **WHEN** a user runs `aeterna admin backup validate <archive>`
- **THEN** the CLI SHALL verify the archive structure, manifest schema version, and per-file checksums without requiring a server connection
- **AND** the command SHALL report any integrity violations
- **AND** the command SHALL exit with a non-zero status code if any validation fails

### Requirement: Export and Import Observability
The system SHALL emit metrics for export and import operations to support operational monitoring.

#### Scenario: Export job metrics
- **WHEN** an export job is running or completes
- **THEN** the system SHALL emit Prometheus metrics for rows processed, duration in seconds, and bytes written
- **AND** the metrics SHALL be labeled by job_id, scope, and target

#### Scenario: Import job metrics
- **WHEN** an import job is running or completes
- **THEN** the system SHALL emit Prometheus metrics for rows processed, conflicts detected, and duration in seconds
- **AND** the metrics SHALL be labeled by job_id, mode, and scope
