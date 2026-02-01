import type { Event } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
type EventInput = {
    event: Event;
};
export declare const createSessionHook: (client: AeternaClient) => (input: EventInput) => Promise<void>;
export {};
//# sourceMappingURL=session.d.ts.map