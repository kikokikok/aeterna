import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";
export declare const createPermissionHook: (client: AeternaClient) => {
    "permission.ask": (input: PluginInput, context: HookContext) => Promise<void>;
};
//# sourceMappingURL=permission.d.ts.map