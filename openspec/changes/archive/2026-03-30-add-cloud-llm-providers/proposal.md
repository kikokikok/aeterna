## Why

Aeterna currently defines provider-agnostic LLM and embedding traits, but the core runtime only has a clearly implemented OpenAI path and lacks a runtime factory that selects providers from deployment configuration. Enterprise deployments need first-class support for cloud-managed providers so operators can run Aeterna on AWS or Google Cloud without forcing all model traffic through OpenAI-specific credentials and APIs.

## What Changes

- Add a supported runtime provider-selection layer for LLM and embedding services so the server can instantiate providers from deployment configuration instead of relying on ad hoc construction
- Add Google Cloud Vertex AI / Gemini as a supported server-side provider for text generation and embeddings
- Add AWS Bedrock as a supported server-side provider for text generation and embeddings
- Extend deployment, Helm, and setup surfaces so operators can configure provider-specific credentials, model identifiers, regions/projects, and runtime environment for OpenAI, Google Cloud, AWS Bedrock, and explicit no-provider mode
- Document provider-specific operational requirements, including ADC/service-account setup for Google Cloud and IAM/region requirements for AWS Bedrock

## Capabilities

### New Capabilities
- `model-provider-runtime`: Runtime selection and configuration of LLM and embedding providers for server-side Aeterna services

### Modified Capabilities
- `memory-system`: Change memory and reasoning requirements so configured LLM and embedding providers can be instantiated from runtime configuration and used consistently across server-side operations
- `deployment`: Add supported deployment requirements for Google Cloud and AWS Bedrock credentials, secret handling, runtime env wiring, and provider-specific configuration

## Impact

- Affected code:
  - `memory/`
  - `mk_core/`
  - `cli/src/commands/setup/`
  - `charts/aeterna/`
  - `docs/`, `README.md`, `INSTALL.md`
- Affected systems:
  - server-side LLM/embedding service construction
  - memory and reasoning workflows that depend on embeddings or text generation
  - Helm values, secrets, and runtime environment configuration
- External dependencies:
  - Google Cloud Vertex AI / Gemini SDK or API integration
  - AWS Bedrock Runtime SDK integration
  - Google ADC / service account credentials
  - AWS IAM / regional Bedrock model access
