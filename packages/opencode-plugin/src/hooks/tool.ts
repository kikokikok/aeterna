import type { AeternaClient } from "../client.js";
import { detectSignificance } from "../utils/detect.js";

type ToolExecuteBeforeInput = {
  tool: string;
  sessionID: string;
  callID: string;
};

type ToolExecuteBeforeOutput = {
  args: Record<string, unknown>;
};

type ToolExecuteAfterInput = {
  tool: string;
  sessionID: string;
  callID: string;
};

type ToolExecuteAfterOutput = {
  title: string;
  output: string;
  metadata: unknown;
};

export const createToolHooks = (client: AeternaClient) => ({
  before: async (input: ToolExecuteBeforeInput, output: ToolExecuteBeforeOutput): Promise<void> => {
    if (!input.tool.startsWith("aeterna_")) return;

    const enrichedArgs = await client.enrichToolArgs(input.tool, output.args);

    if (Object.keys(enrichedArgs).length > Object.keys(output.args).length) {
      Object.assign(output.args, enrichedArgs);
    }
  },

  after: async (input: ToolExecuteAfterInput, output: ToolExecuteAfterOutput): Promise<void> => {
    const sessionContext = client.getSessionContext();
    if (!sessionContext) return;

    await client.captureToolExecution({
      tool: input.tool,
      sessionId: sessionContext.sessionId,
      callId: input.callID,
      title: output.title,
      args: {} as Record<string, unknown>,
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
