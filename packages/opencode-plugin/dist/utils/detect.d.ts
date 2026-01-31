import type { PluginInput, HookContext } from "@opencode-ai/plugin";
export declare function detectSignificance(input: PluginInput, _context: HookContext): boolean;
export declare function recordExecution(sessionId: string, tool: string, outcome: "success" | "error"): void;
export declare function getRepeatedPatterns(sessionId: string): string[];
export declare function detectNovelApproach(sessionId: string, tool: string): boolean;
export declare function clearSessionHistory(sessionId: string): void;
//# sourceMappingURL=detect.d.ts.map