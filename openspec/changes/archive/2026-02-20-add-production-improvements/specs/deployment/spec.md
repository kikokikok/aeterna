# Spec Delta: Deployment

## ADDED Requirements

### Requirement: High Availability Infrastructure

The system SHALL provide 99.9% availability through automated failover and redundancy.

#### Scenario: PostgreSQL Primary Failure
- **WHEN** PostgreSQL primary node fails
- **THEN** Patroni detects failure within 10 seconds
- **AND** promotes replica to primary within 20 seconds
- **AND** updates DNS/connection routing
- **AND** service resumes with < 30 seconds downtime
- **AND** no data loss (synchronous replication)

#### Scenario: Qdrant Node Failure
- **WHEN** one Qdrant node fails in 3-node cluster
- **THEN** remaining nodes continue serving requests
- **AND** replication factor 2 ensures data availability
- **AND** no queries fail
- **AND** performance degradation < 20%

#### Scenario: Redis Master Failure
- **WHEN** Redis master fails
- **THEN** Redis Sentinel detects failure within 5 seconds
- **AND** promotes replica to master within 10 seconds
- **AND** clients reconnect automatically
- **AND** cache remains available

### Requirement: Disaster Recovery Capabilities

The system SHALL support disaster recovery with RTO < 15 minutes and RPO < 5 minutes.

#### Scenario: Full Region Failure
- **WHEN** primary region becomes unavailable
- **THEN** operations team initiates DR procedure
- **AND** restores PostgreSQL from WAL archive (< 10 min)
- **AND** restores Qdrant from S3 snapshots (< 5 min)
- **AND** restores Redis from RDB backup (< 2 min)
- **AND** validates data integrity
- **AND** resumes service within 15 minutes

#### Scenario: Point-in-Time Recovery
- **WHEN** data corruption detected at timestamp T
- **THEN** system can restore to any point in last 7 days
- **AND** uses WAL replay for PostgreSQL
- **AND** uses snapshot + WAL for Qdrant
- **AND** recovery time < 30 minutes

### Requirement: Horizontal Scaling Support

The system SHALL support horizontal scaling of all services independently.

#### Scenario: Memory Service Autoscaling
- **WHEN** memory service QPS exceeds 1000/replica
- **THEN** Kubernetes HPA scales replicas
- **AND** new pods join within 30 seconds
- **AND** load balancer distributes traffic
- **AND** no requests fail during scale-up

#### Scenario: Tenant Sharding
- **WHEN** tenant memory count exceeds 100,000
- **THEN** system creates dedicated Qdrant collection
- **AND** migrates tenant data to dedicated collection
- **AND** routes subsequent requests to dedicated collection
- **AND** migration completes without service interruption

## ADDED Requirements

### Requirement: Encryption at Rest

The system SHALL encrypt all data at rest using industry-standard encryption.

#### Scenario: PostgreSQL Encryption
- **WHEN** PostgreSQL stores data
- **THEN** TDE encrypts data before writing to disk
- **AND** uses AES-256 encryption
- **AND** keys managed by KMS
- **AND** automatic key rotation every 90 days

#### Scenario: Field-Level Encryption
- **WHEN** sensitive field (email, PII) stored
- **THEN** system applies field-level encryption
- **AND** uses AES-256-GCM
- **AND** encryption transparent to queries
- **AND** audit log records encryption operations

## ADDED Requirements

### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options.

#### Scenario: Production HA Deployment
- **WHEN** deploying to production
- **THEN** Helm chart deploys:
  - PostgreSQL with Patroni (1 primary + 2 replicas)
  - Qdrant cluster (3 nodes, RF=2)
  - Redis Sentinel (3 nodes)
  - Memory service (3+ replicas)
  - Knowledge service (2+ replicas)
- **AND** configures PodDisruptionBudgets
- **AND** enables automatic backups
- **AND** configures monitoring
