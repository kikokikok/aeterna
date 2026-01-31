import { AeternaClient } from "./client.js";
import { createMemoryTools } from "./tools/memory.js";
import { createGraphTools } from "./tools/graph.js";
import { createCcaTools } from "./tools/cca.js";
import { createKnowledgeTools } from "./tools/knowledge.js";
import { createGovernanceTools } from "./tools/governance.js";
import { createChatHook, createSystemHook, createToolHooks, createPermissionHook, createSessionHook } from "./hooks/index.js";
export const aeterna = async (input) => {
    const client = new AeternaClient({
        project: input.project.name,
        directory: input.directory,
        serverUrl: process.env.AETERNA_SERVER_URL,
        token: process.env.AETERNA_TOKEN,
        team: input.project.org ?? input.project.team,
        org: input.project.org,
        userId: input.user?.id,
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