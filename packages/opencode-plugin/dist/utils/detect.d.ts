interface ToolExecuteAfterInput {
    tool: string;
    sessionID: string;
    callID: string;
}
interface ToolExecuteAfterOutput {
    title: string;
    output: string;
    metadata: unknown;
}
export declare function detectSignificance(input: ToolExecuteAfterInput, output: ToolExecuteAfterOutput): boolean;
export declare function recordExecution(sessionId: string, tool: string, outcome: "success" | "error"): void;
export declare function getRepeatedPatterns(sessionId: string): string[];
export declare function detectNovelApproach(sessionId: string, tool: string): boolean;
export declare function clearSessionHistory(sessionId: string): void;
export {};
//# sourceMappingURL=detect.d.ts.map