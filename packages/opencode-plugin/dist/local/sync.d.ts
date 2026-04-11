import type { AeternaClient } from "../client.js";
import type { LocalConfig } from "./config.js";
import type { LocalMemoryManager } from "./manager.js";
export declare class SyncEngine {
    private readonly manager;
    private readonly client;
    private readonly config;
    private pushInterval;
    private pullInterval;
    private running;
    private readonly deviceId;
    private consecutivePushFailures;
    private nextPushAllowedAt;
    private serverConnectivity;
    constructor(manager: LocalMemoryManager, client: AeternaClient, config: LocalConfig);
    getDeviceId(): string;
    getServerConnectivity(): boolean;
    pushCycle(options?: {
        signal?: AbortSignal;
    }): Promise<void>;
    pullCycle(options?: {
        signal?: AbortSignal;
    }): Promise<void>;
    start(): void;
    stop(): void;
    flushOnShutdown(): Promise<void>;
    private getBackoffDelayMs;
}
//# sourceMappingURL=sync.d.ts.map