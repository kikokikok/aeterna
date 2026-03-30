## MODIFIED Requirements

### Requirement: Deployment Configuration
The system SHALL support deployment-time configuration of server-side model providers, including provider-specific credentials, model identifiers, and runtime settings.

#### Scenario: Configure Google Cloud model provider
- **WHEN** deploying with Google Cloud as the selected model provider
- **THEN** the deployment configuration SHALL expose the required project, location, generation model, embedding model, and credential reference settings
- **AND** the runtime environment SHALL receive the corresponding configuration needed to construct the provider

#### Scenario: Configure AWS Bedrock model provider
- **WHEN** deploying with AWS Bedrock as the selected model provider
- **THEN** the deployment configuration SHALL expose the required AWS region, generation model, embedding model, and credential-chain-compatible runtime settings
- **AND** the runtime environment SHALL receive the corresponding configuration needed to construct the provider

#### Scenario: Reject incomplete cloud provider deployment configuration
- **WHEN** a deployment selects Google Cloud or AWS Bedrock without the required provider-specific configuration
- **THEN** the deployment SHALL fail closed during validation or startup
- **AND** the operator SHALL receive an error identifying the missing required settings
