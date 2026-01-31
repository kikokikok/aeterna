import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";
export declare const createChatHook: (client: AeternaClient) => {
    "chat.message": (input: PluginInput, context: HookContext) => Promise<void>;
};
//# sourceMappingURL=chat.d.ts.map