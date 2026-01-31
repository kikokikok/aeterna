import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";

export const createPermissionHook = (client: AeternaClient) => ({
  "permission.ask": async (input: PluginInput, context: HookContext) => {
    if (!input.tool?.startsWith("aeterna_knowledge_propose")) {
      context.status = "allow";
      return;
    }

    const canPropose = await client.checkProposalPermission();

    if (!canPropose) {
      context.status = "deny";
      context.message = "You do not have permission to propose knowledge to this scope. Contact your team lead or architect.";
    } else {
      context.status = "allow";
    }
  },
});
