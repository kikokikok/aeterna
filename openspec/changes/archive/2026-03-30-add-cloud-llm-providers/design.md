## Context

Aeterna already defines provider-agnostic `LlmService` and `EmbeddingService` traits, and the `MemoryManager` can accept concrete implementations through dependency injection. However, the production runtime does not have a factory that reads deployment configuration and instantiates the requested provider. The only clearly implemented core path today is OpenAI. Helm and setup surfaces mention Anthropic and Ollama, but those options are not fully wired through the server runtime.

At the same time, the project already carries patterns that make cloud-provider support plausible: vector backend factories, Vertex AI vector integration, feature-gated provider modules, and deployment-time environment injection. This change needs to convert those pieces into a coherent runtime provider model for server-side LLM and embedding services.

## Goals / Non-Goals

**Goals:**
- Add a runtime provider factory for LLM and embedding services
- Add Google Cloud Vertex AI / Gemini as a supported provider for text generation and embeddings
- Add AWS Bedrock as a supported provider for text generation and embeddings
- Ensure provider selection is driven by deployment/runtime config rather than ad hoc service construction
- Extend Helm and setup surfaces to configure provider-specific auth and model settings
- Preserve explicit fail-closed behavior when provider configuration is missing or invalid

**Non-Goals:**
- Rework the existing trait model for LLM or embedding services
- Implement every documented provider mentioned in setup or docs
- Add direct browser-user identity federation to Bedrock or Google APIs
- Replace OpenAI as a supported provider
- Solve every cloud-specific optimization such as provider-specific streaming, guardrails, or advanced tool-use semantics in the first change

## Decisions

### Decision: Introduce a dedicated runtime factory for LLM and embedding providers

The server runtime will gain a provider factory layer that reads runtime configuration and constructs concrete `LlmService` and `EmbeddingService` implementations. This mirrors the existing vector backend factory pattern and removes the current mismatch where Helm sets `AETERNA_LLM_PROVIDER` but the Rust runtime does not consume it directly.

**Why:**
- The current trait boundary is good, but provider selection is missing
- New providers should not require ad hoc wiring at every call site
- A single construction layer makes fail-closed validation testable and consistent

**Alternatives considered:**
- Continue constructing providers manually in callers: rejected because it spreads provider-selection logic and keeps deployment configuration disconnected from runtime behavior
- Fold provider selection into `MemoryManager::new()`: rejected because factory concerns should remain separate from manager lifecycle and test injection

### Decision: Treat Google Cloud support as Vertex AI / Gemini server-side integration

Google Cloud support will target Vertex AI / Gemini rather than ad hoc Google AI Studio paths. The provider will support server-side text generation and embeddings through a consistent cloud-authenticated runtime path.

**Why:**
- Enterprise deployments need project/location-scoped server credentials rather than user API keys
- The repo already contains Vertex-style patterns on the vector side
- Vertex AI offers both generation and embeddings in one cloud integration model

**Alternatives considered:**
- Google AI Studio API-key path as the primary provider: rejected because it is less aligned with enterprise cloud deployment and existing repo patterns
- Gemini-only generation without embeddings: rejected because memory workflows require embeddings as a first-class server capability

### Decision: Treat AWS support as Bedrock Runtime integration

AWS support will target Bedrock Runtime for text generation and embeddings. Text generation will use the normalized conversational runtime path where possible, while embeddings will use the provider-appropriate invocation path for supported embedding models.

**Why:**
- Bedrock is the AWS-managed model surface that fits the enterprise deployment goal
- It allows operators to use AWS-managed credentials and model access rather than external API keys
- It supports both generation and embeddings under the same cloud control plane, even if the API shapes differ internally

**Alternatives considered:**
- Direct vendor APIs from AWS-hosted workloads: rejected because that does not provide a first-class AWS cloud-provider integration
- Limiting AWS to generation only: rejected because the memory system depends on embeddings too

### Decision: Keep the common trait surface simple and absorb provider differences in adapters

The existing `generate()` and `embed()` trait model will remain the common server interface. Provider-specific differences — such as model path formats, regional configuration, ADC/IAM auth, or Bedrock embedding request schemas — will be handled inside provider adapters and provider configuration parsing.

**Why:**
- The current trait abstraction is already sufficient for the current server needs
- Avoids leaking provider-specific complexity across the rest of the codebase
- Makes tests and fail-closed behavior easier to reason about

**Alternatives considered:**
- Expand the trait model immediately for every advanced provider-specific feature: rejected because it would make the first multi-cloud provider step much larger and less coherent

### Decision: Deployment surfaces MUST expose explicit provider-specific auth and model settings

Helm, setup flows, and runtime config will explicitly model the settings needed to run each provider:
- OpenAI: API key and model identifiers
- Google Cloud: project, location, model identifiers, and ADC/service-account credential references
- AWS Bedrock: region, model identifiers, and IAM/credential-chain compatible runtime settings

Missing or inconsistent provider configuration MUST fail closed during render, startup, or provider construction rather than silently falling back.

**Why:**
- Bedrock and Vertex AI have materially different auth and model selection requirements from OpenAI
- Deployment surfaces must reflect those differences honestly
- Silent fallback would create confusing and insecure runtime behavior

**Alternatives considered:**
- One generic provider config blob for all providers: rejected because it obscures required fields and weakens validation

## Risks / Trade-offs

- **[Provider factory without full runtime adoption]** → Mitigate by defining explicit server-side provider-construction requirements and wiring tests
- **[Google auth complexity]** → Reuse ADC/service-account patterns already familiar from GCP environments and document required secrets/workload identity behavior
- **[Bedrock request-shape differences for embeddings]** → Keep those differences inside the Bedrock embedding adapter and constrain the first supported model set
- **[Feature-flag drift]** → Tie provider modules and Cargo features to explicit tests and deployment docs so unsupported builds fail clearly
- **[Operators choosing providers that are scaffolded but not fully wired]** → Add fail-closed validation in Helm/runtime and document exactly which providers are production-supported

## Migration Plan

1. Add runtime provider factory modules and connect server-side construction to explicit provider configuration
2. Add Google Cloud provider implementations and tests for generation, embeddings, and fail-closed config validation
3. Add AWS Bedrock provider implementations and tests for generation, embeddings, and fail-closed config validation
4. Extend setup and Helm surfaces to support Google Cloud and Bedrock config/auth requirements
5. Document the supported production paths and rollout requirements for both providers

Rollback strategy:
- revert provider selection to `none` or an already-supported provider such as OpenAI
- keep fail-closed runtime behavior for operations that require unavailable LLM or embedding services

## Open Questions

- Which Google model set should be the initial supported default for generation and embeddings?
- Which Bedrock embedding model(s) should be the first officially supported set?
- Should the first Bedrock implementation support only synchronous calls, leaving streaming for a later change?
- Should provider-specific secrets/config validation happen entirely at Helm render time, or also at binary startup even outside Helm-managed deployments?
