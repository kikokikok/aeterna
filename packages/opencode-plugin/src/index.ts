import type { Plugin, PluginInput, Hooks } from "@opencode-ai/plugin";
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

export const aeterna: Plugin = async (input: PluginInput): Promise<Hooks> => {
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
  let syncEngine: SyncEngine | null = null;

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
    event: createSessionHook(client, syncEngine),
  };
};

export default aeterna;
