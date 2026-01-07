---
title: Memory System Specification
status: draft
version: 0.1.0
created: 2025-01-07
authors:
  - AI Systems Architecture Team
related:
  - 01-core-concepts.md
  - 04-memory-knowledge-sync.md
  - 05-adapter-architecture.md
---

# Memory System Specification

This document specifies the Memory System component: a hierarchical, provider-agnostic semantic memory store for AI agents.

## Table of Contents

1. [Overview](#overview)
2. [Layer Hierarchy](#layer-hierarchy)
3. [Memory Entry Schema](#memory-entry-schema)
4. [Core Operations](#core-operations)
5. [Layer Resolution](#layer-resolution)
6. [Provider Adapter Interface](#provider-adapter-interface)
7. [Memory Lifecycle](#memory-lifecycle)
8. [Error Handling](#error-handling)

---

## Overview

The Memory System provides:

- **Semantic storage**: Vector-based content for similarity search
- **Hierarchical scoping**: 7 layers from agent-specific to organization-wide
- **Provider abstraction**: Swap backends without code changes
- **Flexible retrieval**: Query across layers with precedence rules

```
┌─────────────────────────────────────────────────────────────────┐
│                      MEMORY SYSTEM                               │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Memory Manager                         │    │
│  │  • Coordinates all memory operations                     │    │
│  │  • Enforces layer rules                                  │    │
│  │  • Routes to provider adapter                            │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Layer Resolver                         │    │
│  │  • Determines target layers for operations               │    │
│  │  • Applies precedence rules                              │    │
│  │  • Merges results from multiple layers                   │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                  Provider Adapter                        │    │
│  │  • Translates to provider-specific API                   │    │
│  │  • Handles connection, auth, retries                     │    │
│  │  • Manages embedding generation                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Provider (Mem0, Letta, etc.)                │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer Hierarchy

### The Seven Layers

Memory is organized into seven hierarchical layers, from most specific to least specific.

**OpenSpec v1.0.0 Protocol Compliant:**

All memory operations SHALL implement the OpenSpec Knowledge Provider protocol endpoints:

1. **Discovery** (`GET /openspec/v1/knowledge`) - Expose provider capabilities
2. **Query** (`POST /openspec/v1/knowledge/query`) - Semantic search across layers
3. **Create** (`POST /openspec/v1/knowledge/create`) - Store new memory entries
4. **Update** (`PUT /openspec/v1/knowledge/{id}`) - Modify existing entries
5. **Delete** (`DELETE /openspec/v1/knowledge/{id}`) - Remove entries
6. **Batch** (`POST /openspec/v1/knowledge/batch`) - Bulk operations
7. **Stream** (`GET /openspec/v1/knowledge/stream`) - Real-time updates
8. **Metadata** (`GET /openspec/v1/knowledge/{id}/metadata`) - Entry metadata

**Rust Implementation Pattern:**

```rust
use async_trait::async_trait;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Memory Manager - Core coordination layer
pub struct MemoryManager {
    layers: RwLock<HashMap<MemoryLayer, LayerStore>>,
    embedding_provider: Arc<dyn EmbeddingProvider + Send + Sync>,
    config: MemoryConfig,
}

#[async_trait]
pub trait MemoryLayer: Send + Sync {
    async fn add(&self, entry: MemoryEntry) -> Result<String, MemoryError>;
    async fn query(&self, query: MemoryQuery) -> Result<Vec<SearchResult>, MemoryError>;
    async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError>;
    async fn delete(&self, id: &str) -> Result<bool, MemoryError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryLayer {
    /// Sub-millisecond, in-memory (Redis)
    Working,
    /// Milliseconds, cache with TTL (Redis)
    Session,
    /// Hours, PostgreSQL + pgvector
    Episodic,
    /// Days, Qdrant vector search
    Semantic,
    /// Weeks, PostgreSQL facts
    Procedural,
    /// Months, PostgreSQL + pgvector
    UserPersonal,
    /// Years, Qdrant long-term storage
    Archival,
}

pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub layer: MemoryLayer,
    pub metadata: MemoryMetadata,
    pub source: MemorySource,
    pub access_policy: AccessPolicy,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub version: Option<u32>,
}

pub struct MemoryMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub language: Option<String>,
    pub author: Option<String>,
    pub confidence: Option<f32>, // 0.0-1.0
    pub importance: Option<f32>, // 0.0-1.0
    pub custom: HashMap<String, serde_json::Value>,
}
```

---

### Rust Implementation: Layer Storage Providers

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  LAYER          SCOPE                    EXAMPLES                │
│  ─────────────────────────────────────────────────────────────  │
│                                                                  │
│  agent    ←── Per-agent instance         Agent-specific learnings│
│    │          (most specific)            Tool preferences        │
│    │                                                             │
│  user          Per-user                  User preferences        │
│    │                                     Communication style     │
│    │                                                             │
│  session       Per-session               Current task context    │
│    │           (conversation)            Recent decisions        │
│    │                                                             │
│  project       Per-project/repo          Project conventions     │
│    │                                     Tech stack choices      │
│    │                                                             │
│  team          Per-team                  Team standards          │
│    │                                     Shared knowledge        │
│    │                                                             │
│  org           Per-organization          Org-wide policies       │
│    │                                     Compliance rules        │
│    │                                                             │
│  company  ←── Per-company/tenant         Company standards       │
│               (least specific)           Global policies         │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Identifiers

Each layer requires specific identifiers to scope memory:

```typescript
interface LayerIdentifiers {
  /** Required for agent layer */
  agentId?: string;
  
  /** Required for user layer and below */
  userId?: string;
  
  /** Required for session layer and below */
  sessionId?: string;
  
  /** Required for project layer and below */
  projectId?: string;
  
  /** Required for team layer and below */
  teamId?: string;
  
  /** Required for org layer and below */
  orgId?: string;
  
  /** Required for company layer */
  companyId?: string;
}
```

### Layer Requirements Matrix

| Layer | agentId | userId | sessionId | projectId | teamId | orgId | companyId |
|-------|---------|--------|-----------|-----------|--------|-------|-----------|
| agent | ✓ | ✓ | - | - | - | - | - |
| user | - | ✓ | - | - | - | - | - |
| session | - | ✓ | ✓ | - | - | - | - |
| project | - | - | - | ✓ | - | - | - |
| team | - | - | - | - | ✓ | - | - |
| org | - | - | - | - | - | ✓ | - |
| company | - | - | - | - | - | - | ✓ |

---

## Memory Entry Schema

### Core Schema

```typescript
/**
 * A single memory entry in the system.
 */
interface MemoryEntry {
  /** Unique identifier (provider-generated or UUID) */
  id: string;
  
  /** The memory content (human-readable text) */
  content: string;
  
  /** Layer this memory belongs to */
  layer: MemoryLayer;
  
  /** Layer-specific identifiers */
  identifiers: LayerIdentifiers;
  
  /** Arbitrary metadata */
  metadata: MemoryMetadata;
  
  /** Creation timestamp (ISO 8601) */
  createdAt: string;
  
  /** Last update timestamp (ISO 8601) */
  updatedAt: string;
  
  /** Vector embedding (provider-specific format) */
  embedding?: number[];
}

type MemoryLayer = 
  | 'agent'
  | 'user'
  | 'session'
  | 'project'
  | 'team'
  | 'org'
  | 'company';
```

### Metadata Schema

```typescript
/**
 * Flexible metadata attached to memories.
 */
interface MemoryMetadata {
  /** Optional: Tags for categorization */
  tags?: string[];
  
  /** Optional: Source of this memory */
  source?: MemorySource;
  
  /** Optional: Pointer to knowledge item (see 04-memory-knowledge-sync.md) */
  knowledgePointer?: KnowledgePointer;
  
  /** Optional: Relevance score (0.0 - 1.0) */
  relevance?: number;
  
  /** Optional: Decay factor for aging */
  decayFactor?: number;
  
  /** Custom fields (string keys, JSON-serializable values) */
  [key: string]: unknown;
}

interface MemorySource {
  /** Source type */
  type: 'conversation' | 'tool_result' | 'knowledge_sync' | 'manual' | 'import';
  
  /** Optional: Reference to source (message ID, tool call ID, etc.) */
  reference?: string;
}

interface KnowledgePointer {
  /** Type of knowledge item */
  sourceType: 'adr' | 'policy' | 'pattern' | 'spec';
  
  /** ID of knowledge item */
  sourceId: string;
  
  /** Content hash at sync time */
  contentHash: string;
  
  /** Sync timestamp */
  syncedAt: string;
}
```

### Example Memory Entries

#### Agent-Level Memory

```json
{
  "id": "mem_agent_001",
  "content": "When debugging TypeScript, always check tsconfig.json first",
  "layer": "agent",
  "identifiers": {
    "agentId": "agent_debugger",
    "userId": "user_123"
  },
  "metadata": {
    "tags": ["debugging", "typescript"],
    "source": {
      "type": "conversation",
      "reference": "msg_abc123"
    }
  },
  "createdAt": "2025-01-07T10:30:00Z",
  "updatedAt": "2025-01-07T10:30:00Z"
}
```

#### Project-Level Memory (Knowledge Pointer)

```json
{
  "id": "mem_proj_042",
  "content": "Use PostgreSQL for all new services per ADR-042",
  "layer": "project",
  "identifiers": {
    "projectId": "proj_backend_api"
  },
  "metadata": {
    "tags": ["database", "architecture"],
    "source": {
      "type": "knowledge_sync"
    },
    "knowledgePointer": {
      "sourceType": "adr",
      "sourceId": "adr-042-database-selection",
      "contentHash": "sha256:abc123def456...",
      "syncedAt": "2025-01-07T09:00:00Z"
    }
  },
  "createdAt": "2025-01-07T09:00:00Z",
  "updatedAt": "2025-01-07T09:00:00Z"
}
```

---

## Core Operations

### Operation: Add Memory

Add a new memory entry to a specific layer.

```typescript
interface AddMemoryInput {
  /** Memory content (required) */
  content: string;
  
  /** Target layer (required) */
  layer: MemoryLayer;
  
  /** Layer identifiers (required fields depend on layer) */
  identifiers: LayerIdentifiers;
  
  /** Optional metadata */
  metadata?: Partial<MemoryMetadata>;
}

interface AddMemoryOutput {
  /** Created memory entry */
  memory: MemoryEntry;
  
  /** Whether embedding was generated */
  embeddingGenerated: boolean;
}
```

**Behavior:**

1. Validate `identifiers` contains required fields for `layer`
2. Generate embedding from `content` via provider
3. Persist to provider with layer isolation
4. Return created entry with generated `id`

**Errors:**

| Error | Condition |
|-------|-----------|
| `INVALID_LAYER` | Unknown layer value |
| `MISSING_IDENTIFIER` | Required identifier not provided |
| `CONTENT_TOO_LONG` | Content exceeds provider limit |
| `EMBEDDING_FAILED` | Embedding generation failed |
| `PROVIDER_ERROR` | Provider-specific error |

### Operation: Search Memory

Search for memories semantically matching a query.

```typescript
interface SearchMemoryInput {
  /** Search query (natural language) */
  query: string;
  
  /** Layers to search (default: all accessible layers) */
  layers?: MemoryLayer[];
  
  /** Layer identifiers for scoping */
  identifiers: LayerIdentifiers;
  
  /** Maximum results per layer (default: 10) */
  limit?: number;
  
  /** Minimum similarity threshold (0.0 - 1.0, default: 0.7) */
  threshold?: number;
  
  /** Optional: Filter by metadata */
  filter?: MetadataFilter;
}

interface MetadataFilter {
  /** Match any of these tags */
  tags?: string[];
  
  /** Match source type */
  sourceType?: MemorySource['type'];
  
  /** Only knowledge pointers */
  hasKnowledgePointer?: boolean;
  
  /** Custom field filters */
  custom?: Record<string, unknown>;
}

interface SearchMemoryOutput {
  /** Search results, ordered by relevance */
  results: MemorySearchResult[];
  
  /** Total results before limit */
  totalCount: number;
  
  /** Layers that were searched */
  searchedLayers: MemoryLayer[];
}

interface MemorySearchResult {
  /** The memory entry */
  memory: MemoryEntry;
  
  /** Similarity score (0.0 - 1.0) */
  score: number;
  
  /** Layer this result came from */
  layer: MemoryLayer;
}
```

**Behavior:**

1. Generate embedding for `query`
2. For each layer in `layers`:
   a. Verify `identifiers` provides required fields
   b. Execute vector similarity search
   c. Apply `threshold` filter
   d. Apply `filter` if provided
3. Merge results using layer precedence (see [Layer Resolution](#layer-resolution))
4. Return top `limit` results

**Errors:**

| Error | Condition |
|-------|-----------|
| `INVALID_LAYER` | Unknown layer in `layers` array |
| `MISSING_IDENTIFIER` | Required identifier for layer not provided |
| `QUERY_TOO_LONG` | Query exceeds embedding limit |
| `PROVIDER_ERROR` | Provider-specific error |

### Operation: Get Memory

Retrieve a specific memory by ID.

```typescript
interface GetMemoryInput {
  /** Memory ID */
  id: string;
}

interface GetMemoryOutput {
  /** The memory entry, or null if not found */
  memory: MemoryEntry | null;
}
```

**Behavior:**

1. Look up memory by `id` in provider
2. Return entry or null

### Operation: Update Memory

Update an existing memory's content or metadata.

```typescript
interface UpdateMemoryInput {
  /** Memory ID */
  id: string;
  
  /** New content (optional, triggers re-embedding) */
  content?: string;
  
  /** Metadata updates (merged with existing) */
  metadata?: Partial<MemoryMetadata>;
}

interface UpdateMemoryOutput {
  /** Updated memory entry */
  memory: MemoryEntry;
  
  /** Whether embedding was regenerated */
  embeddingRegenerated: boolean;
}
```

**Behavior:**

1. Fetch existing memory by `id`
2. If `content` changed, regenerate embedding
3. Merge `metadata` with existing (shallow merge)
4. Update `updatedAt` timestamp
5. Persist to provider

**Errors:**

| Error | Condition |
|-------|-----------|
| `MEMORY_NOT_FOUND` | No memory with given ID |
| `CONTENT_TOO_LONG` | New content exceeds limit |
| `EMBEDDING_FAILED` | Re-embedding failed |

### Operation: Delete Memory

Remove a memory from the system.

```typescript
interface DeleteMemoryInput {
  /** Memory ID */
  id: string;
}

interface DeleteMemoryOutput {
  /** Whether deletion succeeded */
  success: boolean;
}
```

**Behavior:**

1. Remove memory from provider
2. Return success status

### Operation: List Memories

List memories in a specific layer with pagination.

```typescript
interface ListMemoriesInput {
  /** Target layer */
  layer: MemoryLayer;
  
  /** Layer identifiers */
  identifiers: LayerIdentifiers;
  
  /** Pagination cursor */
  cursor?: string;
  
  /** Page size (default: 50, max: 100) */
  limit?: number;
  
  /** Optional: Filter by metadata */
  filter?: MetadataFilter;
}

interface ListMemoriesOutput {
  /** Memories in this page */
  memories: MemoryEntry[];
  
  /** Cursor for next page (null if no more) */
  nextCursor: string | null;
  
  /** Total count in layer */
  totalCount: number;
}
```

---

## Layer Resolution

### Precedence Rules

When searching across multiple layers, results are merged using these rules:

```
┌─────────────────────────────────────────────────────────────────┐
│                   LAYER PRECEDENCE                               │
│                                                                  │
│  1. agent    (highest priority - most specific)                 │
│  2. user                                                        │
│  3. session                                                     │
│  4. project                                                     │
│  5. team                                                        │
│  6. org                                                         │
│  7. company  (lowest priority - least specific)                 │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Merge Algorithm

```typescript
function mergeSearchResults(
  resultsByLayer: Map<MemoryLayer, MemorySearchResult[]>,
  limit: number
): MemorySearchResult[] {
  // 1. Flatten all results
  const allResults: MemorySearchResult[] = [];
  for (const [layer, results] of resultsByLayer) {
    allResults.push(...results);
  }
  
  // 2. Sort by: layer precedence (primary), score (secondary)
  allResults.sort((a, b) => {
    const layerDiff = getLayerPrecedence(a.layer) - getLayerPrecedence(b.layer);
    if (layerDiff !== 0) return layerDiff;
    return b.score - a.score; // Higher score first
  });
  
  // 3. Deduplicate by content similarity (optional)
  const deduped = deduplicateBySimilarity(allResults, 0.95);
  
  // 4. Return top N
  return deduped.slice(0, limit);
}

function getLayerPrecedence(layer: MemoryLayer): number {
  const precedence: Record<MemoryLayer, number> = {
    agent: 1,
    user: 2,
    session: 3,
    project: 4,
    team: 5,
    org: 6,
    company: 7
  };
  return precedence[layer];
}
```

### Override Behavior

More specific layers **override** less specific layers when content conflicts:

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  company layer: "Use spaces for indentation"                    │
│                              │                                   │
│                              ▼                                   │
│  project layer: "Use tabs for indentation"  ◄── WINS            │
│                                                                  │
│  Result: Agent uses tabs (project overrides company)            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Layer Access Control

Layers are only searchable if appropriate identifiers are provided:

```typescript
function getAccessibleLayers(identifiers: LayerIdentifiers): MemoryLayer[] {
  const layers: MemoryLayer[] = [];
  
  // Always accessible with company ID
  if (identifiers.companyId) layers.push('company');
  
  // Org requires org ID
  if (identifiers.orgId) layers.push('org');
  
  // Team requires team ID
  if (identifiers.teamId) layers.push('team');
  
  // Project requires project ID
  if (identifiers.projectId) layers.push('project');
  
  // Session requires user + session ID
  if (identifiers.userId && identifiers.sessionId) layers.push('session');
  
  // User requires user ID
  if (identifiers.userId) layers.push('user');
  
  // Agent requires agent + user ID
  if (identifiers.agentId && identifiers.userId) layers.push('agent');
  
  return layers;
}
```

---

## Provider Adapter Interface

### Interface Definition

All memory providers must implement this interface:

```typescript
/**
 * Memory provider adapter interface.
 * Implement this to add support for a new storage backend.
 */
interface MemoryProviderAdapter {
  /** Provider name (e.g., "mem0", "letta", "chroma") */
  readonly name: string;
  
  /** Provider version */
  readonly version: string;
  
  /** Initialize the provider connection */
  initialize(config: ProviderConfig): Promise<void>;
  
  /** Clean up resources */
  shutdown(): Promise<void>;
  
  /** Health check */
  healthCheck(): Promise<HealthCheckResult>;
  
  // Core operations
  add(input: AddMemoryInput): Promise<AddMemoryOutput>;
  search(input: SearchMemoryInput): Promise<SearchMemoryOutput>;
  get(input: GetMemoryInput): Promise<GetMemoryOutput>;
  update(input: UpdateMemoryInput): Promise<UpdateMemoryOutput>;
  delete(input: DeleteMemoryInput): Promise<DeleteMemoryOutput>;
  list(input: ListMemoriesInput): Promise<ListMemoriesOutput>;
  
  // Embedding operations
  generateEmbedding(content: string): Promise<number[]>;
  
  // Bulk operations (optional)
  bulkAdd?(inputs: AddMemoryInput[]): Promise<AddMemoryOutput[]>;
  bulkDelete?(ids: string[]): Promise<{ deleted: number; failed: string[] }>;
}

interface ProviderConfig {
  /** Provider-specific configuration */
  [key: string]: unknown;
}

interface HealthCheckResult {
  /** Overall health status */
  status: 'healthy' | 'degraded' | 'unhealthy';
  
  /** Latency in milliseconds */
  latencyMs: number;
  
  /** Optional: Detailed component health */
  components?: Record<string, {
    status: 'healthy' | 'degraded' | 'unhealthy';
    message?: string;
  }>;
}
```

### Layer Isolation

Providers MUST ensure layer isolation. Implementation strategies:

#### Strategy 1: Namespace by Layer

```
Collection: memories_agent_{agentId}_{userId}
Collection: memories_user_{userId}
Collection: memories_session_{userId}_{sessionId}
Collection: memories_project_{projectId}
...
```

#### Strategy 2: Metadata Filtering

```json
{
  "content": "...",
  "metadata": {
    "_layer": "project",
    "_projectId": "proj_123"
  }
}
```

Query with filter: `metadata._layer == "project" AND metadata._projectId == "proj_123"`

#### Strategy 3: Tenant Partitioning

Use provider's native multi-tenancy:
- Qdrant: Separate collections per layer
- Pinecone: Namespaces per layer
- Chroma: Collections per layer

---

## Memory Lifecycle

### State Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│                        ┌─────────┐                              │
│                        │ CREATED │                              │
│                        └────┬────┘                              │
│                             │                                    │
│                             ▼                                    │
│                        ┌─────────┐                              │
│              ┌────────►│ ACTIVE  │◄────────┐                    │
│              │         └────┬────┘         │                    │
│              │              │              │                    │
│              │    ┌─────────┴─────────┐    │                    │
│              │    │                   │    │                    │
│              │    ▼                   ▼    │                    │
│         ┌────┴────┐             ┌─────────┐                     │
│         │ UPDATED │             │ DECAYED │                     │
│         └─────────┘             └────┬────┘                     │
│                                      │                          │
│                                      ▼                          │
│                                ┌──────────┐                     │
│                                │ ARCHIVED │                     │
│                                └────┬─────┘                     │
│                                     │                           │
│                                     ▼                           │
│                                ┌─────────┐                      │
│                                │ DELETED │                      │
│                                └─────────┘                      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Memory Decay (Optional)

Providers MAY support memory decay to reduce old memory relevance:

```typescript
interface DecayConfig {
  /** Enable decay */
  enabled: boolean;
  
  /** Decay rate per day (0.0 - 1.0) */
  ratePerDay: number;
  
  /** Minimum relevance before archival */
  archiveThreshold: number;
  
  /** Layers exempt from decay */
  exemptLayers: MemoryLayer[];
}
```

**Decay Formula:**

```
relevance(t) = initial_relevance * (1 - rate)^days_since_creation
```

### Memory Consolidation (Optional)

Providers MAY support consolidation to merge similar memories:

```typescript
interface ConsolidationConfig {
  /** Enable consolidation */
  enabled: boolean;
  
  /** Similarity threshold for merging (0.0 - 1.0) */
  similarityThreshold: number;
  
  /** Maximum memories before triggering consolidation */
  maxMemoriesBeforeTrigger: number;
  
  /** Layers to consolidate */
  targetLayers: MemoryLayer[];
}
```

### Session Memory Cleanup

Session-layer memories have special lifecycle:

```typescript
interface SessionCleanupConfig {
  /** Auto-delete session memories after session ends */
  autoDelete: boolean;
  
  /** Retention period after session end (e.g., "7d", "30d") */
  retentionPeriod?: string;
  
  /** Promote important memories to user layer */
  promoteImportant: boolean;
  
  /** Threshold for promotion (0.0 - 1.0) */
  promotionThreshold?: number;
}
```

---

## Error Handling

### Error Response Format

```typescript
interface MemoryError {
  /** Error code */
  code: MemoryErrorCode;
  
  /** Human-readable message */
  message: string;
  
  /** Operation that failed */
  operation: string;
  
  /** Additional context */
  details?: Record<string, unknown>;
  
  /** Whether operation can be retried */
  retryable: boolean;
}

type MemoryErrorCode =
  | 'INVALID_LAYER'
  | 'MISSING_IDENTIFIER'
  | 'MEMORY_NOT_FOUND'
  | 'CONTENT_TOO_LONG'
  | 'QUERY_TOO_LONG'
  | 'EMBEDDING_FAILED'
  | 'PROVIDER_ERROR'
  | 'RATE_LIMITED'
  | 'UNAUTHORIZED'
  | 'CONFIGURATION_ERROR';
```

### Error Handling Guidelines

| Error Code | Recommended Action |
|------------|-------------------|
| `INVALID_LAYER` | Fix input, do not retry |
| `MISSING_IDENTIFIER` | Add required identifier, do not retry |
| `MEMORY_NOT_FOUND` | Check ID, may be deleted |
| `CONTENT_TOO_LONG` | Truncate or split content |
| `QUERY_TOO_LONG` | Shorten query |
| `EMBEDDING_FAILED` | Retry with backoff |
| `PROVIDER_ERROR` | Retry with backoff, check provider status |
| `RATE_LIMITED` | Retry after delay (use `Retry-After` if provided) |
| `UNAUTHORIZED` | Check credentials, do not retry |
| `CONFIGURATION_ERROR` | Fix configuration, do not retry |

### Retry Strategy

```typescript
interface RetryConfig {
  /** Maximum retry attempts */
  maxAttempts: number;
  
  /** Initial delay in milliseconds */
  initialDelayMs: number;
  
  /** Maximum delay in milliseconds */
  maxDelayMs: number;
  
  /** Backoff multiplier */
  backoffMultiplier: number;
  
  /** Error codes to retry */
  retryableCodes: MemoryErrorCode[];
}

const defaultRetryConfig: RetryConfig = {
  maxAttempts: 3,
  initialDelayMs: 1000,
  maxDelayMs: 30000,
  backoffMultiplier: 2,
  retryableCodes: ['EMBEDDING_FAILED', 'PROVIDER_ERROR', 'RATE_LIMITED']
};
```

---

## Implementation Notes

### Thread Safety

Memory operations MUST be thread-safe. Implementations should:

1. Use atomic operations where possible
2. Implement optimistic locking for updates
3. Handle concurrent access to same memory gracefully

### Caching

Implementations MAY cache:

- Embeddings for frequently accessed content
- Layer resolution results
- Provider connection metadata

Implementations MUST NOT cache:

- Memory content (may be stale)
- Search results (query-dependent)

### Observability

Implementations SHOULD emit metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `memory.operations.total` | Counter | Total operations by type |
| `memory.operations.errors` | Counter | Failed operations by error code |
| `memory.operations.latency` | Histogram | Operation latency in ms |
| `memory.search.results` | Histogram | Number of results per search |
| `memory.storage.size` | Gauge | Total memories by layer |
| 
---

## Rust Storage Implementations

### Working Memory Layer (Redis - In-Memory HashMap)

**Purpose**: Ultra-fast, ephemeral context with microsecond latency

```rust
use redis::AsyncCommands;
use tokio::sync::RwLock;
use std::collections::HashMap;

/// Working memory - In-memory HashMap backed by Redis persistence
pub struct WorkingMemoryLayer {
    cache: Arc<RwLock<HashMap<String, MemoryEntry>>>,
    redis: Arc<redis::Client>,
}

impl WorkingMemoryLayer {
    pub async fn add(&self, entry: MemoryEntry) -> Result<String, MemoryError> {
        // O(1) in-memory insertion
        let id = uuid::Uuid::new_v4().to_string();
        let mut cache = self.cache.write().await;
        cache.insert(id.clone(), entry);

        // Async persist to Redis (non-blocking)
        let redis = Arc::clone(&self.redis);
        tokio::spawn(async move {
            let _: Result<(), _> = redis
                .set(&id, serde_json::to_string(&entry).unwrap(), None, None, false)
                .await;
        });

        Ok(id)
    }

    pub async fn query(&self, query: &MemoryQuery) -> Result<Vec<SearchResult>, MemoryError> {
        let cache = self.cache.read().await;
        let embedding = self.embedding_provider.embed(&query.text).await?;

        // O(n) similarity search (n typically < 1000 for working memory)
        let results: Vec<SearchResult> = cache
            .values()
            .filter_map(|entry| {
                if let Some(ref emb) = entry.embedding {
                    let similarity = cosine_similarity(emb, &embedding);
                    if similarity > query.threshold {
                        Some(SearchResult {
                            entry: entry.clone(),
                            score: similarity,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }

    // O(1) retrieval
    pub async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        let cache = self.cache.read().await;
        Ok(cache.get(id).cloned())
    }

    // Automatic eviction - clear on operation completion
    pub async fn clear(&self) -> Result<(), MemoryError> {
        let mut cache = self.cache.write().await;
        cache.clear();
        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    // Optimized cosine similarity computation
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot_product / (magnitude_a * magnitude_b)
}
```

**Performance**:
- Add: ~5µs (in-memory) + ~1ms (async Redis)
- Query: ~50µs - ~100µs (in-memory vector search)
- Get: ~1µs (in-memory)
- Capacity: ~10,000 entries (fits in memory)

---

### Session Memory Layer (Redis with TTL)

**Purpose**: Per-conversation context with automatic expiration

```rust
use redis::AsyncCommands;

/// Session memory - Redis with TTL
pub struct SessionMemoryLayer {
    redis: Arc<redis::Client>,
    default_ttl: Duration,  // Default: 1 hour
}

impl SessionMemoryLayer {
    pub async fn add(&self, entry: MemoryEntry, ttl: Option<Duration>) -> Result<String, MemoryError> {
        let id = uuid::Uuid::new_v4().to_string();
        let ttl = ttl.unwrap_or(self.default_ttl);

        let mut conn = self.redis.get_multiplexed_async_connection().await?;

        // Store with TTL - automatic cleanup
        conn.set(&id, serde_json::to_string(&entry).unwrap(), Some(ttl), None, false)
            .await?;

        // Index for search
        conn.sadd(&format!("session:{}", entry.session_id.unwrap_or("default")), &id)
            .await?;

        Ok(id)
    }

    pub async fn query(&self, query: &MemoryQuery) -> Result<Vec<SearchResult>, MemoryError> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let session_key = format!("session:{}", query.session_id.unwrap_or("default"));

        // Get all IDs in session
        let ids: Vec<String> = conn.smembers(&session_key).await?;

        // Batch get entries
        let entries: Vec<MemoryEntry> = conn.mget(&ids).await?
            .into_iter()
            .filter_map(|data| data.and_then(|s| serde_json::from_str(s).ok()))
            .collect();

        // Filter by similarity
        let embedding = self.embedding_provider.embed(&query.text).await?;
        let results: Vec<SearchResult> = entries
            .into_iter()
            .filter_map(|entry| {
                if let Some(ref emb) = entry.embedding {
                    let similarity = cosine_similarity(emb, &embedding);
                    if similarity > query.threshold {
                        Some(SearchResult {
                            entry,
                            score: similarity,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        Ok(results)
    }

    // TTL-aware get
    pub async fn get(&self, id: &str) -> Result<Option<MemoryEntry>, MemoryError> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let data: Option<String> = conn.get(id).await?;
        data.and_then(|s| serde_json::from_str(s).ok())
            .map(Ok)
            .unwrap_or(Ok(None))
    }
}
```

**Performance**:
- Add: ~5ms (Redis SET)
- Query: ~20ms (Redis SMEMBERS + MGET + vector search)
- Get: ~5ms (Redis GET)
- Capacity: ~100,000 entries per session
- TTL: Configurable (default 1 hour)

---

### Episodic Memory Layer (PostgreSQL + pgvector)

**Purpose**: Event storage with time-series indexing

```rust
use sqlx::postgres::PgPoolOptions;
use sqlx::{Postgres, Pool};

/// Episodic memory - PostgreSQL with time-series queries
pub struct EpisodicMemoryLayer {
    pool: Pool<Postgres>,
}

impl EpisodicMemoryLayer {
    pub async fn add(&self, entry: MemoryEntry) -> Result<String, MemoryError> {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding = self.embedding_provider.embed(&entry.content).await?;

        // Insert with generated embedding
        sqlx::query!(
            r#"
            INSERT INTO episodic_memory (id, content, embedding, layer, metadata, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            RETURNING id
            "#
        )
        .bind(&id)
        .bind(&entry.content)
        .bind(&embedding)
        .bind("episodic")
        .bind(serde_json::to_value(&entry.metadata)?)
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn query(&self, query: &MemoryQuery) -> Result<Vec<SearchResult>, MemoryError> {
        let embedding = self.embedding_provider.embed(&query.text).await?;

        // pgvector similarity search with < operator
        let rows = sqlx::query!(
            r#"
            SELECT id, content, embedding, metadata, created_at,
                   1 - (embedding <=> $1::vector) as score
            FROM episodic_memory
            WHERE 1 - (embedding <=> $1::vector) > $2
            ORDER BY embedding <=> $1::vector
            LIMIT $3
            "#
        )
        .bind(&embedding)
        .bind(query.threshold.unwrap_or(0.7))
        .bind(query.limit.unwrap_or(10) as i64)
        .fetch_all(&self.pool)
        .await?;

        let results: Vec<SearchResult> = rows
            .into_iter()
            .map(|row| SearchResult {
                entry: MemoryEntry {
                    id: row.id,
                    content: row.content,
                    embedding: Some(row.embedding),
                    metadata: serde_json::from_value(row.metadata)?,
                    layer: MemoryLayer::Episodic,
                    ..Default::default()
                },
                score: row.score,
            })
            .collect();

        Ok(results)
    }

    // Time-range queries
    pub async fn query_time_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<MemoryEntry>, MemoryError> {
        let rows = sqlx::query!(
            "SELECT * FROM episodic_memory WHERE created_at >= $1 AND created_at <= $2 ORDER BY created_at DESC"
        )
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| MemoryEntry {
            id: row.id,
            content: row.content,
            metadata: serde_json::from_value(row.metadata)?,
            layer: MemoryLayer::Episodic,
            ..Default::default()
        }).collect::<Result<_, _>>().map_err(MemoryError::from)
    }
}
```

**Performance**:
- Add: ~50ms (INSERT with embedding)
- Query: ~50-100ms (pgvector similarity search)
- Time-range query: ~30ms (indexed)
- Capacity: Unlimited (PostgreSQL)
- Indexing: B-tree on `created_at`, HNSW on `embedding` (via pgvector)

---

### Semantic Memory Layer (Qdrant Vector Database)

**Purpose**: Long-term semantic search with high-dimensional vectors

```rust
use qdrant_client::prelude::*;

/// Semantic memory - Qdrant vector database
pub struct SemanticMemoryLayer {
    client: QdrantClient,
    collection_name: String,
}

impl SemanticMemoryLayer {
    pub async fn add(&self, entry: MemoryEntry) -> Result<String, MemoryError> {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding = self.embedding_provider.embed(&entry.content).await?;

        // Insert point into Qdrant
        self.client
            .upsert_points_blocking(
                &self.collection_name,
                None,
                None,
                vec![PointStruct::new(
                    id.clone(),
                    embedding,
                    Payload {
                        index_data: Some(entry.id.clone()),
                        payload: Some(serde_json::to_value(&entry.metadata)?),
                    }
                )],
                Some(1),
                None
            )
            .await?;

        Ok(id)
    }

    pub async fn query(&self, query: &MemoryQuery) -> Result<Vec<SearchResult>, MemoryError> {
        let embedding = self.embedding_provider.embed(&query.text).await?;

        // Search in Qdrant
        let results = self.client
            .search_points(
                &SearchPoints {
                    collection_name: self.collection_name.clone(),
                    limit: Some(query.limit.unwrap_or(10) as u64),
                    vector: Some(embedding),
                    score_threshold: Some(query.threshold.unwrap_or(0.0)),
                    with_payload: Some(true.into()),
                    ..Default::default()
                }
            )
            .await?;

        let search_results: Vec<SearchResult> = results
            .result
            .into_iter()
            .map(|point| SearchResult {
                entry: MemoryEntry {
                    id: point.id,
                    content: point.payload.as_ref()
                        .and_then(|p| p.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    embedding: Some(point.vector),
                    metadata: point.payload.as_ref()
                        .and_then(|p| p.get("metadata"))
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default(),
                    layer: MemoryLayer::Semantic,
                    ..Default::default()
                },
                score: point.score,
            })
            .collect();

        Ok(search_results)
    }

    // Batch upsert for performance
    pub async fn batch_add(&self, entries: Vec<MemoryEntry>) -> Result<Vec<String>, MemoryError> {
        let embeddings = self.embedding_provider.embed_batch(
            entries.iter().map(|e| e.content.as_str()).collect()
        ).await?;

        let points: Vec<PointStruct> = entries
            .into_iter()
            .zip(embeddings.into_iter())
            .map(|(entry, embedding)| {
                let id = uuid::Uuid::new_v4().to_string();
                PointStruct::new(
                    id.clone(),
                    embedding,
                    Payload {
                        index_data: Some(id.clone()),
                        payload: Some(serde_json::to_value(&entry.metadata).unwrap()),
                    }
                )
            })
            .collect();

        self.client
            .upsert_points_blocking(&self.collection_name, None, None, points, Some(len(points) as u64), None)
            .await?;

        Ok(points.iter().map(|p| p.id.clone().unwrap()).collect())
    }
}
```

**Performance**:
- Add: ~5ms (Qdrant UPSERT)
- Query: ~50-200ms (HNSW search, depends on collection size)
- Batch add: ~10ms per 100 points
- Capacity: Millions of vectors (limited by RAM)
- Indexing: HNSW with configurable `m`, `ef_construction`

---

### Procedural Memory Layer (PostgreSQL Facts)

**Purpose**: Fact storage for declarative knowledge

```rust
/// Procedural memory - PostgreSQL facts
pub struct ProceduralMemoryLayer {
    pool: Pool<Postgres>,
}

impl ProceduralMemoryLayer {
    pub async fn add_fact(&self, fact: Fact) -> Result<String, MemoryError> {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query!(
            r#"
            INSERT INTO procedural_facts (id, fact_type, subject, predicate, object, confidence, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(&id)
        .bind(&fact.fact_type)
        .bind(&fact.subject)
        .bind(&fact.predicate)
        .bind(&fact.object)
        .bind(fact.confidence.unwrap_or(1.0))
        .bind(serde_json::to_value(&fact.metadata)?)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn query_facts(&self, query: FactQuery) -> Result<Vec<Fact>, MemoryError> {
        let sql = self.build_fact_query(query)?;
        let rows = sqlx::query_as(&sql)
            .bind_all(query.bindings())
            .fetch_all(&self.pool)
            .await?;

        Ok(rows)
    }

    fn build_fact_query(&self, query: FactQuery) -> Result<String, MemoryError> {
        // Build flexible SPARQL-like query
        let mut clauses = Vec::new();
        let mut bindings = Vec::new();

        if let Some(subject) = &query.subject {
            clauses.push("subject = $?");
            bindings.push(subject.clone());
        }
        if let Some(predicate) = &query.predicate {
            clauses.push("predicate = $?");
            bindings.push(predicate.clone());
        }
        if let Some(object) = &query.object {
            clauses.push("object = $?");
            bindings.push(object.clone());
        }
        if let Some(fact_type) = &query.fact_type {
            clauses.push("fact_type = $?");
            bindings.push(fact_type.clone());
        }

        let where_clause = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };

        Ok(format!("SELECT * FROM procedural_facts {} ORDER BY confidence DESC LIMIT {}", where_clause, query.limit.unwrap_or(100)))
    }
}

pub struct Fact {
    pub fact_type: String,  // "uses", "implements", "located_at"
    pub subject: String,     // "User", "Database", "API"
    pub predicate: String,   // "prefers", "has_version", "in_location"
    pub object: String,      // "dark_mode", "v16", "us-east-1"
    pub confidence: Option<f32>,
    pub metadata: serde_json::Value,
}
```

**Performance**:
- Add: ~30ms (INSERT)
- Query: ~30ms (indexed queries)
- Capacity: Millions of facts
- Indexing: Composite indexes on `(subject, predicate)`, `(fact_type, confidence)`

---

---

**Next**: [03-knowledge-repository.md](./03-knowledge-repository.md) - Knowledge Repository Specification
