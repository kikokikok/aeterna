import type { PluginInput, HookContext } from "@opencode-ai/plugin";
import type { AeternaClient } from "../client.js";

export const createToolHooks = (client: AeternaClient) => ({
  "tool.execute.before": async (input: PluginInput, context: HookContext) => {
    if (!input.tool.startsWith("aeterna_")) return;

    const enrichedArgs = await client.enrichToolArgs(input.tool, context.args);

    if (Object.keys(enrichedArgs).length > Object.keys(context.args).length) {
      context.args = enrichedArgs;
    }
  },

  "tool.execute.after": async (input: PluginInput, context: HookContext) => {
    const sessionContext = client.getSessionContext();
    if (!sessionContext) return;

    await client.captureToolExecution({
      tool: input.tool,
      sessionId: sessionContext.sessionId,
      callId: context.callID,
      title: context.title,
      args: context.args as Record<string, unknown>,
      output: String(context.output),
      metadata: {
        timestamp: Date.now(),
        directory: input.directory,
      },
      duration: undefined,
      success: !context.error,
    });

    const isSignificant = await client.detectSignificance(
      { tool: input.tool },
      { output: { output: String(context.output) } }
    );

    if (isSignificant) {
      await client.flagForPromotion(sessionContext.sessionId, context.callID);
    }
  },
});
