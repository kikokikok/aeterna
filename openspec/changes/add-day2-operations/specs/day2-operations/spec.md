---
title: Day-2 Operations Specification
status: draft
version: 0.1.0
created: 2026-04-12
authors:
  - AI Systems Architecture Team
related:
  - memory-system
  - knowledge-repository
  - storage
  - runtime-operations
  - backup-restore
---

## Purpose

The Day-2 Operations capability provides autonomous data lifecycle management for Aeterna deployments. It ensures cross-layer data consistency, enforces retention and quota policies, ages stale data, detects and repairs orphaned references, and routes destructive remediations through a human-in-the-loop approval system — minimizing operator burden while preventing silent data corruption.

## Requirements

### Requirement: Cascading Delete Across All Backends
The system SHALL ensure that deleting an entity from any layer cascades to all backends holding related data.

#### Scenario: Memory delete cascades to Qdrant and graph
- **WHEN** a memory entry is deleted from PostgreSQL
- **THEN** the system SHALL delete the corresponding vector point from Qdrant
- **AND** the system SHALL soft-delete the corresponding graph node in DuckDB
- **AND** the system SHALL delete any embedding cache keys in Redis matching the memory ID

#### Scenario: Knowledge item delete cascades to promotions and relations
- **WHEN** a knowledge item is deleted
- **THEN** the system SHALL delete all promotion requests referencing that item as source or target
- **AND** the system SHALL delete all knowledge relations where source_id or target_id matches the deleted item

#### Scenario: User deletion completes all residual data
- **WHEN** a user is deleted via the GDPR flow
- **THEN** the system SHALL delete all role assignments for the user
- **AND** the system SHALL anonymize the actor field in governance events authored by the user
- **AND** the system SHALL cascade memory and knowledge deletion as specified above

#### Scenario: Org unit delete cascades to role assignments
- **WHEN** an organizational unit is deleted
- **THEN** the system SHALL delete all role assignments scoped to that unit

#### Scenario: Tenant purge after quarantine
- **WHEN** a tenant has been deactivated for longer than the configured quarantine period (default 30 days)
- **THEN** the system SHALL create a RequireApproval remediation request for full tenant data purge
- **AND** upon approval, the system SHALL delete all tenant data from PostgreSQL, Qdrant, DuckDB, and Redis

### Requirement: Cross-Layer Reconciliation
The system SHALL periodically detect and report divergence between storage backends.

#### Scenario: PG-Qdrant reconciliation detects orphaned vectors
- **WHEN** the reconciliation job samples memory IDs from PostgreSQL and finds Qdrant points with no corresponding PG record
- **THEN** the system SHALL create a RequireApproval remediation request listing the orphaned vector IDs, affected tenant, and proposed action (delete orphaned points)

#### Scenario: PG-Qdrant reconciliation detects missing vectors
- **WHEN** the reconciliation job finds PostgreSQL memory entries with no corresponding Qdrant point
- **THEN** the system SHALL create a RequireApproval remediation request proposing re-embedding the affected memories

#### Scenario: PG-Graph reconciliation detects orphaned nodes
- **WHEN** the reconciliation job finds DuckDB graph nodes with no corresponding PostgreSQL memory entry
- **THEN** the system SHALL create a RequireApproval remediation request proposing hard-deletion of orphaned graph nodes

#### Scenario: Reconciliation uses sampling by default
- **WHEN** the scheduled reconciliation job runs
- **THEN** the system SHALL sample a configurable percentage of records (default 5%)
- **AND** if orphans are detected in the sample, the system SHALL trigger a targeted full scan of the affected time window

### Requirement: Remediation Request System
The system SHALL route detected issues through a risk-tiered approval system before executing destructive actions.

#### Scenario: Auto-execute tier actions proceed without approval
- **WHEN** a lifecycle task detects an issue classified as AutoExecute risk tier (expired TTL cleanup, completed job removal)
- **THEN** the system SHALL execute the remediation immediately
- **AND** the system SHALL log the action with details

#### Scenario: Notify-and-execute tier actions proceed with notification
- **WHEN** a lifecycle task detects an issue classified as NotifyAndExecute risk tier (quota enforcement, importance decay, audit archival)
- **THEN** the system SHALL execute the remediation
- **AND** the system SHALL create a notification record visible in the admin dashboard

#### Scenario: Require-approval tier actions wait for operator
- **WHEN** a lifecycle task detects an issue classified as RequireApproval risk tier (reconciliation deletes, tenant purge, dead-letter discard)
- **THEN** the system SHALL create a remediation request and NOT execute the action
- **AND** the system SHALL only execute the action after an operator approves the request

#### Scenario: Remediation request auto-expiry
- **WHEN** a remediation request has been pending for more than 7 days without action
- **THEN** the system SHALL mark the request as expired
- **AND** the system SHALL log the expiry with the original details for audit

#### Scenario: Remediation escalation
- **WHEN** a RequireApproval remediation request has been pending for more than 48 hours
- **THEN** the system SHALL emit an escalation alert via the configured notification channel

