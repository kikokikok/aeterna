import type { Permission } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";

type PermissionOutput = {
  status: "ask" | "deny" | "allow";
};

export const createPermissionHook = (client: AeternaClient) => {
  return async (input: Permission, output: PermissionOutput): Promise<void> => {
    const toolName = (input as { tool?: string }).tool;
    
    if (!toolName?.startsWith("aeterna_knowledge_propose")) {
      output.status = "allow";
      return;
    }

    const canPropose = await client.checkProposalPermission();

    if (!canPropose) {
      output.status = "deny";
    } else {
      output.status = "allow";
    }
  };
};
