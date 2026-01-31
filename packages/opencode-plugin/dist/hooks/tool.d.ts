import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";
export declare const createToolHooks: (client: AeternaClient) => {
    "tool.execute.before": (input: PluginInput, context: HookContext) => Promise<void>;
    "tool.execute.after": (input: PluginInput, context: HookContext) => Promise<void>;
};
//# sourceMappingURL=tool.d.ts.map