### Requirement: Retention Policies
The system SHALL enforce configurable retention periods for time-series data to prevent unbounded storage growth.

#### Scenario: Audit log purge after archival
- **WHEN** audit log records have been archived to S3 and are older than the configured retention period (default 90 days)
- **THEN** the system SHALL hard-delete them from PostgreSQL

#### Scenario: Governance event retention
- **WHEN** governance events are older than the configured TTL (default 180 days)
- **THEN** the system SHALL delete them from PostgreSQL

#### Scenario: Soft-delete hard-purge
- **WHEN** soft-deleted graph nodes have a `deleted_at` timestamp older than the configured TTL (default 7 days)
- **THEN** the system SHALL hard-delete them from DuckDB

#### Scenario: Export/import job cleanup
- **WHEN** export or import job records are in completed or failed status for more than 24 hours
- **THEN** the system SHALL delete the job records and any associated temporary archive files

#### Scenario: Promotion request cleanup
- **WHEN** rejected or abandoned promotion requests are older than 30 days
- **THEN** the system SHALL delete them from the knowledge repository

### Requirement: Per-Tenant Storage Quota Enforcement
The system SHALL enforce configurable storage limits per tenant to prevent resource exhaustion.

#### Scenario: Soft limit warning
- **WHEN** a tenant's storage usage exceeds 80% of their configured quota
- **THEN** the system SHALL emit a warning metric and log entry
- **AND** the system SHALL continue accepting writes

#### Scenario: Hard limit rejection
- **WHEN** a tenant's storage usage reaches 100% of their configured quota
- **THEN** the system SHALL reject new write operations with HTTP 429
- **AND** the system SHALL create a NotifyAndExecute remediation request alerting operators

#### Scenario: Quota usage reporting
- **WHEN** an administrator queries tenant storage usage
- **THEN** the system SHALL return current counts and sizes for memories, knowledge items, and vectors compared to configured quotas

### Requirement: Memory Importance Decay and Archival
The system SHALL age memories based on access patterns and archive stale data to cold storage.

#### Scenario: Importance score decay
- **WHEN** the periodic decay job runs
- **THEN** the system SHALL apply exponential decay to importance scores based on time since last access
- **AND** the decay rate SHALL be configurable per memory layer

#### Scenario: Cold-tier archival trigger
- **WHEN** a memory's importance score drops below the archival threshold (default 0.01) after decay
- **THEN** the system SHALL archive the memory content to cold storage
- **AND** the system SHALL remove the Qdrant vector to free hot storage
- **AND** the system SHALL retain a stub record in PostgreSQL with archived status and cold-tier reference

#### Scenario: Access resets decay clock
- **WHEN** a memory is accessed via search or direct retrieval
- **THEN** the system SHALL update the `last_accessed_at` timestamp
- **AND** the decay calculation SHALL use this updated timestamp

### Requirement: Dead-Letter Queue for Failed Operations
The system SHALL quarantine permanently failed sync and promotion items for manual review rather than retrying indefinitely.

#### Scenario: Failed sync item moves to dead-letter
- **WHEN** a sync item has failed more than the configured max retries (default 5)
- **THEN** the system SHALL move it to the dead-letter queue with the error details and retry history

#### Scenario: Manual retry from dead-letter
- **WHEN** an operator triggers a retry for a dead-letter item
- **THEN** the system SHALL re-attempt the operation and move the item back to active processing if successful

#### Scenario: Dead-letter discard requires approval
- **WHEN** an operator requests permanent discard of a dead-letter item
- **THEN** the system SHALL create a RequireApproval remediation request before deleting

### Requirement: Lifecycle Task Scheduling
The system SHALL run lifecycle tasks as periodic background jobs with configurable schedules and graceful shutdown.

#### Scenario: Lifecycle tasks start with server
- **WHEN** the server starts and `AETERNA_LIFECYCLE_ENABLED` is true (default)
- **THEN** the system SHALL spawn all lifecycle tasks as background Tokio tasks with their configured schedules

#### Scenario: Graceful shutdown
- **WHEN** the server receives SIGTERM
- **THEN** the system SHALL cancel all pending lifecycle tasks
- **AND** the system SHALL wait for in-progress tasks to complete before shutting down

#### Scenario: Lifecycle feature flag
- **WHEN** `AETERNA_LIFECYCLE_ENABLED` is set to false
- **THEN** no lifecycle tasks SHALL be spawned
- **AND** the server SHALL start normally without lifecycle management

### Requirement: Lifecycle Observability
The system SHALL emit metrics and structured logs for all lifecycle operations.

#### Scenario: Reconciliation metrics
- **WHEN** a reconciliation job completes
- **THEN** the system SHALL emit metrics for orphans detected, duration, and records sampled

#### Scenario: Quota usage metrics
- **WHEN** quota checks run
- **THEN** the system SHALL emit per-tenant usage count metrics labeled by entity type

#### Scenario: Remediation queue depth metric
- **WHEN** remediation requests exist
- **THEN** the system SHALL emit a gauge metric for pending remediation count by risk tier
