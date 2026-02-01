import type { Event } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";

type EventInput = {
  event: Event;
};

export const createSessionHook = (client: AeternaClient) => {
  return async (input: EventInput): Promise<void> => {
    const eventType = (input.event as { type?: string }).type;
    
    if (eventType === "session.start") {
      await client.sessionStart();
    } else if (eventType === "session.end") {
      await client.sessionEnd();
    }
  };
};
