import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";
export declare const createSystemHook: (client: AeternaClient) => {
    "experimental.chat.system.transform": (input: PluginInput, context: HookContext) => Promise<void>;
};
//# sourceMappingURL=system.d.ts.map