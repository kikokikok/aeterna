export const createToolHooks = (client) => ({
    "tool.execute.before": async (input, context) => {
        if (!input.tool.startsWith("aeterna_"))
            return;
        const enrichedArgs = await client.enrichToolArgs(input.tool, context.args);
        if (Object.keys(enrichedArgs).length > Object.keys(context.args).length) {
            context.args = enrichedArgs;
        }
    },
    "tool.execute.after": async (input, context) => {
        const sessionContext = client.getSessionContext();
        if (!sessionContext)
            return;
        await client.captureToolExecution({
            tool: input.tool,
            sessionId: sessionContext.sessionId,
            callId: context.callID,
            title: context.title,
            args: context.args,
            output: String(context.output),
            metadata: {
                timestamp: Date.now(),
                directory: input.directory,
            },
            duration: undefined,
            success: !context.error,
        });
        const isSignificant = await client.detectSignificance({ tool: input.tool }, { output: { output: String(context.output) } });
        if (isSignificant) {
            await client.flagForPromotion(sessionContext.sessionId, context.callID);
        }
    },
});
//# sourceMappingURL=tool.js.map