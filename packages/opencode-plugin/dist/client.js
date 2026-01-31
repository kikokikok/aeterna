export class AeternaClient {
    serverUrl;
    token;
    config;
    sessionContext = null;
    pluginConfig = DEFAULT_CONFIG;
    knowledgeCache = new Map();
    governanceSubscription = null;
    pendingCaptures = new Map();
    captureDebounceTimer = null;
    constructor(config) {
        this.config = config;
        this.serverUrl =
            config.serverUrl ?? process.env.AETERNA_SERVER_URL ?? "http://localhost:8080";
        this.token = config.token ?? process.env.AETERNA_TOKEN ?? "";
    }
    async request(method, path, body) {
        try {
            const response = await fetch(`${this.serverUrl}${path}`, {
                method,
                headers: {
                    "Content-Type": "application/json",
                    Authorization: `Bearer ${this.token}`,
                    "X-Aeterna-Project": this.config.project,
                    ...(this.config.team && { "X-Aeterna-Team": this.config.team }),
                    ...(this.config.org && { "X-Aeterna-Org": this.config.org }),
                    ...(this.sessionContext && {
                        "X-Aeterna-Session": this.sessionContext.sessionId,
                    }),
                },
                body: body ? JSON.stringify(body) : undefined,
            });
            if (!response.ok) {
                const error = await response.json().catch(() => ({
                    code: "HTTP_ERROR",
                    message: `HTTP ${response.status}: ${response.statusText}`,
                }));
                return { ok: false, error };
            }
            const data = (await response.json());
            return { ok: true, value: data };
        }
        catch (err) {
            return {
                ok: false,
                error: {
                    code: "NETWORK_ERROR",
                    message: err instanceof Error ? err.message : "Network error",
                },
            };
        }
    }
    async sessionStart() {
        const result = await this.request("POST", "/api/v1/sessions", {
            project: this.config.project,
            directory: this.config.directory,
            team: this.config.team,
            org: this.config.org,
            userId: this.config.userId,
        });
        if (result.ok) {
            this.sessionContext = result.value;
            await this.prefetchKnowledge();
            this.subscribeToGovernance();
            return result.value;
        }
        this.sessionContext = {
            sessionId: crypto.randomUUID(),
            project: this.config.project,
            team: this.config.team,
            org: this.config.org,
            userId: this.config.userId,
            startedAt: new Date().toISOString(),
        };
        return this.sessionContext;
    }
    async sessionEnd() {
        if (!this.sessionContext)
            return;
        await this.flushPendingCaptures();
        if (this.governanceSubscription) {
            this.governanceSubscription.abort();
            this.governanceSubscription = null;
        }
        await this.request("POST", `/api/v1/sessions/${this.sessionContext.sessionId}/end`, {
            summary: await this.generateSessionSummary(),
        });
        this.sessionContext = null;
        this.knowledgeCache.clear();
    }
    async memoryAdd(params) {
        const result = await this.request("POST", "/api/v1/memories", {
            ...params,
            sessionId: params.sessionId ?? this.sessionContext?.sessionId,
        });
        if (!result.ok) {
            throw new Error(`Failed to add memory: ${result.error.message}`);
        }
        return result.value;
    }
    async memorySearch(params) {
        const result = await this.request("POST", "/api/v1/memories/search", {
            ...params,
            sessionId: params.sessionId ?? this.sessionContext?.sessionId,
        });
        if (!result.ok) {
            throw new Error(`Failed to search memories: ${result.error.message}`);
        }
        return result.value;
    }
    async memoryGet(memoryId) {
        const result = await this.request("GET", `/api/v1/memories/${memoryId}`);
        if (!result.ok)
            return null;
        return result.value;
    }
    async memoryPromote(params) {
        const result = await this.request("POST", `/api/v1/memories/${params.memoryId}/promote`, {
            targetLayer: params.targetLayer,
            reason: params.reason,
        });
        if (!result.ok) {
            throw new Error(`Failed to promote memory: ${result.error.message}`);
        }
        return result.value;
    }
    async knowledgeQuery(params) {
        const cacheKey = JSON.stringify(params);
        const cached = this.knowledgeCache.get(cacheKey);
        const ttl = (this.pluginConfig.knowledge.cacheTtlSeconds ?? 60) * 1000;
        if (cached && Date.now() - cached.timestamp < ttl) {
            return cached.results;
        }
        const timeout = this.pluginConfig.knowledge.timeoutMs ?? 200;
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), timeout);
        try {
            const response = await fetch(`${this.serverUrl}/api/v1/knowledge/search`, {
                method: "POST",
                headers: {
                    "Content-Type": "application/json",
                    Authorization: `Bearer ${this.token}`,
                    "X-Aeterna-Project": this.config.project,
                },
                body: JSON.stringify(params),
                signal: controller.signal,
            });
            clearTimeout(timeoutId);
            if (!response.ok) {
                if (cached)
                    return cached.results;
                return [];
            }
            const results = (await response.json());
            this.knowledgeCache.set(cacheKey, {
                results,
                timestamp: Date.now(),
                query: params.query,
            });
            return results;
        }
        catch {
            if (cached)
                return cached.results;
            return [];
        }
    }
    async knowledgePropose(params) {
        const result = await this.request("POST", "/api/v1/knowledge/proposals", {
            ...params,
            proposer: this.config.userId,
        });
        if (!result.ok) {
            throw new Error(`Failed to propose knowledge: ${result.error.message}`);
        }
        return result.value;
    }
    async getSyncStatus() {
        const result = await this.request("GET", "/api/v1/sync/status");
        if (!result.ok) {
            return {
                lastSync: new Date().toISOString(),
                pendingPromotions: 0,
                pendingProposals: 0,
                syncHealth: "error",
                errors: [result.error.message],
            };
        }
        return result.value;
    }
    async getGovernanceStatus() {
        const result = await this.request("GET", "/api/v1/governance/status");
        if (!result.ok) {
            return {
                activePolicies: 0,
                pendingProposals: 0,
                recentViolations: 0,
                driftDetected: false,
                notifications: [],
            };
        }
        return result.value;
    }
    async getProjectContext() {
        const result = await this.request("GET", "/api/v1/context/project");
        if (!result.ok) {
            return {
                project: { name: this.config.project, path: this.config.directory },
                policies: [],
                recentMemories: [],
            };
        }
        return result.value;
    }
    async queryRelevantKnowledge(message, options) {
        if (!this.pluginConfig.knowledge.injectionEnabled) {
            return [];
        }
        return this.knowledgeQuery({
            query: message,
            limit: options?.limit ?? this.pluginConfig.knowledge.maxItems,
            threshold: options?.threshold ?? this.pluginConfig.knowledge.threshold,
        });
    }
    async searchSessionMemories(query, options) {
        return this.memorySearch({
            query,
            layers: ["session", "working"],
            limit: options?.limit ?? 5,
            sessionId: this.sessionContext?.sessionId,
        });
    }
    async captureToolExecution(record) {
        if (!this.pluginConfig.capture.enabled)
            return;
        this.pendingCaptures.set(record.callId, record);
        if (this.captureDebounceTimer) {
            clearTimeout(this.captureDebounceTimer);
        }
        const debounceMs = this.pluginConfig.capture.debounceMs ?? 500;
        this.captureDebounceTimer = setTimeout(() => {
            this.flushPendingCaptures();
        }, debounceMs);
    }
    async enrichToolArgs(_tool, args) {
        if (this.sessionContext) {
            args.sessionId = this.sessionContext.sessionId;
        }
        return args;
    }
    async checkProposalPermission() {
        const result = await this.request("GET", "/api/v1/governance/permissions/propose");
        return result.ok && result.value.allowed;
    }
    async flagForPromotion(sessionId, callId) {
        await this.request("POST", "/api/v1/memories/flag-promotion", {
            sessionId,
            callId,
        });
    }
    async detectSignificance(input, output) {
        const toolPatterns = [
            "aeterna_memory_add",
            "aeterna_knowledge_propose",
        ];
        if (toolPatterns.includes(input.tool)) {
            return true;
        }
        if (output.output && output.output.length > 500) {
            return true;
        }
        return false;
    }
    setPluginConfig(config) {
        this.pluginConfig = { ...this.pluginConfig, ...config };
    }
    getSessionContext() {
        return this.sessionContext;
    }
    async prefetchKnowledge() {
        try {
            await this.knowledgeQuery({
                query: `project:${this.config.project}`,
                scope: "project",
                limit: 10,
            });
        }
        catch {
            // Prefetch failures are non-critical
        }
    }
    subscribeToGovernance() {
        if (!this.pluginConfig.governance.notifications)
            return;
        this.governanceSubscription = new AbortController();
        this.pollGovernanceEvents(this.governanceSubscription.signal);
    }
    async pollGovernanceEvents(signal) {
        while (!signal.aborted) {
            try {
                const result = await this.request("GET", "/api/v1/governance/events");
                if (result.ok && result.value.length > 0) {
                    for (const event of result.value) {
                        this.handleGovernanceEvent(event);
                    }
                }
                await new Promise((resolve) => setTimeout(resolve, 30000));
            }
            catch {
                if (!signal.aborted) {
                    await new Promise((resolve) => setTimeout(resolve, 60000));
                }
            }
        }
    }
    handleGovernanceEvent(_event) {
        // Events are surfaced through the governance status endpoint
    }
    async flushPendingCaptures() {
        if (this.pendingCaptures.size === 0)
            return;
        const captures = Array.from(this.pendingCaptures.values());
        this.pendingCaptures.clear();
        await this.request("POST", "/api/v1/captures/batch", { captures });
    }
    async generateSessionSummary() {
        const captures = Array.from(this.pendingCaptures.values());
        const successCount = captures.filter((c) => c.success).length;
        const toolsUsed = [...new Set(captures.map((c) => c.tool))];
        return `Session completed with ${captures.length} tool executions (${successCount} successful). Tools used: ${toolsUsed.join(", ") || "none"}`;
    }
    async contextAssemble(params) {
        const result = await this.request("POST", "/api/v1/cca/context-assemble", {
            query: params.query,
            tokenBudget: params.tokenBudget ?? 8000,
            layers: params.layers ?? ["project", "team"],
            viewMode: params.viewMode ?? "AX",
            includeKnowledge: params.includeKnowledge ?? true,
        });
        if (!result.ok) {
            return {
                context: "",
                tokensUsed: 0,
                tokenBudget: params.tokenBudget ?? 8000,
                layerBreakdown: {},
                truncated: false,
                sources: [],
            };
        }
        return result.value;
    }
    async noteCapture(params) {
        const result = await this.request("POST", "/api/v1/cca/note-capture", {
            description: params.description,
            toolName: params.toolName,
            success: params.success,
            tags: params.tags,
        });
        if (!result.ok) {
            throw new Error(`Failed to capture note: ${result.error.message}`);
        }
        return result.value;
    }
    async hindsightQuery(params) {
        const result = await this.request("POST", "/api/v1/cca/hindsight-query", {
            errorType: params.errorType,
            messagePattern: params.messagePattern,
            contextPatterns: params.contextPatterns,
            limit: params.limit ?? 10,
        });
        if (!result.ok) {
            throw new Error(`Failed to query hindsight: ${result.error.message}`);
        }
        return result.value;
    }
    async metaLoopStatus(loopId) {
        const path = loopId ? `/api/v1/cca/meta-loop-status?loopId=${loopId}` : "/api/v1/cca/meta-loop-status";
        const result = await this.request("GET", path);
        if (!result.ok) {
            return {
                loopId: loopId ?? "",
                phase: "idle",
                iteration: 0,
                maxIterations: 0,
                startedAt: new Date().toISOString(),
                qualityScore: 0,
                improvements: [],
                errors: [],
            };
        }
        return result.value;
    }
    async graphQuery(params) {
        const result = await this.request("POST", "/api/v1/graph/query", {
            startNodeId: params.startNodeId,
            relations: params.relations,
            depth: params.depth ?? 2,
            limit: params.limit ?? 50,
            direction: params.direction ?? "outgoing",
        });
        if (!result.ok) {
            throw new Error(`Failed to query graph: ${result.error.message}`);
        }
        return result.value;
    }
    async graphNeighbors(params) {
        const result = await this.request("POST", "/api/v1/graph/neighbors", {
            nodeId: params.nodeId,
            relations: params.relations,
            depth: params.depth ?? 2,
            limit: params.limit ?? 20,
        });
        if (!result.ok) {
            throw new Error(`Failed to query graph neighbors: ${result.error.message}`);
        }
        return result.value;
    }
    async graphPath(params) {
        const result = await this.request("POST", "/api/v1/graph/path", {
            sourceId: params.sourceId,
            targetId: params.targetId,
            maxDepth: params.maxDepth ?? 5,
            relations: params.relations,
        });
        if (!result.ok) {
            throw new Error(`Failed to find graph path: ${result.error.message}`);
        }
        return result.value;
    }
    async memoryOptimize(params) {
        const result = await this.request("POST", "/api/v1/memory/optimize", {
            targetLayer: params?.targetLayer,
            maxPromotions: params?.maxPromotions,
            minImportance: params?.minImportance,
            dryRun: params?.dryRun ?? false,
        });
        if (!result.ok) {
            throw new Error(`Failed to optimize memory: ${result.error.message}`);
        }
        return result.value;
    }
}
//# sourceMappingURL=client.js.map