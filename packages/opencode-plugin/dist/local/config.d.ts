export interface LocalConfig {
    enabled: boolean;
    db_path: string;
    sync_push_interval_ms: number;
    sync_pull_interval_ms: number;
    max_cached_entries: number;
    session_storage_ttl_hours: number;
}
export declare const DEFAULT_LOCAL_CONFIG: LocalConfig;
export declare const parseLocalConfig: (env?: NodeJS.ProcessEnv, cwd?: string) => LocalConfig;
//# sourceMappingURL=config.d.ts.map