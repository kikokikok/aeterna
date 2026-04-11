import type { AeternaClient } from "../client.js";
import type { MemoryAddParams, MemoryEntry, MemorySearchParams, MemorySearchResult } from "../types.js";
import type { LocalConfig } from "./config.js";
import type { LocalMemoryManager } from "./manager.js";
export declare class MemoryRouter {
    private readonly localManager;
    private readonly client;
    private readonly config;
    constructor(localManager: LocalMemoryManager, client: AeternaClient, config: LocalConfig);
    search(params: MemorySearchParams): Promise<MemorySearchResult[]>;
    add(params: MemoryAddParams): Promise<MemoryEntry>;
    private withSource;
    private withCacheMetadata;
    private getSyncedAt;
}
//# sourceMappingURL=router.d.ts.map