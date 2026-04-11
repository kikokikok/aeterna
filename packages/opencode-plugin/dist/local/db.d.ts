import { Database } from "bun:sqlite";
export declare class LocalDatabase {
    private readonly db;
    constructor(dbPath: string);
    get connection(): Database;
    close(): void;
    private initialize;
}
//# sourceMappingURL=db.d.ts.map