import { LocalDatabase } from "./db.js";
const LOCAL_LAYERS = ["agent", "user", "session"];
const SHARED_LAYERS = ["project", "team", "org", "company"];
export class LocalMemoryManager {
    localDb;
    db;
    config;
    constructor(dbPath, config) {
        this.localDb = new LocalDatabase(dbPath);
        this.db = this.localDb.connection;
        this.config = config;
    }
    close() {
        this.localDb.close();
    }
    add(params) {
        const now = Date.now();
        const id = crypto.randomUUID();
        const layer = params.layer ?? "session";
        if (!LOCAL_LAYERS.includes(layer)) {
            throw new Error(`Layer ${layer} is not local-owned`);
        }
        const tagsJson = JSON.stringify(params.tags ?? []);
        const metadataJson = params.metadata ? JSON.stringify(params.metadata) : null;
        const embeddingBuffer = params.embedding ? encodeEmbedding(params.embedding) : null;
        const importance = params.importance ?? 0;
        const write = this.db.transaction(() => {
            this.db
                .prepare(`
          INSERT INTO memories (
            id, content, layer, ownership, embedding, tags, metadata,
            importance_score, tenant_context, device_id, created_at, updated_at, synced_at, deleted_at
          ) VALUES (
            @id, @content, @layer, 'local', @embedding, @tags, @metadata,
            @importance_score, NULL, NULL, @created_at, @updated_at, NULL, NULL
          )
          `)
                .run({
                id,
                content: params.content,
                layer,
                embedding: embeddingBuffer,
                tags: tagsJson,
                metadata: metadataJson,
                importance_score: importance,
                created_at: now,
                updated_at: now,
            });
            enqueueSync(this.db, id, "upsert", now);
        });
        write();
        return this.getByIdOrThrow(id);
    }
    update(id, params) {
        const existing = this.fetchLocalRowById(id);
        if (!existing) {
            throw new Error(`Memory not found: ${id}`);
        }
        const now = Date.now();
        const nextTags = params.tags ? JSON.stringify([...params.tags]) : existing.tags;
        const nextMetadata = params.metadata !== undefined ? JSON.stringify(params.metadata) : existing.metadata;
        const nextEmbedding = params.embedding !== undefined ? encodeEmbedding(params.embedding) : existing.embedding;
        const nextContent = params.content ?? existing.content;
        const nextImportance = params.importance ?? existing.importance_score;
        const write = this.db.transaction(() => {
            this.db
                .prepare(`
          UPDATE memories
          SET content = @content,
              tags = @tags,
              metadata = @metadata,
              embedding = @embedding,
              importance_score = @importance_score,
              updated_at = @updated_at
          WHERE id = @id
            AND ownership = 'local'
            AND deleted_at IS NULL
          `)
                .run({
                id,
                content: nextContent,
                tags: nextTags,
                metadata: nextMetadata,
                embedding: nextEmbedding,
                importance_score: nextImportance,
                updated_at: now,
            });
            enqueueSync(this.db, id, "upsert", now);
        });
        write();
        return this.getByIdOrThrow(id);
    }
    delete(id) {
        const row = this.fetchLocalRowById(id);
        if (!row) {
            return;
        }
        const now = Date.now();
        const write = this.db.transaction(() => {
            this.db
                .prepare(`
          UPDATE memories
          SET deleted_at = @deleted_at,
              updated_at = @updated_at
          WHERE id = @id
            AND ownership = 'local'
            AND deleted_at IS NULL
          `)
                .run({ id, deleted_at: now, updated_at: now });
            enqueueSync(this.db, id, "delete", now);
        });
        write();
    }
    getById(id) {
        const row = this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE id = @id
          AND ownership = 'local'
          AND deleted_at IS NULL
        LIMIT 1
        `)
            .get({ id });
        if (!row) {
            return null;
        }
        return toMemoryEntry(row);
    }
    search(query, options = {}) {
        const limit = options.limit ?? 10;
        const threshold = options.threshold ?? 0;
        const layers = filterLocalLayers(options.layers);
        const layerSet = new Set(layers);
        const readLimit = Math.max(limit * 5, limit);
        if (!options.queryEmbedding) {
            const likeRows = this.db
                .prepare(`
          SELECT id, content, layer, ownership, embedding, tags, metadata,
                 importance_score, created_at, updated_at, synced_at, deleted_at
          FROM memories
          WHERE ownership = 'local'
            AND deleted_at IS NULL
            AND layer IN ('agent', 'user', 'session')
            AND content LIKE @pattern
          ORDER BY updated_at DESC
          LIMIT @limit
          `)
                .all({
                pattern: `%${query}%`,
                limit: readLimit,
            })
                .filter((row) => layerSet.has(row.layer));
            return likeRows
                .map((row) => ({
                memory: toMemoryEntry(row),
                score: 1,
            }))
                .slice(0, limit);
        }
        const rows = this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE ownership = 'local'
          AND deleted_at IS NULL
          AND layer IN ('agent', 'user', 'session')
        ORDER BY updated_at DESC
        LIMIT @limit
        `)
            .all({ limit: readLimit })
            .filter((row) => layerSet.has(row.layer));
        if (rows.length === 0) {
            return [];
        }
        const cosineResults = [];
        let hasEmbedding = false;
        for (const row of rows) {
            if (!row.embedding) {
                continue;
            }
            const vector = decodeEmbedding(row.embedding);
            if (vector.length === 0) {
                continue;
            }
            hasEmbedding = true;
            const score = cosineSimilarity(options.queryEmbedding, vector);
            if (score >= threshold) {
                cosineResults.push({
                    memory: toMemoryEntry(row),
                    score,
                });
            }
        }
        if (!hasEmbedding) {
            return [];
        }
        return cosineResults.sort((a, b) => b.score - a.score).slice(0, limit);
    }
    upsertCached(params) {
        const now = Date.now();
        const updatedAt = Date.parse(params.updatedAt);
        const createdAt = params.createdAt ? Date.parse(params.createdAt) : updatedAt;
        const tagsJson = JSON.stringify(params.tags ?? []);
        const metadataJson = params.metadata ? JSON.stringify(params.metadata) : null;
        const embeddingBuffer = params.embedding ? encodeEmbedding(params.embedding) : null;
        this.db
            .prepare(`
        INSERT OR REPLACE INTO memories (
          id, content, layer, ownership, embedding, tags, metadata,
          importance_score, tenant_context, device_id, created_at, updated_at, synced_at, deleted_at
        ) VALUES (
          @id, @content, @layer, 'cached', @embedding, @tags, @metadata,
          @importance_score, NULL, NULL, @created_at, @updated_at, @synced_at, NULL
        )
        `)
            .run({
            id: params.id,
            content: params.content,
            layer: params.layer,
            embedding: embeddingBuffer,
            tags: tagsJson,
            metadata: metadataJson,
            importance_score: params.importance ?? 0,
            created_at: Number.isFinite(createdAt) ? createdAt : now,
            updated_at: Number.isFinite(updatedAt) ? updatedAt : now,
            synced_at: now,
        });
        const row = this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE id = @id
          AND ownership = 'cached'
          AND deleted_at IS NULL
        LIMIT 1
        `)
            .get({ id: params.id });
        if (!row) {
            throw new Error(`Cached memory not found after upsert: ${params.id}`);
        }
        return toMemoryEntry(row);
    }
    searchCached(query, options = {}) {
        const limit = options.limit ?? 10;
        const threshold = options.threshold ?? 0;
        const layers = filterSharedLayers(options.layers);
        const layerSet = new Set(layers);
        const readLimit = Math.max(limit * 5, limit);
        if (!options.queryEmbedding) {
            const likeRows = this.db
                .prepare(`
          SELECT id, content, layer, ownership, embedding, tags, metadata,
                 importance_score, created_at, updated_at, synced_at, deleted_at
          FROM memories
          WHERE ownership = 'cached'
            AND deleted_at IS NULL
            AND layer IN ('project', 'team', 'org', 'company')
            AND content LIKE @pattern
          ORDER BY updated_at DESC
          LIMIT @limit
          `)
                .all({
                pattern: `%${query}%`,
                limit: readLimit,
            })
                .filter((row) => layerSet.has(row.layer));
            return likeRows
                .map((row) => ({
                memory: toMemoryEntry(row, {
                    synced_at: row.synced_at,
                }),
                score: 1,
            }))
                .slice(0, limit);
        }
        const rows = this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE ownership = 'cached'
          AND deleted_at IS NULL
          AND layer IN ('project', 'team', 'org', 'company')
        ORDER BY updated_at DESC
        LIMIT @limit
        `)
            .all({ limit: readLimit })
            .filter((row) => layerSet.has(row.layer));
        if (rows.length === 0) {
            return [];
        }
        const cosineResults = [];
        let hasEmbedding = false;
        for (const row of rows) {
            if (!row.embedding) {
                continue;
            }
            const vector = decodeEmbedding(row.embedding);
            if (vector.length === 0) {
                continue;
            }
            hasEmbedding = true;
            const score = cosineSimilarity(options.queryEmbedding, vector);
            if (score >= threshold) {
                cosineResults.push({
                    memory: toMemoryEntry(row, {
                        synced_at: row.synced_at,
                    }),
                    score,
                });
            }
        }
        if (!hasEmbedding) {
            return [];
        }
        return cosineResults.sort((a, b) => b.score - a.score).slice(0, limit);
    }
    evictOldCached() {
        const maxEntries = this.config.max_cached_entries;
        const currentCached = this.db
            .prepare(`
        SELECT COUNT(*) AS count
        FROM memories
        WHERE ownership = 'cached'
        `)
            .get()?.count ?? 0;
        if (currentCached <= maxEntries) {
            return 0;
        }
        const toDelete = currentCached - maxEntries;
        const result = this.db
            .prepare(`
        DELETE FROM memories
        WHERE id IN (
          SELECT id
          FROM memories
          WHERE ownership = 'cached'
          ORDER BY COALESCE(synced_at, 0) ASC
          LIMIT @limit
        )
        `)
            .run({ limit: toDelete });
        return result.changes;
    }
    expireSessionMemories() {
        const cutoff = Date.now() - this.config.session_storage_ttl_hours * 3600 * 1000;
        const expiredRows = this.db
            .prepare(`
        SELECT id
        FROM memories
        WHERE ownership = 'local'
          AND layer = 'session'
          AND created_at < @cutoff
        `)
            .all({ cutoff });
        if (expiredRows.length === 0) {
            return 0;
        }
        const expire = this.db.transaction((ids) => {
            for (const id of ids) {
                this.db.prepare("DELETE FROM sync_queue WHERE memory_id = ?").run(id);
                this.db.prepare("DELETE FROM memories WHERE id = ?").run(id);
            }
        });
        expire(expiredRows.map((row) => row.id));
        return expiredRows.length;
    }
    updateEmbedding(id, embedding) {
        this.db
            .prepare(`
        UPDATE memories
        SET embedding = @embedding,
            updated_at = @updated_at
        WHERE id = @id
          AND deleted_at IS NULL
        `)
            .run({
            id,
            embedding: encodeEmbedding(embedding),
            updated_at: Date.now(),
        });
    }
    getPendingSyncCount() {
        return (this.db
            .prepare(`
          SELECT COUNT(*) AS count
          FROM sync_queue
          `)
            .get()?.count ?? 0);
    }
    getLastSyncTimestamps() {
        const push = this.db
            .prepare(`
        SELECT MAX(updated_at) AS updated_at
        FROM sync_cursors
        WHERE direction = 'push'
          AND server_url <> '_device'
        `)
            .get();
        const pull = this.db
            .prepare(`
        SELECT MAX(updated_at) AS updated_at
        FROM sync_cursors
        WHERE direction = 'pull'
          AND server_url <> '_device'
        `)
            .get();
        return {
            lastPush: push?.updated_at ?? null,
            lastPull: pull?.updated_at ?? null,
        };
    }
    getEntryCounts() {
        const byLayer = this.db
            .prepare(`
        SELECT 'layer:' || layer AS key, COUNT(*) AS count
        FROM memories
        GROUP BY layer
        `)
            .all();
        const byOwnership = this.db
            .prepare(`
        SELECT 'ownership:' || ownership AS key, COUNT(*) AS count
        FROM memories
        GROUP BY ownership
        `)
            .all();
        const total = this.db
            .prepare(`
        SELECT COUNT(*) AS count
        FROM memories
        `)
            .get()?.count ?? 0;
        const counts = { total };
        for (const row of [...byLayer, ...byOwnership]) {
            counts[row.key] = row.count;
        }
        return counts;
    }
    listSyncQueue(limit = 100) {
        const rows = this.db
            .prepare(`
        SELECT id, memory_id, operation, queued_at
        FROM sync_queue
        ORDER BY queued_at ASC
        LIMIT @limit
        `)
            .all({ limit });
        return rows.map((row) => ({
            queueId: row.id,
            memoryId: row.memory_id,
            operation: row.operation,
            queuedAt: row.queued_at,
        }));
    }
    getSyncMemorySnapshot(memoryId) {
        const row = this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE id = @id
        LIMIT 1
        `)
            .get({ id: memoryId });
        if (!row) {
            return null;
        }
        return {
            id: row.id,
            content: row.content,
            layer: row.layer,
            tags: parseJson(row.tags, []),
            metadata: parseJson(row.metadata, undefined),
            importance: row.importance_score,
            createdAt: new Date(row.created_at).toISOString(),
            updatedAt: new Date(row.updated_at).toISOString(),
            deletedAt: row.deleted_at !== null ? new Date(row.deleted_at).toISOString() : undefined,
        };
    }
    removeSyncQueueItems(queueIds) {
        if (queueIds.length === 0) {
            return;
        }
        const remove = this.db.transaction((ids) => {
            const statement = this.db.prepare("DELETE FROM sync_queue WHERE id = ?");
            for (const id of ids) {
                statement.run(id);
            }
        });
        remove(queueIds);
    }
    getSyncCursor(serverUrl, direction) {
        const row = this.db
            .prepare(`
        SELECT cursor
        FROM sync_cursors
        WHERE server_url = @server_url
          AND direction = @direction
        LIMIT 1
        `)
            .get({ server_url: serverUrl, direction });
        return row?.cursor ?? null;
    }
    setSyncCursor(serverUrl, direction, cursor, updatedAt = Date.now()) {
        this.db
            .prepare(`
        INSERT INTO sync_cursors (server_url, direction, cursor, updated_at)
        VALUES (@server_url, @direction, @cursor, @updated_at)
        ON CONFLICT(server_url, direction)
        DO UPDATE SET
          cursor = excluded.cursor,
          updated_at = excluded.updated_at
        `)
            .run({
            server_url: serverUrl,
            direction,
            cursor,
            updated_at: updatedAt,
        });
    }
    fetchLocalRowById(id) {
        return this.db
            .prepare(`
        SELECT id, content, layer, ownership, embedding, tags, metadata,
               importance_score, created_at, updated_at, synced_at, deleted_at
        FROM memories
        WHERE id = @id
          AND ownership = 'local'
          AND deleted_at IS NULL
        LIMIT 1
        `)
            .get({ id });
    }
    getByIdOrThrow(id) {
        const entry = this.getById(id);
        if (!entry) {
            throw new Error(`Memory not found after write: ${id}`);
        }
        return entry;
    }
}
const filterLocalLayers = (layers) => {
    if (!layers || layers.length === 0) {
        return LOCAL_LAYERS;
    }
    return layers.filter((layer) => LOCAL_LAYERS.includes(layer));
};
const filterSharedLayers = (layers) => {
    if (!layers || layers.length === 0) {
        return SHARED_LAYERS;
    }
    return layers.filter((layer) => SHARED_LAYERS.includes(layer));
};
const enqueueSync = (db, memoryId, operation, queuedAt) => {
    db.prepare(`
    INSERT INTO sync_queue (memory_id, operation, queued_at)
    VALUES (@memory_id, @operation, @queued_at)
    `).run({
        memory_id: memoryId,
        operation,
        queued_at: queuedAt,
    });
};
const toMemoryEntry = (row, metadataExtras) => {
    const parsedTags = parseJson(row.tags, []);
    const parsedMetadata = parseJson(row.metadata, {});
    return {
        id: row.id,
        content: row.content,
        layer: row.layer,
        importance: row.importance_score,
        tags: parsedTags,
        createdAt: new Date(row.created_at).toISOString(),
        updatedAt: new Date(row.updated_at).toISOString(),
        metadata: Object.keys({ ...parsedMetadata, ...(metadataExtras ?? {}) }).length > 0
            ? { ...parsedMetadata, ...(metadataExtras ?? {}) }
            : undefined,
    };
};
const encodeEmbedding = (embedding) => {
    const array = new Float32Array(embedding.length);
    array.set(embedding);
    return Buffer.from(array.buffer.slice(0));
};
const decodeEmbedding = (buffer) => {
    const offset = buffer.byteOffset;
    const length = Math.floor(buffer.byteLength / Float32Array.BYTES_PER_ELEMENT);
    return new Float32Array(buffer.buffer.slice(offset, offset + length * Float32Array.BYTES_PER_ELEMENT));
};
const cosineSimilarity = (left, right) => {
    if (left.length === 0 || right.length === 0 || left.length !== right.length) {
        return 0;
    }
    let dot = 0;
    let leftNorm = 0;
    let rightNorm = 0;
    for (let i = 0; i < left.length; i += 1) {
        const l = left[i];
        const r = right[i] ?? 0;
        dot += l * r;
        leftNorm += l * l;
        rightNorm += r * r;
    }
    if (leftNorm === 0 || rightNorm === 0) {
        return 0;
    }
    return dot / (Math.sqrt(leftNorm) * Math.sqrt(rightNorm));
};
const parseJson = (value, fallback) => {
    if (!value) {
        return fallback;
    }
    try {
        return JSON.parse(value);
    }
    catch {
        return fallback;
    }
};
//# sourceMappingURL=manager.js.map