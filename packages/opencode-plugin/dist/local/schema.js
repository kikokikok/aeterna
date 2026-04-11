export const SCHEMA_VERSION = 1;
export const CREATE_MEMORIES_TABLE_SQL = `
CREATE TABLE IF NOT EXISTS memories (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  layer TEXT NOT NULL,
  ownership TEXT NOT NULL,
  embedding BLOB,
  tags TEXT,
  metadata TEXT,
  importance_score REAL NOT NULL DEFAULT 0.0,
  tenant_context TEXT,
  device_id TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  synced_at INTEGER,
  deleted_at INTEGER
);
`;
export const CREATE_SYNC_QUEUE_TABLE_SQL = `
CREATE TABLE IF NOT EXISTS sync_queue (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  memory_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  queued_at INTEGER NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);
`;
export const CREATE_SYNC_CURSORS_TABLE_SQL = `
CREATE TABLE IF NOT EXISTS sync_cursors (
  server_url TEXT NOT NULL,
  direction TEXT NOT NULL,
  cursor TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (server_url, direction)
);
`;
export const CREATE_INDEXES_SQL = `
CREATE INDEX IF NOT EXISTS idx_memories_layer ON memories(layer);
CREATE INDEX IF NOT EXISTS idx_memories_ownership ON memories(ownership);
CREATE INDEX IF NOT EXISTS idx_memories_updated ON memories(updated_at);
CREATE INDEX IF NOT EXISTS idx_sync_queue_memory_id ON sync_queue(memory_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_queued_at ON sync_queue(queued_at);
`;
export const SCHEMA_STATEMENTS = [
    CREATE_MEMORIES_TABLE_SQL,
    CREATE_SYNC_QUEUE_TABLE_SQL,
    CREATE_SYNC_CURSORS_TABLE_SQL,
    CREATE_INDEXES_SQL,
];
//# sourceMappingURL=schema.js.map