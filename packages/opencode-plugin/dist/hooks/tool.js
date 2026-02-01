import { detectSignificance } from "../utils/detect.js";
export const createToolHooks = (client) => ({
    before: async (input, output) => {
        if (!input.tool.startsWith("aeterna_"))
            return;
        const enrichedArgs = await client.enrichToolArgs(input.tool, output.args);
        if (Object.keys(enrichedArgs).length > Object.keys(output.args).length) {
            Object.assign(output.args, enrichedArgs);
        }
    },
    after: async (input, output) => {
        const sessionContext = client.getSessionContext();
        if (!sessionContext)
            return;
        await client.captureToolExecution({
            tool: input.tool,
            sessionId: sessionContext.sessionId,
            callId: input.callID,
            title: output.title,
            args: {},
            output: String(output.output),
            metadata: {},
            timestamp: Date.now(),
            duration: undefined,
            success: true,
        });
        const isSignificant = detectSignificance(input, output);
        if (isSignificant) {
            await client.flagForPromotion(sessionContext.sessionId, input.callID);
        }
    },
});
//# sourceMappingURL=tool.js.map