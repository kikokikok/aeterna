import { AeternaClient } from "./client.js";
import { createMemoryTools } from "./tools/memory.js";
import { createGraphTools } from "./tools/graph.js";
import { createCcaTools } from "./tools/cca.js";
import { createKnowledgeTools } from "./tools/knowledge.js";
import { createGovernanceTools } from "./tools/governance.js";
import { createChatHook, createSystemHook, createToolHooks, createPermissionHook, createSessionHook } from "./hooks/index.js";
import { parseLocalConfig } from "./local/config.js";
import { LocalMemoryManager } from "./local/manager.js";
import { SyncEngine } from "./local/sync.js";
import { MemoryRouter } from "./local/router.js";
// ---------------------------------------------------------------------------
// Auth bootstrap helpers – GitHub OAuth device flow
// ---------------------------------------------------------------------------
/**
 * Attempt to authenticate the plugin client before the session starts.
 *
 * Priority:
 *  1. `AETERNA_TOKEN` set → static token already loaded by constructor.
 *  2. `AETERNA_PLUGIN_AUTH_ENABLED=true` → device-flow auth:
 *     a. Refresh token available → silent refresh.
 *     b. Otherwise → request device code, show user_code + verification_uri,
 *        poll GitHub for an access token, then bootstrap Aeterna auth.
 *  3. Neither → unauthenticated / local-only.
 *
 * Never throws — failures are logged; the plugin continues in degraded mode.
 */
async function attemptPluginAuth(client) {
    // (1) Static token already loaded — nothing to do.
    if (process.env.AETERNA_TOKEN) {
        return;
    }
    // (2) Dynamic auth enabled?
    if (process.env.AETERNA_PLUGIN_AUTH_ENABLED !== "true") {
        return;
    }
    // (2a) Refresh token in environment → silent refresh
    const cachedRefreshToken = process.env.AETERNA_PLUGIN_REFRESH_TOKEN;
    if (cachedRefreshToken) {
        try {
            client.setAuthTokens("", cachedRefreshToken);
            const tokens = await client.refreshAuth();
            console.error(`[aeterna] Plugin auth: refreshed token for ${tokens.githubLogin}`);
            return;
        }
        catch (err) {
            console.error(`[aeterna] Plugin auth: silent refresh failed (${err.message}), attempting device flow sign-in`);
        }
    }
    // (2b) Device flow bootstrap
    const clientId = process.env.AETERNA_PLUGIN_AUTH_GITHUB_CLIENT_ID;
    if (!clientId) {
        console.error("[aeterna] Plugin auth: AETERNA_PLUGIN_AUTH_GITHUB_CLIENT_ID is not set — skipping interactive auth");
        return;
    }
    try {
        const deviceResp = await client.requestDeviceCode(clientId);
        console.error(`[aeterna] Plugin auth: please visit the following URL and enter the code shown below:\n\n` +
            `  URL:  ${deviceResp.verification_uri}\n` +
            `  Code: ${deviceResp.user_code}\n\n` +
            `Waiting for authorisation…`);
        const githubAccessToken = await client.pollDeviceToken(clientId, deviceResp.device_code, deviceResp.interval, deviceResp.expires_in);
        const tokens = await client.bootstrapAuth(githubAccessToken);
        console.error(`[aeterna] Plugin auth: signed in as ${tokens.githubLogin}`);
    }
    catch (err) {
        console.error(`[aeterna] Plugin auth: device flow failed (${err.message}) — continuing unauthenticated`);
    }
}
// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------
export const aeterna = async (input) => {
    // Extract project identifier from worktree path or project ID
    // OpenCode SDK Project type has: id, worktree, vcsDir, vcs, time
    const projectName = input.project.id || input.worktree.split("/").pop() || "unknown";
    // Team and org can be configured via environment variables since
    // the OpenCode SDK doesn't provide them in the PluginInput
    const team = process.env.AETERNA_TEAM;
    const org = process.env.AETERNA_ORG;
    const client = new AeternaClient({
        project: projectName,
        directory: input.directory,
        serverUrl: process.env.AETERNA_SERVER_URL,
        token: process.env.AETERNA_TOKEN,
        team,
        org,
        // userId not available in PluginInput, can be set via environment
        userId: process.env.AETERNA_USER_ID,
    });
    const localConfig = parseLocalConfig(process.env, input.directory);
    let syncEngine = null;
    if (localConfig.enabled) {
        const localManager = new LocalMemoryManager(localConfig.db_path, localConfig);
        syncEngine = new SyncEngine(localManager, client, localConfig);
        const router = new MemoryRouter(localManager, client, localConfig);
        client.setRouter(router);
        client.setLocalManager(localManager);
        client.setSyncEngine(syncEngine);
        if (process.env.AETERNA_SERVER_URL) {
            syncEngine.start();
        }
    }
    // Authenticate before OpenCode session lifecycle begins.
    // Session creation itself is owned by the session hook so we do not create
    // duplicate backend sessions during plugin startup and `session.start`.
    await attemptPluginAuth(client);
    return {
        tool: {
            ...createMemoryTools(client),
            ...createGraphTools(client),
            ...createCcaTools(client),
            ...createKnowledgeTools(client),
            ...createGovernanceTools(client),
        },
        "chat.message": createChatHook(client),
        "experimental.chat.system.transform": createSystemHook(client),
        "tool.execute.before": createToolHooks(client).before,
        "tool.execute.after": createToolHooks(client).after,
        "permission.ask": createPermissionHook(client),
        event: createSessionHook(client, syncEngine),
    };
};
export default aeterna;
//# sourceMappingURL=index.js.map