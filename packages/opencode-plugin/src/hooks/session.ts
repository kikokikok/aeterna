import type { PluginInput, EventHookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";

export const createSessionHook = (client: AeternaClient) => ({
  event: async (input: PluginInput, context: EventHookContext) => {
    if (context.event.type === "session.start") {
      await client.sessionStart();
    } else if (context.event.type === "session.end") {
      await client.sessionEnd();
    }
  },
});
