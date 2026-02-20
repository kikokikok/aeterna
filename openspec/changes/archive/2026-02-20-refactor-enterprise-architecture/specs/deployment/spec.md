## ADDED Requirements
### Requirement: OpenTofu Multi-Cloud Provisioning
The platform SHALL provide OpenTofu Infrastructure-as-Code modules for automated, highly-available deployment across GCP, AWS, and Azure.

#### Scenario: Provisioning AWS
- **WHEN** applying the AWS module
- **THEN** an EKS cluster, Multi-AZ RDS Postgres, and ElastiCache Redis are provisioned
- **AND** IAM Roles for Service Accounts (IRSA) are generated and bound to Aeterna components

### Requirement: Cloud KMS Encryption
The deployment modules MUST enforce Customer-Managed Encryption Keys (CMEK) at rest for all stateful stores.

#### Scenario: GCP Encryption
- **WHEN** provisioning GCS Buckets or Cloud SQL in GCP
- **THEN** the resources must be encrypted with a Cloud KMS key provisioned by the module