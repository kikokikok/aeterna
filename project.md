# Project Context

## Purpose
OpenSpec-compliant Knowledge Provider specification for universal memory and knowledge backend systems. Defines standardized protocol for AI agents, tools, and applications to query, store, and retrieve knowledge from pluggable backends.

## Tech Stack
- **Primary Language**: Rust (for implementation references)
- **Memory Storage**:
  - Redis 7+ (Working/Session cache)
  - PostgreSQL 16+ with pgvector (Episodic, Procedural, User, Org)
  - Qdrant 1.12+ (Semantic, Archival vectors)
- **Embedding**: rust-genai 0.4+ (multi-provider: OpenAI, Anthropic, Gemini, Z.AI)
- **API**: OpenSpec v1.0.0 protocol
- **Testing**: TDD/BDD with tarpaulin, proptest, cargo-mutants

## Project Conventions

### Code Style
- Use SHALL/MUST for normative requirements
- Scenario format: `#### Scenario: [Name]` with **WHEN**... **THEN**... **AND**...
- Every requirement MUST have at least one scenario
- Follow Rust style guide with `cargo fmt`
- Document public APIs with comprehensive examples

### Architecture Patterns
- **8-Layer Memory Hierarchy**:
  - Working (µs, in-memory Redis)
  - Session (ms, Redis with TTL)
  - Episodic (h, PostgreSQL + pgvector)
  - Semantic (d, Qdrant vector search)
  - Procedural (w, PostgreSQL facts)
  - User Personal (mo, PostgreSQL + pgvector)
  - Organization (mo, PostgreSQL + pgvector)
  - Archival (yr, Qdrant long-term storage)

- **Cross-Layer Queries**: Concurrent queries across multiple memory tiers
- **Knowledge Sources**: Pluggable providers (Git, NotebookLM, Dust.tt, Perplexity, Custom MCP)
- **OpenSpec Protocol**: Standardized endpoints for universal compatibility

### Testing Strategy
- **Test-Driven Development**: Write tests before code (RED-GREEN-REFACTOR)
- **Coverage Thresholds**: 80%+ overall, 85%+ core logic
- **Property-Based Testing**: Critical algorithms (promotion scoring, similarity metrics, confidence aggregation)
- **Mutation Testing**: 90%+ mutants killed for critical code paths
- **Testability**: All external dependencies behind trait abstractions for easy mocking
- **Test Fixtures**: All external service responses have deterministic fixtures
- **BDD Scenarios**: All critical workflows have Gherkin-style scenarios

### Git Workflow
- Feature branches from `main`
- PR reviews required before merge
- Conventional commits format
- Automated CI/CD pipeline

## Domain Context

### Knowledge Provider Protocol
OpenSpec defines a universal protocol for Knowledge Providers enabling:
- **Universal Compatibility**: Any OpenCode-compliant agent can use any OpenSpec provider
- **Pluggable Architecture**: Easy to add/remove knowledge sources
- **Type Safety**: Strong typing for all operations
- **Performance**: Efficient query and retrieval with minimal overhead

### Knowledge Entry Model
```typescript
interface KnowledgeEntry {
  id: string;
  content: string;
  embedding?: number[];
  metadata: KnowledgeMetadata;
  source: KnowledgeSource;
  access_policy: AccessPolicy;
  created_at: ISO8601;
  updated_at?: ISO8601;
  version?: number;
}
```

### Memory Layer Performance Targets
| Layer | Latency (P95) | Storage |
|--------|-----------------|----------|
| Working Memory | ~5ms | In-memory Redis |
| Session Memory | ~20ms | Redis with TTL |
| Episodic Memory | ~50ms | PostgreSQL + pgvector |
| Semantic Memory | ~100ms | Qdrant |
| Procedural Memory | ~30ms | PostgreSQL |
| User Personal | ~50ms | PostgreSQL + pgvector |
| Organization | ~200ms | PostgreSQL + pgvector |
| Archival | ~500ms | Qdrant secondary |

## Important Constraints

### Testing Requirements (Non-Negotiable)
- **Minimum 80% test coverage** enforced in CI/CD
- **Property-based tests** for all critical algorithms (promotion score, similarity metrics)
- **Mutation testing** with 90%+ mutants killed
- **Test fixtures** for all external API responses (deterministic, versioned)
- **TDD/BDD** from Day 1 - no code without failing tests

### Performance Requirements
- **Query latency**: Working <10ms, Session <50ms, Semantic <200ms
- **Throughput**: >100 QPS, >50 CPS (creates per second)
- **Resource utilization**: CPU <70%, Memory <80%, DB connections <80%

### OpenSpec Compliance
All implementations MUST provide:
1. Discovery endpoint (`GET /openspec/v1/knowledge`)
2. Query endpoint (`POST /openspec/v1/knowledge/query`)
3. Create endpoint (`POST /openspec/v1/knowledge/create`)
4. Update endpoint (`PUT /openspec/v1/knowledge/{id}`)
5. Delete endpoint (`DELETE /openspec/v1/knowledge/{id}`)
6. Batch operations endpoint (`POST /openspec/v1/knowledge/batch`)
7. Streaming endpoint (`GET /openspec/v1/knowledge/stream`)
8. Metadata operations (`GET /openspec/v1/knowledge/{id}/metadata`)

## External Dependencies

### Knowledge Source Integrations
- **Git Repository Provider**: Full version tracking, commit/diff extraction
- **NotebookLM Provider**: .ipynb parsing, cell content extraction
- **External AI Providers**: Dust.tt, Perplexity (API integration)
- **Local File System**: Recursive file indexing
- **Custom MCP Provider Framework**: Universal MCP server integration

### LLM/Embedding Providers (via rust-genai)
- OpenAI, Anthropic, Gemini, xAI, Ollama, Groq, DeepSeek, Cohere, Mimo
- Custom endpoints: Z.AI, AWS Bedrock, Vertex AI

### Data Storage
- PostgreSQL 16+ (metadata, facts, user/org data)
- Qdrant 1.12+ (vector embeddings, semantic search)
- Redis 7+ (working/session cache, message bus)

### Tooling
- OpenSpec CLI 0.16.0+ (spec validation, archiving)
- Cargo workspace management
- Docker Compose (development environment)
- OpenTelemetry (distributed tracing)
- Prometheus (metrics collection)
