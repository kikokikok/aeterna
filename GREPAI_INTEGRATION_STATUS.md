# GrepAI Integration - Implementation Status

## Overview

Implementation of semantic code search and call graph analysis through GrepAI integration with Aeterna. This enables AI agents to access both organizational knowledge (Aeterna) and codebase understanding (GrepAI) through a unified MCP interface.

## Implementation Progress: 40% Complete

### ✅ Phase 1: Core MCP Proxy Tools (COMPLETE)

**Files Created** (4 files, 906 lines):
- `tools/src/grepai/mod.rs` - Module entry point
- `tools/src/grepai/client.rs` - MCP client with circuit breaker
- `tools/src/grepai/tools.rs` - 5 tool implementations
- `tools/src/grepai/types.rs` - Type definitions

**Tools Implemented**:
1. **`code_search`** - Semantic code search using natural language
   - Natural language queries
   - Configurable limit and threshold
   - File pattern and language filters
   - Tenant context support

2. **`code_trace_callers`** - Find all functions calling a symbol
   - Recursive tracing with max depth
   - File path hints for accuracy
   - Impact analysis capability

3. **`code_trace_callees`** - Find all functions called by a symbol
   - Recursive tracing with max depth
   - Dependency analysis
   - Execution flow understanding

4. **`code_graph`** - Build call dependency graph
   - Configurable depth (1-5 levels)
   - Include/exclude callers and callees
   - Graph structure for visualization

5. **`code_index_status`** - Get indexing status
   - Project-specific or all projects
   - Files indexed, chunks, state
   - Last indexed timestamp

**Features**:
- ✅ Circuit breaker pattern for resilience
- ✅ Timeout handling (configurable, default 30s)
- ✅ JSON Schema validation
- ✅ Tenant isolation support
- ✅ Graceful error handling
- ✅ Mock responses for development
- ✅ Unit tests

---

### ✅ Phase 2: CLI Commands (COMPLETE)

**Files Created** (5 files, 534 lines):
- `cli/src/commands/grepai/mod.rs` - Subcommand router
- `cli/src/commands/grepai/init.rs` - Project initialization
- `cli/src/commands/grepai/search.rs` - Semantic search
- `cli/src/commands/grepai/trace.rs` - Call graph tracing
- `cli/src/commands/grepai/status.rs` - Index status

**Commands Implemented**:

1. **`aeterna grepai init <path>`** - Initialize GrepAI for a project
   ```bash
   aeterna grepai init . --embedder ollama --store qdrant --qdrant-url http://localhost:6333
   ```
   - Configurable embedder (ollama/openai)
   - Configurable store (qdrant/postgres/gob)
   - Force re-initialization
   - JSON output support

2. **`aeterna grepai search <query>`** - Semantic code search
   ```bash
   aeterna grepai search "authentication logic" --limit 10 --threshold 0.8
   ```
   - Natural language queries
   - Limit and threshold parameters
   - File pattern filters
   - Language filters
   - JSON output
   - Files-only mode

3. **`aeterna grepai trace callers <symbol>`** - Find callers
   ```bash
   aeterna grepai trace callers HandleLogin --recursive --max-depth 3
   ```
   - Recursive tracing
   - Max depth configuration
   - File path hints
   - JSON output

4. **`aeterna grepai trace callees <symbol>`** - Find callees
   ```bash
   aeterna grepai trace callees ProcessPayment --file src/payments.rs
   ```
   - Recursive tracing
   - Max depth configuration
   - File path hints
   - JSON output

5. **`aeterna grepai trace graph <symbol>`** - Build call graph
   ```bash
   aeterna grepai trace graph AuthMiddleware --depth 2 --format dot
   ```
   - Configurable depth
   - Multiple formats (json, dot, mermaid)
   - Include/exclude callers/callees

6. **`aeterna grepai status`** - Index status
   ```bash
   aeterna grepai status --watch
   ```
   - Project-specific or all
   - Watch mode (real-time updates)
   - JSON output

