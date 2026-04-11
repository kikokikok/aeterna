import type { PluginAuthTokens, DeviceCodeResponse, AeternaClientConfig, MemoryEntry, MemoryAddParams, MemorySearchParams, MemorySearchResult, MemoryPromoteParams, KnowledgeEntry, KnowledgeQueryParams, KnowledgeSearchResult, KnowledgeProposeParams, SyncStatus, GovernanceStatus, SessionContext, ToolExecutionRecord, ProjectContext, PluginConfig, ContextAssembleParams, AssembledContext, NoteCaptureParams, GeneratedNote, HindsightQueryParams, HindsightMatch, MetaLoopStatus, GraphQueryParams, GraphQueryResult, GraphNeighborsParams, GraphPathParams, GraphPath, MemoryOptimizeParams, MemoryOptimizeResult, SyncPullParams, SyncPullResponse, SyncPushPayload, SyncPushResponse } from "./types.js";
import type { MemoryRouter } from "./local/router.js";
import type { LocalMemoryManager } from "./local/manager.js";
import type { SyncEngine } from "./local/sync.js";
export declare class AeternaClient {
    private readonly serverUrl;
    private accessToken;
    private refreshTokenValue;
    private readonly config;
    private sessionContext;
    private pluginConfig;
    private knowledgeCache;
    private governanceSubscription;
    private pendingCaptures;
    private captureDebounceTimer;
    private router;
    private localManager;
    private syncEngine;
    constructor(config: AeternaClientConfig);
    private request;
    sessionStart(): Promise<SessionContext>;
    sessionEnd(): Promise<void>;
    memoryAdd(params: MemoryAddParams): Promise<MemoryEntry>;
    memoryAddRemote(params: MemoryAddParams): Promise<MemoryEntry>;
    memorySearch(params: MemorySearchParams): Promise<MemorySearchResult[]>;
    memorySearchRemote(params: MemorySearchParams): Promise<MemorySearchResult[]>;
    syncPush(payload: SyncPushPayload, options?: {
        signal?: AbortSignal;
    }): Promise<SyncPushResponse>;
    syncPull(params: SyncPullParams, options?: {
        signal?: AbortSignal;
    }): Promise<SyncPullResponse>;
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
    setRouter(router: MemoryRouter): void;
    setLocalManager(manager: LocalMemoryManager): void;
    setSyncEngine(engine: SyncEngine): void;
    getServerUrl(): string;
    getLocalSyncStatus(): {
        pendingPushCount: number;
        lastPush: number | null;
        lastPull: number | null;
        entryCounts: Record<string, number>;
        serverConnectivity: boolean;
    } | null;
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
    /** Current access token – useful for inspecting auth state in tests. */
    getAccessToken(): string;
    /** Whether the client currently holds a dynamic refresh token. */
    hasRefreshToken(): boolean;
    /**
     * Initiate the GitHub OAuth device flow.
     *
     * Calls `POST https://github.com/login/device/code` and returns the
     * device code payload including the `user_code` and `verification_uri`
     * that must be shown to the user.
     */
    requestDeviceCode(clientId: string, scope?: string): Promise<DeviceCodeResponse>;
    /**
     * Poll GitHub for an OAuth access token using the device code.
     *
     * Polls `POST https://github.com/login/oauth/access_token` at the
     * interval specified in the device-code response until the user
     * completes authorisation, the code expires, or an unrecoverable error
     * occurs.
     *
     * @returns The GitHub OAuth access token string.
     */
    pollDeviceToken(clientId: string, deviceCode: string, interval: number, expiresIn: number, signal?: AbortSignal): Promise<string>;
    /**
     * Bootstrap Aeterna plugin credentials using a GitHub OAuth access token
     * obtained via the device flow.
     *
     * On success the client's internal access token and refresh token are
     * updated so that subsequent `request()` calls carry the new bearer token.
     */
    bootstrapAuth(githubAccessToken: string): Promise<PluginAuthTokens>;
    /**
     * Use the stored refresh token to obtain a new access token.
     *
     * Implements single-use refresh token rotation: the server consumes the
     * current refresh token and issues a new pair.
     *
     * @throws {Error} When no refresh token is stored or the server rejects it.
     */
    refreshAuth(): Promise<PluginAuthTokens>;
    /**
     * Revoke the current refresh token on the server and clear local auth state.
     *
     * Safe to call even when no refresh token is held (no-op in that case).
     */
    logoutAuth(): Promise<void>;
    /**
     * Inject a token pair obtained externally (e.g. from a persisted credential
     * store or a test stub).
     */
    setAuthTokens(accessToken: string, refreshToken: string): void;
}
//# sourceMappingURL=client.d.ts.map