import { tool, type ToolDefinition } from "@opencode-ai/plugin/tool";
import type { AeternaClient } from "../client.js";

export const createGovernanceTools = (client: AeternaClient): Record<string, ToolDefinition> => ({
  aeterna_sync_status: tool({
    description: "Check the sync status between memory and knowledge repository",
    args: {},
    async execute(_args, _context) {
      const status = await client.getSyncStatus();
      const statusEmoji = {
        healthy: "✅",
        degraded: "⚠️",
        error: "❌",
      };

      const errorsSection = status.errors && status.errors.length > 0
        ? `\n\nErrors:\n${status.errors.map((e) => `  - ${e}`).join("\n")}`
        : "";

      return `Sync status: ${statusEmoji[status.syncHealth]} ${status.syncHealth}\n\nLast sync: ${new Date(status.lastSync).toLocaleString()}\nPending promotions: ${status.pendingPromotions}\nPending proposals: ${status.pendingProposals}${errorsSection}`;
    },
  }),

  aeterna_governance_status: tool({
    description: "Check governance state: policies, proposals, and compliance",
    args: {},
    async execute(_args, _context) {
      const status = await client.getGovernanceStatus();
      const driftEmoji = status.driftDetected ? "⚠️" : "✅";

      return `Governance Status:\n\nActive policies: ${status.activePolicies}\nPending proposals: ${status.pendingProposals}\nRecent violations: ${status.recentViolations}\nSemantic drift: ${driftEmoji} ${status.driftDetected ? "detected" : "none"}\n\nRecent notifications:\n${status.notifications.length > 0 ? status.notifications.map((n) => `  [${n.type}] ${n.message}`).join("\n") : "  None"}`;
    },
  }),
});
