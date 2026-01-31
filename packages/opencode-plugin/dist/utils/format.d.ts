import type { KnowledgeSearchResult, MemorySearchResult, GeneratedNote, HindsightMatch, AssembledContext, GraphQueryResult } from "../types.js";
export declare function formatKnowledgeContext(knowledge: KnowledgeSearchResult[]): string;
export declare function formatMemoryContext(memories: MemorySearchResult[]): string;
export declare function formatNote(notes: GeneratedNote[]): string;
export declare function formatHindsightResults(matches: HindsightMatch[]): string;
export declare function formatAssembledContext(context: AssembledContext): string;
export declare function formatGraphResults(result: GraphQueryResult): string;
//# sourceMappingURL=format.d.ts.map