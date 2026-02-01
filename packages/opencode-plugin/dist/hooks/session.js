export const createSessionHook = (client) => {
    return async (input) => {
        const eventType = input.event.type;
        if (eventType === "session.start") {
            await client.sessionStart();
        }
        else if (eventType === "session.end") {
            await client.sessionEnd();
        }
    };
};
//# sourceMappingURL=session.js.map