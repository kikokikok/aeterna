export const createSessionHook = (client) => ({
    event: async (input, context) => {
        if (context.event.type === "session.start") {
            await client.sessionStart();
        }
        else if (context.event.type === "session.end") {
            await client.sessionEnd();
        }
    },
});
//# sourceMappingURL=session.js.map