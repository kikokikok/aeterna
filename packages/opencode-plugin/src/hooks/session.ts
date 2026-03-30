import type { Event } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
import type { SyncEngine } from "../local/sync.js";

type EventInput = {
  event: Event;
};

export const createSessionHook = (client: AeternaClient, syncEngine: SyncEngine | null = null) => {
  return async (input: EventInput): Promise<void> => {
    const eventType = (input.event as { type?: string }).type;
    
    if (eventType === "session.start") {
      await client.sessionStart();
    } else if (eventType === "session.end") {
      if (syncEngine) {
        await syncEngine.flushOnShutdown();
      }
      await client.sessionEnd();
    }
  };
};
