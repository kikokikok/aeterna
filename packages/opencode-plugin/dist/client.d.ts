import type { AeternaClientConfig, MemoryEntry, MemoryAddParams, MemorySearchParams, MemorySearchResult, MemoryPromoteParams, KnowledgeEntry, KnowledgeQueryParams, KnowledgeSearchResult, KnowledgeProposeParams, SyncStatus, GovernanceStatus, SessionContext, ToolExecutionRecord, ProjectContext, PluginConfig, ContextAssembleParams, AssembledContext, NoteCaptureParams, GeneratedNote, HindsightQueryParams, HindsightMatch, MetaLoopStatus, GraphQueryParams, GraphQueryResult, GraphNeighborsParams, GraphPathParams, GraphPath, MemoryOptimizeParams, MemoryOptimizeResult } from "./types.js";
export declare class AeternaClient {
    private readonly serverUrl;
    private readonly token;
    private readonly config;
    private sessionContext;
    private pluginConfig;
    private knowledgeCache;
    private governanceSubscription;
    private pendingCaptures;
    private captureDebounceTimer;
    constructor(config: AeternaClientConfig);
    private request;
    sessionStart(): Promise<SessionContext>;
    sessionEnd(): Promise<void>;
    memoryAdd(params: MemoryAddParams): Promise<MemoryEntry>;
    memorySearch(params: MemorySearchParams): Promise<MemorySearchResult[]>;
    memoryGet(memoryId: string): Promise<MemoryEntry | null>;
    memoryPromote(params: MemoryPromoteParams): Promise<MemoryEntry>;
    knowledgeQuery(params: KnowledgeQueryParams): Promise<KnowledgeSearchResult[]>;
    knowledgePropose(params: KnowledgeProposeParams): Promise<KnowledgeEntry>;
    getSyncStatus(): Promise<SyncStatus>;
    getGovernanceStatus(): Promise<GovernanceStatus>;
    getProjectContext(): Promise<ProjectContext>;
    queryRelevantKnowledge(message: string, options?: {
        limit?: number;
        threshold?: number;
    }): Promise<KnowledgeSearchResult[]>;
    searchSessionMemories(query: string, options?: {
        limit?: number;
    }): Promise<MemorySearchResult[]>;
    captureToolExecution(record: ToolExecutionRecord): Promise<void>;
    enrichToolArgs(_tool: string, args: Record<string, unknown>): Promise<Record<string, unknown>>;
    checkProposalPermission(): Promise<boolean>;
    flagForPromotion(sessionId: string, callId: string): Promise<void>;
    detectSignificance(input: {
        tool: string;
    }, output: {
        output?: string;
    }): Promise<boolean>;
    setPluginConfig(config: Partial<PluginConfig>): void;
    getSessionContext(): SessionContext | null;
    private prefetchKnowledge;
    private subscribeToGovernance;
    private pollGovernanceEvents;
    private handleGovernanceEvent;
    private flushPendingCaptures;
    private generateSessionSummary;
    contextAssemble(params: ContextAssembleParams): Promise<AssembledContext>;
    noteCapture(params: NoteCaptureParams): Promise<GeneratedNote>;
    hindsightQuery(params: HindsightQueryParams): Promise<HindsightMatch[]>;
    metaLoopStatus(loopId?: string): Promise<MetaLoopStatus>;
    graphQuery(params: GraphQueryParams): Promise<GraphQueryResult>;
    graphNeighbors(params: GraphNeighborsParams): Promise<GraphQueryResult>;
    graphPath(params: GraphPathParams): Promise<GraphPath>;
    memoryOptimize(params?: MemoryOptimizeParams): Promise<MemoryOptimizeResult>;
}
//# sourceMappingURL=client.d.ts.map