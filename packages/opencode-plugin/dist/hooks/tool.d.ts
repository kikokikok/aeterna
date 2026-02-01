import type { AeternaClient } from "../client.js";
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
export declare const createToolHooks: (client: AeternaClient) => {
    before: (input: ToolExecuteBeforeInput, output: ToolExecuteBeforeOutput) => Promise<void>;
    after: (input: ToolExecuteAfterInput, output: ToolExecuteAfterOutput) => Promise<void>;
};
export {};
//# sourceMappingURL=tool.d.ts.map