**Integration**:
- ✅ Registered in `cli/src/commands/mod.rs`
- ✅ Handler in `cli/src/main.rs`
- ✅ Full command-line argument parsing
- ✅ Error handling and user feedback

---

### ⏳ Phase 3: Helm Chart Sidecar (PENDING)

**Planned Changes**:

1. **Update `charts/aeterna/values.yaml`**:
   ```yaml
   grepai:
     enabled: false  # Default disabled
     image:
       repository: greptileai/grepai
       tag: "v0.26.0"
       pullPolicy: IfNotPresent
     embedder:
       provider: ollama  # or openai
       model: nomic-embed-text
       ollamaUrl: http://ollama:11434
       openaiApiKey: ""
     store:
       backend: qdrant  # or postgres, gob
       qdrantUrl: ""  # Shared with Aeterna
       postgresUrl: ""  # Shared with Aeterna
     resources:
       requests:
         memory: 512Mi
         cpu: 250m
       limits:
         memory: 1Gi
         cpu: 500m
     projects: []  # Auto-initialize these projects
   ```

2. **Create `charts/aeterna/templates/grepai-configmap.yaml`**:
   - GrepAI configuration
   - Workspace setup
   - Backend connections

3. **Update `charts/aeterna/templates/aeterna/deployment.yaml`**:
   - Add GrepAI sidecar container
   - Add init container for `grepai init`
   - Add shared volumes
   - Add stdio communication setup

4. **Update `charts/aeterna/values.schema.json`**:
   - Add grepai schema validation

**Estimated Effort**: 4-6 hours

---

### ⏳ Phase 4: Documentation (PENDING)

**Planned Documentation**:

1. **`docs/grepai-integration.md`** - User guide
   - Overview and benefits
   - Installation instructions
   - Configuration options
   - Usage examples
   - Troubleshooting

2. **Update `charts/aeterna/README.md`**:
   - GrepAI section
   - Values documentation
   - Example configurations

3. **Architecture diagram**:
   - Component interaction
   - Data flow
   - Sidecar communication

**Estimated Effort**: 2-3 hours

---

### ⏳ Phase 5: Testing (PENDING)

**Planned Tests**:

1. **Unit tests** for MCP proxy tools
2. **Integration tests** for CLI commands
3. **Helm chart validation** tests
4. **End-to-end** tests with real GrepAI

**Estimated Effort**: 4-6 hours

---

## Code Statistics

**Total Implementation**:
- **Lines of Code**: 1,440
- **Files Created**: 9
- **Modules**: 2 (tools, cli)

**Breakdown**:
- MCP Tools: 906 lines (4 files)
- CLI Commands: 534 lines (5 files)

---

## Next Steps

### Immediate (Phase 3):
1. Add GrepAI configuration section to `values.yaml`
2. Create GrepAI ConfigMap template
3. Update deployment template with sidecar
4. Add init container configuration
5. Test sidecar deployment in kind cluster

### Follow-up (Phases 4-5):
6. Create comprehensive documentation
7. Add examples and tutorials
8. Write unit and integration tests
9. Test with real codebase (Aeterna itself)

---

## Usage Example

Once deployment is complete:

```bash
# Initialize GrepAI for a project
aeterna grepai init /path/to/project --embedder ollama --store qdrant

# Search for authentication logic
aeterna grepai search "user authentication flow" --limit 5

# Find who calls a function
aeterna grepai trace callers HandleLogin --recursive

# Build call graph
aeterna grepai trace graph AuthMiddleware --depth 2 --format dot | dot -Tpng > graph.png

# Check indexing status
aeterna grepai status --watch
```

---

## OpenSpec Compliance

This implementation follows the OpenSpec change proposal at:
`openspec/changes/add-grepai-integration/`

**Completed Requirements**:
- ✅ MCP tool proxy implementation
- ✅ CLI integration
- ✅ Tenant context support
- ⏳ Helm chart sidecar (in progress)
- ⏳ Shared backend configuration (in progress)
- ⏳ Documentation (planned)

**Status**: 40% complete (Phases 1-2 of 5)
