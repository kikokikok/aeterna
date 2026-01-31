/**
 * Aeterna OpenCode Plugin - Type Definitions
 *
 * Shared types for the plugin, matching the Aeterna Rust backend API.
 */

// =============================================================================
// Memory Types
// =============================================================================

/** Memory layers in Aeterna's 7-layer hierarchy (most specific to least) */
export type MemoryLayer =
  | "agent"
  | "user"
  | "session"
  | "project"
  | "team"
  | "org"
  | "company";

/** Memory entry as stored in Aeterna */
export interface MemoryEntry {
  id: string;
  content: string;
  layer: MemoryLayer;
  importance: number;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  sessionId?: string;
  metadata?: Record<string, unknown>;
}

/** Memory search result with relevance score */
export interface MemorySearchResult {
  memory: MemoryEntry;
  score: number;
  highlights?: string[];
}

/** Parameters for adding a memory */
export interface MemoryAddParams {
  content: string;
  layer?: MemoryLayer;
  tags?: string[];
  importance?: number;
  sessionId?: string;
  metadata?: Record<string, unknown>;
}

/** Parameters for searching memories */
export interface MemorySearchParams {
  query: string;
  layers?: MemoryLayer[];
  limit?: number;
  threshold?: number;
  sessionId?: string;
  tags?: string[];
}

/** Parameters for promoting a memory */
export interface MemoryPromoteParams {
  memoryId: string;
  targetLayer: MemoryLayer;
  reason?: string;
}

// =============================================================================
// Knowledge Types
// =============================================================================

/** Knowledge scope levels */
export type KnowledgeScope = "project" | "team" | "org" | "company";

/** Knowledge item types */
export type KnowledgeType = "adr" | "pattern" | "policy" | "reference";

/** Knowledge entry from the repository */
export interface KnowledgeEntry {
  id: string;
  type: KnowledgeType;
  title: string;
  content: string;
  scope: KnowledgeScope;
  tags: string[];
  createdAt: string;
  updatedAt: string;
  status: "draft" | "approved" | "deprecated";
  author?: string;
  metadata?: Record<string, unknown>;
}

/** Knowledge search result with relevance */
export interface KnowledgeSearchResult {
  knowledge: KnowledgeEntry;
  score: number;
  summary?: string;
}

/** Parameters for querying knowledge */
export interface KnowledgeQueryParams {
  query: string;
  scope?: KnowledgeScope;
  types?: KnowledgeType[];
  limit?: number;
  threshold?: number;
}

/** Parameters for proposing new knowledge */
export interface KnowledgeProposeParams {
  type: KnowledgeType;
  title: string;
  content: string;
  scope: KnowledgeScope;
  tags?: string[];
  metadata?: Record<string, unknown>;
}

// =============================================================================
// Governance Types
// =============================================================================

/** Governance event types */
export type GovernanceEventType =
  | "ProposalApproved"
  | "ProposalRejected"
  | "DriftDetected"
  | "PolicyViolation";

/** Governance event notification */
export interface GovernanceEvent {
  type: GovernanceEventType;
  message: string;
  resource?: string;
  timestamp: string;
  metadata?: Record<string, unknown>;
}

/** Sync status between memory and knowledge */
export interface SyncStatus {
  lastSync: string;
  pendingPromotions: number;
  pendingProposals: number;
  syncHealth: "healthy" | "degraded" | "error";
  errors?: string[];
}

/** Governance status for the project/team/org */
export interface GovernanceStatus {
  activePolicies: number;
  pendingProposals: number;
  recentViolations: number;
  driftDetected: boolean;
  notifications: GovernanceEvent[];
}

// =============================================================================
// Session Types
// =============================================================================

/** Session context for Aeterna operations */
export interface SessionContext {
  sessionId: string;
  userId?: string;
  project?: string;
  team?: string;
  org?: string;
  company?: string;
  startedAt: string;
}

