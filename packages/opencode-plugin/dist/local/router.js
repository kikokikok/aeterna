const PERSONAL_LAYERS = ["agent", "user", "session"];
const SHARED_LAYERS = ["project", "team", "org", "company"];
export class MemoryRouter {
    localManager;
    client;
    config;
    constructor(localManager, client, config) {
        this.localManager = localManager;
        this.client = client;
        this.config = config;
    }
    async search(params) {
        const allLayers = params.layers && params.layers.length > 0
            ? params.layers
            : [
                "agent",
                "user",
                "session",
                "project",
                "team",
                "org",
                "company",
            ];
        const localLayers = allLayers.filter((layer) => PERSONAL_LAYERS.includes(layer));
        const sharedLayers = allLayers.filter((layer) => SHARED_LAYERS.includes(layer));
        const merged = [];
        const limit = Math.min(params.limit ?? 10, this.config.max_cached_entries);
        if (localLayers.length > 0) {
            const localResults = this.localManager.search(params.query, {
                layers: localLayers,
                limit,
                threshold: params.threshold,
                queryEmbedding: params.queryEmbedding,
            });
            merged.push(...localResults.map((result) => this.withSource(result, "local")));
        }
        if (sharedLayers.length > 0) {
            const cachedResults = this.localManager.searchCached(params.query, {
                layers: sharedLayers,
                limit,
                threshold: params.threshold,
                queryEmbedding: params.queryEmbedding,
            });
            const now = Date.now();
            const hasCachedResults = cachedResults.length > 0;
            const cacheFresh = hasCachedResults &&
                cachedResults.every((result) => {
                    const syncedAt = this.getSyncedAt(result);
                    return syncedAt !== null && now - syncedAt < 60_000;
                });
            if (cacheFresh) {
                merged.push(...cachedResults.map((result) => this.withCacheMetadata(result)));
            }
            else {
                try {
                    const remoteResults = await this.client.memorySearchRemote({
                        ...params,
                        layers: sharedLayers,
                    });
                    merged.push(...remoteResults.map((result) => this.withSource(result, "remote")));
                }
                catch {
                    merged.push(...cachedResults.map((result) => this.withCacheMetadata(result)));
                }
            }
        }
        return merged.sort((a, b) => b.score - a.score).slice(0, limit);
    }
    async add(params) {
        const layer = params.layer ?? "session";
        if (PERSONAL_LAYERS.includes(layer)) {
            return this.localManager.add({
                ...params,
                layer,
            });
        }
        return this.client.memoryAddRemote({
            ...params,
            layer,
        });
    }
    withSource(result, source) {
        return {
            ...result,
            memory: {
                ...result.memory,
                metadata: {
                    ...(result.memory.metadata ?? {}),
                    source,
                },
            },
        };
    }
    withCacheMetadata(result) {
        const syncedAt = this.getSyncedAt(result);
        const stale = syncedAt !== null && Date.now() - syncedAt > 10 * 60_000;
        return {
            ...result,
            memory: {
                ...result.memory,
                metadata: {
                    ...(result.memory.metadata ?? {}),
                    source: "cache",
                    ...(stale ? { stale: true } : {}),
                },
            },
        };
    }
    getSyncedAt(result) {
        const syncedAt = result.memory.metadata?.synced_at;
        if (typeof syncedAt === "number" && Number.isFinite(syncedAt)) {
            return syncedAt;
        }
        return null;
    }
}
//# sourceMappingURL=router.js.map