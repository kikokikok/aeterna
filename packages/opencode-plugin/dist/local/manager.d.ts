import type { LocalConfig } from "./config.js";
import type { MemoryAddParams, MemoryEntry, MemoryLayer, MemorySearchResult } from "../types.js";
type SyncOperation = "upsert" | "delete";
export interface LocalMemoryAddParams extends MemoryAddParams {
    embedding?: readonly number[];
}
export interface LocalMemoryUpdateParams {
    content?: string;
    tags?: readonly string[];
    importance?: number;
    metadata?: Record<string, unknown>;
    embedding?: readonly number[];
}
export interface LocalMemorySearchOptions {
    layers?: readonly MemoryLayer[];
    limit?: number;
    threshold?: number;
    queryEmbedding?: readonly number[];
}
export interface CachedMemoryUpsertParams {
    id: string;
    content: string;
    layer: MemoryLayer;
    embedding?: readonly number[];
    tags?: readonly string[];
    metadata?: Record<string, unknown>;
    importance?: number;
    updatedAt: string;
    createdAt?: string;
}
export interface SyncQueueItem {
    queueId: number;
    memoryId: string;
    operation: SyncOperation;
    queuedAt: number;
}
export interface SyncMemorySnapshot {
    id: string;
    content: string;
    layer: MemoryLayer;
    tags: string[];
    metadata?: Record<string, unknown>;
    importance: number;
    createdAt: string;
    updatedAt: string;
    deletedAt?: string;
}
export declare class LocalMemoryManager {
    private readonly localDb;
    private readonly db;
    private readonly config;
    constructor(dbPath: string, config: LocalConfig);
    close(): void;
    add(params: LocalMemoryAddParams): MemoryEntry;
    update(id: string, params: LocalMemoryUpdateParams): MemoryEntry;
    delete(id: string): void;
    getById(id: string): MemoryEntry | null;
    search(query: string, options?: LocalMemorySearchOptions): MemorySearchResult[];
    upsertCached(params: CachedMemoryUpsertParams): MemoryEntry;
    searchCached(query: string, options?: LocalMemorySearchOptions): MemorySearchResult[];
    evictOldCached(): number;
    expireSessionMemories(): number;
    updateEmbedding(id: string, embedding: readonly number[]): void;
    getPendingSyncCount(): number;
    getLastSyncTimestamps(): {
        lastPush: number | null;
        lastPull: number | null;
    };
    getEntryCounts(): Record<string, number>;
    listSyncQueue(limit?: number): SyncQueueItem[];
    getSyncMemorySnapshot(memoryId: string): SyncMemorySnapshot | null;
    removeSyncQueueItems(queueIds: readonly number[]): void;
    getSyncCursor(serverUrl: string, direction: string): string | null;
    setSyncCursor(serverUrl: string, direction: string, cursor: string, updatedAt?: number): void;
    private fetchLocalRowById;
    private getByIdOrThrow;
}
export {};
//# sourceMappingURL=manager.d.ts.map