/** Tool execution record for capture */
export interface ToolExecutionRecord {
  tool: string;
  sessionId: string;
  callId: string;
  title?: string;
  args?: Record<string, unknown>;
  output?: string;
  metadata?: Record<string, unknown>;
  timestamp: number;
  duration?: number;
  success: boolean;
  error?: string;
}

/** Project context from Aeterna */
export interface ProjectContext {
  project: {
    name: string;
    path?: string;
  };
  team?: {
    name: string;
  };
  org?: {
    name: string;
  };
  policies: Array<{
    name: string;
    summary: string;
  }>;
  recentMemories: Array<{
    id: string;
    summary: string;
  }>;
}

// =============================================================================
// Client Configuration
// =============================================================================

/** Configuration for the Aeterna client */
export interface AeternaClientConfig {
  /** Project name for context */
  project: string;
  /** Working directory */
  directory: string;
  /** Aeterna server URL (defaults to AETERNA_SERVER_URL env var) */
  serverUrl?: string;
  /** API token (defaults to AETERNA_TOKEN env var) */
  token?: string;
  /** Team context */
  team?: string;
  /** Organization context */
  org?: string;
  /** User ID */
  userId?: string;
}

/** Plugin configuration from .aeterna/config.toml */
export interface PluginConfig {
  capture: {
    enabled: boolean;
    sensitivity: "low" | "medium" | "high";
    autoPromote: boolean;
    sampleRate?: number;
    debounceMs?: number;
  };
  knowledge: {
    injectionEnabled: boolean;
    maxItems: number;
    threshold: number;
    cacheTtlSeconds?: number;
    timeoutMs?: number;
  };
  governance: {
    notifications: boolean;
    driftAlerts: boolean;
  };
  session: {
    storageTtlHours?: number;
    useRedis?: boolean;
  };
  experimental: {
    systemPromptHook: boolean;
    permissionHook: boolean;
  };
}

/** Default plugin configuration */
export const DEFAULT_CONFIG: PluginConfig = {
  capture: {
    enabled: true,
    sensitivity: "medium",
    autoPromote: true,
    sampleRate: 1.0,
    debounceMs: 500,
  },
  knowledge: {
    injectionEnabled: true,
    maxItems: 3,
    threshold: 0.75,
    cacheTtlSeconds: 60,
    timeoutMs: 200,
  },
  governance: {
    notifications: true,
    driftAlerts: true,
  },
  session: {
    storageTtlHours: 24,
    useRedis: false,
  },
  experimental: {
    systemPromptHook: true,
    permissionHook: true,
  },
};

// =============================================================================
// Error Types
// =============================================================================

/** Aeterna API error response */
export interface AeternaError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

/** Result type for operations that can fail */
export type Result<T, E = AeternaError> =
  | { ok: true; value: T }
  | { ok: false; error: E };

// =============================================================================
// Significance Detection
// =============================================================================

/** Significance indicators for memory promotion */
export interface SignificanceIndicators {
  isErrorResolution: boolean;
  isRepeatedPattern: boolean;
  isNovelApproach: boolean;
  isExplicitCapture: boolean;
  score: number;
}

// =============================================================================
// CCA (Confucius Code Agent) Types
// =============================================================================

/** View modes for context assembly (Agent/User/Developer experience) */
export type ViewMode = "AX" | "UX" | "DX";

/** Parameters for context assembly */
export interface ContextAssembleParams {
  query: string;
  tokenBudget?: number;
  layers?: MemoryLayer[];
  viewMode?: ViewMode;
  includeKnowledge?: boolean;
}

/** Assembled context result from Context Architect */
export interface AssembledContext {
  context: string;
  tokensUsed: number;
  tokenBudget: number;
  layerBreakdown: Record<MemoryLayer, number>;
  truncated: boolean;
  sources: Array<{
    id: string;
    layer: MemoryLayer;
    relevance: number;
  }>;
}

