import { AeternaClient } from "./client.js";
import { createMemoryTools } from "./tools/memory.js";
import { createGraphTools } from "./tools/graph.js";
import { createCcaTools } from "./tools/cca.js";
import { createKnowledgeTools } from "./tools/knowledge.js";
import { createGovernanceTools } from "./tools/governance.js";
import { createChatHook, createSystemHook, createToolHooks, createPermissionHook, createSessionHook } from "./hooks/index.js";
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
    await client.sessionStart();
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
        event: createSessionHook(client),
    };
};
export default aeterna;
//# sourceMappingURL=index.js.map