## 1. Shared runtime provider factory
- [x] 1.1 Add runtime factory modules for LLM and embedding provider construction
- [x] 1.2 Define provider-selection config parsing and fail-closed validation for supported providers
- [x] 1.3 Wire factory-created services into server-side memory and reasoning initialization paths
- [x] 1.4 Add unit tests for provider selection, invalid provider handling, and missing-config failures

## 2. Google Cloud provider support
- [x] 2.1 Add Google Cloud generation provider implementation
- [x] 2.2 Add Google Cloud embedding provider implementation
- [x] 2.3 Add tests for Google provider request/response adaptation and fail-closed config handling

## 3. AWS Bedrock provider support
- [x] 3.1 Add AWS Bedrock generation provider implementation
- [x] 3.2 Add AWS Bedrock embedding provider implementation
- [x] 3.3 Add tests for Bedrock request/response adaptation and fail-closed config handling

## 4. Deployment and setup surfaces
- [x] 4.1 Extend Helm values, schema, configmap, deployment, and secret handling for Google Cloud and AWS Bedrock
- [x] 4.2 Extend setup CLI provider enums, prompts, and config generation for Google Cloud and AWS Bedrock
- [x] 4.3 Add deployment validation coverage for supported provider configurations

## 5. Documentation and verification
- [x] 5.1 Document supported provider configuration and operational requirements for Google Cloud and AWS Bedrock
- [x] 5.2 Run targeted tests for new provider modules and provider-factory wiring
- [ ] 5.3 Run required workspace validation and coverage checks and resolve regressions caused by the change