/** Trajectory event for Note-Taking Agent */
export interface TrajectoryEvent {
  description: string;
  toolName?: string;
  success: boolean;
  tags?: string[];
  timestamp?: number;
  duration?: number;
  metadata?: Record<string, unknown>;
}

/** Parameters for capturing a trajectory event */
export interface NoteCaptureParams {
  description: string;
  toolName?: string;
  success: boolean;
  tags?: string[];
}

/** Generated note from trajectory distillation */
export interface GeneratedNote {
  id: string;
  title: string;
  content: string;
  tags: string[];
  createdAt: string;
  trajectoryCount: number;
}

/** Parameters for hindsight query */
export interface HindsightQueryParams {
  errorType?: string;
  messagePattern?: string;
  contextPatterns?: string[];
  limit?: number;
}

/** Hindsight note with error pattern and resolution */
export interface HindsightNote {
  id: string;
  errorSignature: string;
  errorType: string;
  resolution: string;
  successRate: number;
  occurrences: number;
  lastSeen: string;
  tags: string[];
}

/** Hindsight query result with relevance */
export interface HindsightMatch {
  note: HindsightNote;
  score: number;
  matchedPatterns: string[];
}

/** Meta-agent loop phase */
export type MetaLoopPhase = "build" | "test" | "improve" | "idle" | "completed";

/** Meta-agent loop status */
export interface MetaLoopStatus {
  loopId: string;
  phase: MetaLoopPhase;
  iteration: number;
  maxIterations: number;
  startedAt: string;
  lastPhaseAt?: string;
  qualityScore?: number;
  improvements: string[];
  errors: string[];
}

// =============================================================================
// RLM/Memory-R1 Graph Types
// =============================================================================

/** Graph node representing a memory or knowledge entity */
export interface GraphNode {
  id: string;
  label: string;
  nodeType: "memory" | "knowledge" | "entity";
  properties: Record<string, unknown>;
}

/** Graph edge representing a relationship */
export interface GraphEdge {
  source: string;
  target: string;
  relation: string;
  weight?: number;
  properties?: Record<string, unknown>;
}

/** Parameters for graph query */
export interface GraphQueryParams {
  startNodeId: string;
  relations?: string[];
  depth?: number;
  limit?: number;
  direction?: "outgoing" | "incoming" | "both";
}

/** Graph query result */
export interface GraphQueryResult {
  nodes: GraphNode[];
  edges: GraphEdge[];
  paths?: Array<{
    nodes: string[];
    edges: string[];
    totalWeight: number;
  }>;
}

/** Parameters for finding graph neighbors */
export interface GraphNeighborsParams {
  nodeId: string;
  relations?: string[];
  depth?: number;
  limit?: number;
}

/** Parameters for finding graph path */
export interface GraphPathParams {
  sourceId: string;
  targetId: string;
  maxDepth?: number;
  relations?: string[];
}

/** Graph path result */
export interface GraphPath {
  nodes: GraphNode[];
  edges: GraphEdge[];
  length: number;
  totalWeight: number;
}

/** RLM trajectory step (for reward-based learning) */
export interface RlmTrajectoryStep {
  action: string;
  observation: string;
  reward: number;
  involvedMemoryIds: string[];
  timestamp: number;
}

/** RLM trajectory for decomposition-based search */
export interface RlmTrajectory {
  id: string;
  query: string;
  steps: RlmTrajectoryStep[];
  totalReward: number;
  success: boolean;
  createdAt: string;
}

/** Memory optimization parameters */
export interface MemoryOptimizeParams {
  targetLayer?: MemoryLayer;
  maxPromotions?: number;
  minImportance?: number;
  dryRun?: boolean;
}

/** Memory optimization result */
export interface MemoryOptimizeResult {
  promotedCount: number;
  prunedCount: number;
  compressedCount: number;
  promotions: Array<{
    memoryId: string;
    fromLayer: MemoryLayer;
    toLayer: MemoryLayer;
    reason: string;
  }>;
}
