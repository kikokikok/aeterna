/**
 * Handle OpenCode session lifecycle events.
 *
 * session.start  → start Aeterna session, attempt silent token refresh (task 3.4)
 * session.end    → flush sync queue, end Aeterna session
 */
export const createSessionHook = (client, syncEngine = null) => {
    return async (input) => {
        const eventType = input.event.type;
        if (eventType === "session.start") {
            // (task 3.4) Before starting the session, attempt a silent token refresh
            // when the client holds a refresh token (dynamic auth mode).  This
            // keeps long-running OpenCode sessions authenticated beyond the
            // access-token TTL without user interaction.
            //
            // Only attempt if:
            //   • A refresh token exists (dynamic auth was bootstrapped earlier), AND
            //   • No static AETERNA_TOKEN is in use (static token never expires here)
            if (client.hasRefreshToken() && !process.env.AETERNA_TOKEN) {
                try {
                    await client.refreshAuth();
                }
                catch {
                    // Refresh failed (token revoked / server unreachable).
                    // Log and continue — sessionStart() will create a local fallback
                    // session if the server is unreachable.
                    console.error("[aeterna] Session hook: silent token refresh failed — session may be unauthenticated");
                }
            }
            await client.sessionStart();
        }
        else if (eventType === "session.end") {
            if (syncEngine) {
                await syncEngine.flushOnShutdown();
            }
            await client.sessionEnd();
        }
    };
};
// ---------------------------------------------------------------------------
// Re-auth helper (task 3.4)
// ---------------------------------------------------------------------------
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
export async function callWithReauth(client, fn) {
    try {
        return await fn();
    }
    catch (err) {
        const isAuthError = err instanceof Error &&
            (err.message.includes("401") || err.message.includes("Unauthorized") || err.message.includes("UNAUTHORIZED"));
        if (!isAuthError || !client.hasRefreshToken()) {
            throw err;
        }
        // Attempt token refresh
        try {
            await client.refreshAuth();
        }
        catch {
            console.error("[aeterna] Re-auth: refresh failed — interactive sign-in required");
            return null;
        }
        // Retry the original call once with the new token
        try {
            return await fn();
        }
        catch {
            console.error("[aeterna] Re-auth: retry after refresh also failed");
            return null;
        }
    }
}
//# sourceMappingURL=session.js.map