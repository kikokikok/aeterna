/**
 * Aeterna OpenCode Plugin - Type Definitions
 *
 * Shared types for the plugin, matching the Aeterna Rust backend API.
 */
/** Default plugin configuration */
export const DEFAULT_CONFIG = {
    capture: {
        enabled: true,
        sensitivity: "medium",
        autoPromote: true,
        sampleRate: 1.0,
        debounceMs: 500,
    },
    knowledge: {
        injectionEnabled: true,
        maxItems: 3,
        threshold: 0.75,
        cacheTtlSeconds: 60,
        timeoutMs: 200,
    },
    governance: {
        notifications: true,
        driftAlerts: true,
    },
    session: {
        storageTtlHours: 24,
        useRedis: false,
    },
    experimental: {
        systemPromptHook: true,
        permissionHook: true,
    },
    // auth is intentionally omitted from the default — disabled unless explicitly
    // configured via env vars or .aeterna/config.toml [auth] section.
};
//# sourceMappingURL=types.js.map