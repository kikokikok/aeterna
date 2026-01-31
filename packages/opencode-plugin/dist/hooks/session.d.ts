import type { PluginInput, EventHookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";
export declare const createSessionHook: (client: AeternaClient) => {
    event: (input: PluginInput, context: EventHookContext) => Promise<void>;
};
//# sourceMappingURL=session.d.ts.map