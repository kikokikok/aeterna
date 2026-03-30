import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { Database } from "bun:sqlite";
import { SCHEMA_STATEMENTS, SCHEMA_VERSION } from "./schema.js";

export class LocalDatabase {
  private readonly db: Database;

  constructor(dbPath: string) {
    mkdirSync(dirname(dbPath), { recursive: true });
    this.db = new Database(dbPath, { strict: true });
    this.initialize();
  }

  get connection(): Database {
    return this.db;
  }

  close(): void {
    this.db.close();
  }

  private initialize(): void {
    this.db.pragma("journal_mode = WAL");
    this.db.pragma("busy_timeout = 5000");

    const currentVersion = this.db.pragma("user_version", { simple: true }) as number;
    if (currentVersion > SCHEMA_VERSION) {
      throw new Error(
        `Unsupported local schema version ${currentVersion}, expected <= ${SCHEMA_VERSION}`
      );
    }

    for (const statement of SCHEMA_STATEMENTS) {
      this.db.exec(statement);
    }

    if (currentVersion < SCHEMA_VERSION) {
      this.db.pragma(`user_version = ${SCHEMA_VERSION}`);
    }
  }
}
