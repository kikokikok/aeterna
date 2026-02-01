import { tool } from "@opencode-ai/plugin/tool";
const z = tool.schema;
export const createKnowledgeTools = (client) => ({
    aeterna_knowledge_query: tool({
        description: "Query the knowledge repository for project/team/org knowledge",
        args: {
            query: z.string().describe("Search query"),
            scope: z.enum(["project", "team", "org", "company"]).optional()
                .describe("Knowledge scope (default: project)"),
            types: z.array(z.enum(["adr", "pattern", "policy", "reference"])).optional()
                .describe("Knowledge types to include"),
            limit: z.number().min(1).max(20).optional()
                .describe("Max results (default: 3)"),
            threshold: z.number().min(0).max(1).optional()
                .describe("Similarity threshold (default: 0.75)"),
        },
        async execute(args, _context) {
            const results = await client.knowledgeQuery({
                query: args.query,
                scope: args.scope ?? "project",
                types: args.types,
                limit: args.limit ?? 3,
                threshold: args.threshold ?? 0.75,
            });
            if (results.length === 0) {
                return `No knowledge found for query: "${args.query}"`;
            }
            return results
                .map((r) => `- [${r.knowledge.type.toUpperCase()}] ${r.knowledge.title} (${r.knowledge.scope}): ${r.knowledge.content.slice(0, 150)}...`)
                .join("\n");
        },
    }),
    aeterna_knowledge_propose: tool({
        description: "Propose new knowledge to the repository (requires governance approval)",
        args: {
            type: z.enum(["adr", "pattern", "policy", "reference"])
                .describe("Knowledge type: adr, pattern, policy, or reference"),
            title: z.string().describe("Knowledge item title"),
            content: z.string().describe("Knowledge content"),
            scope: z.enum(["project", "team", "org", "company"])
                .describe("Knowledge scope level"),
            tags: z.array(z.string()).optional()
                .describe("Tags for categorization"),
            metadata: z.record(z.string(), z.unknown()).optional()
                .describe("Additional metadata"),
        },
        async execute(args, _context) {
            const result = await client.knowledgePropose({
                type: args.type,
                title: args.title,
                content: args.content,
                scope: args.scope,
                tags: args.tags,
                metadata: args.metadata,
            });
            return `Knowledge proposed: ${result.id} (status: ${result.status}, type: ${result.type}) - awaiting governance approval`;
        },
    }),
});
//# sourceMappingURL=knowledge.js.map