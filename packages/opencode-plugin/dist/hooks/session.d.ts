import type { Event } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
import type { SyncEngine } from "../local/sync.js";
type EventInput = {
    event: Event;
};
/**
 * Handle OpenCode session lifecycle events.
 *
 * session.start  → start Aeterna session, attempt silent token refresh (task 3.4)
 * session.end    → flush sync queue, end Aeterna session
 */
export declare const createSessionHook: (client: AeternaClient, syncEngine?: SyncEngine | null) => (input: EventInput) => Promise<void>;
/**
 * Attempt to recover from a 401 by refreshing the token, then retry once.
 *
 * Usage pattern for API callers that detect a 401:
 *
 *   const result = await callWithReauth(client, () => doApiCall());
 *
 * Returns null when re-authentication fails and the caller must prompt the
 * user to sign in again.
 */
export declare function callWithReauth<T>(client: AeternaClient, fn: () => Promise<T>): Promise<T | null>;
export {};
//# sourceMappingURL=session.d.ts.map