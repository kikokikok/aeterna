# Memory-Knowledge System Specification

[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Spec Version](https://img.shields.io/badge/spec-v0.1.0-green.svg)](specs/)

A **generic, implementation-agnostic specification** for AI agent memory and knowledge management systems.

## What is This?

This repository contains a complete specification for building memory and knowledge systems for AI agents. It's designed to be:

- **Implementation-agnostic**: Works with any AI agent framework
- **Provider-flexible**: Supports multiple storage backends via adapters
- **Ecosystem-extensible**: Integrates with LangChain, AutoGen, CrewAI, OpenCode, and more

## Why a Specification?

AI agent frameworks each implement their own memory solutions, leading to:

- **Vendor lock-in**: Memories trapped in proprietary formats
- **Inconsistent behavior**: Different semantics across frameworks
- **No knowledge governance**: Ad-hoc decision documentation
- **Migration headaches**: Difficult to switch providers

This specification provides a **common standard** that enables:

- Data portability between providers
- Consistent semantics across frameworks
- Governed knowledge with constraints
- Ecosystem interoperability

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Agent Runtime                            │
│  ┌─────────────────────────────────────────────────────────────┤
│  │                    Tool Interface                            │
│  │  memory_add | memory_search | knowledge_query | ...          │
│  └───────────────────────────┬─────────────────────────────────┤
│                              │                                  │
│  ┌───────────────────────────┼─────────────────────────────────┤
│  │                    Adapter Layer                             │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │  │   Provider  │    │  Ecosystem  │    │    Sync     │      │
│  │  │   Adapters  │    │   Adapters  │    │   Service   │      │
│  │  └─────────────┘    └─────────────┘    └─────────────┘      │
│  └───────────────────────────┬─────────────────────────────────┤
│                              │                                  │
│  ┌───────────────────────────┼─────────────────────────────────┤
│  │                    Storage Layer                             │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐      │
│  │  │   Memory    │    │  Knowledge  │    │   Vector    │      │
│  │  │   Provider  │    │    Repo     │    │     DB      │      │
│  │  │ (Mem0/Letta)│    │ (PostgreSQL)│    │  (Qdrant)   │      │
│  │  └─────────────┘    └─────────────┘    └─────────────┘      │
└─────────────────────────────────────────────────────────────────┘
```

## Key Concepts

### Memory (Ephemeral → Persistent)

7-layer hierarchy for context-appropriate storage:

| Layer | Scope | Lifetime | Example |
|-------|-------|----------|---------|
| **agent** | Single agent | Persistent | Agent personality, capabilities |
| **user** | Cross-session user data | Persistent | User preferences, history |
| **session** | Single conversation | Session | Current task context |
| **project** | Project-wide | Persistent | Codebase patterns |
| **team** | Team-shared | Persistent | Team conventions |
| **org** | Organization | Persistent | Engineering standards |
| **company** | Company-wide | Persistent | Corporate policies |

### Knowledge (Governed Decisions)

4-layer hierarchy for organizational wisdom:

| Type | Purpose | Example |
|------|---------|---------|
| **ADR** | Architecture decisions | "Use TypeScript for all services" |
| **Policy** | Rules and compliance | "All APIs require authentication" |
| **Pattern** | Reusable solutions | "Error handling pattern" |
| **Spec** | Technical specifications | "API contract definition" |

### Constraint DSL

Enforceable rules attached to knowledge:

```yaml
constraints:
  - operator: must_use
    target: file
    pattern: "*.ts"
    appliesTo: ["src/**"]
    severity: warn
    message: "Use TypeScript for source files"
```

## Specification Index

| Spec | Status | Description |
|------|--------|-------------|
| [00-overview](specs/00-overview.md) | ✅ Complete | Executive summary, architecture |
| [01-core-concepts](specs/01-core-concepts.md) | ✅ Complete | Glossary, design principles |
| [02-memory-system](specs/02-memory-system.md) | ✅ Complete | 7-layer hierarchy, operations |
| [03-knowledge-repository](specs/03-knowledge-repository.md) | ✅ Complete | Schema, constraints, versioning |
| [04-memory-knowledge-sync](specs/04-memory-knowledge-sync.md) | ✅ Complete | Pointer architecture, delta sync |
| [05-adapter-architecture](specs/05-adapter-architecture.md) | ✅ Complete | Provider + ecosystem adapters |
| [06-tool-interface](specs/06-tool-interface.md) | ✅ Complete | Tool contracts |
| [07-configuration](specs/07-configuration.md) | ✅ Complete | Config schema, validation |
| [08-deployment](specs/08-deployment.md) | ✅ Complete | Self-hosted, cloud, Kubernetes |
| [09-migration](specs/09-migration.md) | ✅ Complete | Data portability, import/export |

## Quick Start

### 1. Choose Your Provider

| Provider | Type | Use Case |
|----------|------|----------|
| **Mem0** | Cloud/Self-hosted | Production, managed |
| **Letta** | Self-hosted | Local development |
| **OpenMemory** | Self-hosted | Enterprise, air-gapped |

### 2. Choose Your Ecosystem

| Ecosystem | Adapter Status |
|-----------|----------------|
| **OpenCode** | [Available](adapters/opencode/) |
| **LangChain** | Planned |
| **AutoGen** | Planned |
| **CrewAI** | Planned |

### 3. Configure

```json
{
  "memory": {
    "provider": "mem0",
    "config": {
      "apiKey": "${MEM0_API_KEY}"
    }
  },
  "knowledge": {
    "provider": "postgresql",
    "config": {
      "connectionString": "${KNOWLEDGE_DB_URL}"
    }
  },
  "sync": {
    "enabled": true,
    "intervalMs": 60000
  }
}
```

### 4. Use

```typescript
import { MemoryKnowledgeClient } from '@memory-knowledge-spec/client';

const client = new MemoryKnowledgeClient(config);

// Add memory
await client.memory.add({
  content: 'User prefers dark mode',
  layer: 'user',
  metadata: { category: 'preferences' }
});

// Search memories
const results = await client.memory.search({
  query: 'user preferences',
  limit: 10
});

// Check knowledge constraints
const violations = await client.knowledge.checkConstraints({
  code: myCode,
  context: { filePath: 'src/api.ts' }
});
```

## Deployment Options

### Local Development

```bash
docker compose -f docker-compose.local.yml up -d
```

### Self-Hosted Production

```bash
docker compose -f docker-compose.prod.yml up -d
```

### Kubernetes

```bash
kubectl apply -f k8s/
```

See [08-deployment.md](specs/08-deployment.md) for detailed instructions.

## Data Portability

### Export

```bash
mk-export --output ./backup --format standard
```

### Import

```bash
mk-import ./backup.tar.gz --conflict-strategy=merge
```

### Migrate Between Providers

```bash
# Mem0 → Letta
mk-migrate --source mem0 --target letta
```

See [09-migration.md](specs/09-migration.md) for detailed migration guides.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Areas for Contribution

- **New Ecosystem Adapters**: LangChain, AutoGen, CrewAI
- **Provider Implementations**: Custom backends
- **Tooling**: CLI improvements, visualization
- **Documentation**: Examples, tutorials

## Project Structure

```
memory-knowledge-spec/
├── specs/                    # Specification documents
│   ├── 00-overview.md
│   ├── 01-core-concepts.md
│   ├── 02-memory-system.md
│   ├── 03-knowledge-repository.md
│   ├── 04-memory-knowledge-sync.md
│   ├── 05-adapter-architecture.md
│   ├── 06-tool-interface.md
│   ├── 07-configuration.md
│   ├── 08-deployment.md
│   └── 09-migration.md
├── adapters/                 # Ecosystem adapters
│   ├── opencode/            # OpenCode/oh-my-opencode
│   ├── langchain/           # LangChain (planned)
│   ├── autogen/             # AutoGen (planned)
│   └── crewai/              # CrewAI (planned)
├── schemas/                  # JSON schemas (planned)
├── examples/                 # Usage examples (planned)
└── README.md
```

## Roadmap

### v0.1.0 (Current)
- [x] Core specification documents
- [x] OpenCode adapter
- [x] Deployment guides
- [x] Migration procedures

### v0.2.0 (Planned)
- [ ] JSON Schema validation
- [ ] Reference implementation
- [ ] LangChain adapter
- [ ] Test suite

### v1.0.0 (Future)
- [ ] Stable API
- [ ] Conformance testing
- [ ] Multiple provider implementations
- [ ] Production case studies

## License

Apache License 2.0 - See [LICENSE](LICENSE) for details.

## Acknowledgments

This specification was informed by:
- [Mem0](https://mem0.ai) - Memory layer for AI
- [Letta](https://letta.ai) - Stateful AI agents
- [oh-my-opencode](https://github.com/opencode-project/oh-my-opencode) - OpenCode plugin system

---

**Questions?** Open an issue or start a discussion.

**Want to implement?** Check [adapters/](adapters/) for integration guides.
