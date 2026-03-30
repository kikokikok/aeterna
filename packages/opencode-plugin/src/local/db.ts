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
    this.db.exec("PRAGMA journal_mode = WAL");
    this.db.exec("PRAGMA busy_timeout = 5000");

    const currentVersion =
      this.db
        .query<never, { user_version: number }>("PRAGMA user_version")
        .get()?.user_version ?? 0;
    if (currentVersion > SCHEMA_VERSION) {
      throw new Error(
        `Unsupported local schema version ${currentVersion}, expected <= ${SCHEMA_VERSION}`
      );
    }

    for (const statement of SCHEMA_STATEMENTS) {
      this.db.exec(statement);
    }

    if (currentVersion < SCHEMA_VERSION) {
      this.db.exec(`PRAGMA user_version = ${SCHEMA_VERSION}`);
    }
  }
}
