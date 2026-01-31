import { formatKnowledgeContext, formatMemoryContext } from "../utils/format.js";
export const createChatHook = (client) => ({
    "chat.message": async (input, context) => {
        const sessionContext = client.getSessionContext();
        if (!sessionContext)
            return;
        const userMessage = context.message.content;
        const [knowledge, memories] = await Promise.all([
            client.queryRelevantKnowledge(userMessage, {
                limit: 3,
                threshold: 0.75,
            }),
            client.searchSessionMemories(userMessage, { limit: 5 }),
        ]);
        if (knowledge.length === 0 && memories.length === 0)
            return;
        const contextParts = [];
        if (knowledge.length > 0) {
            const knowledgeText = formatKnowledgeContext(knowledge);
            contextParts.push({
                type: "text",
                text: knowledgeText,
            });
        }
        if (memories.length > 0) {
            const memoryText = formatMemoryContext(memories);
            contextParts.push({
                type: "text",
                text: memoryText,
            });
        }
        if (contextParts.length > 0) {
            const combinedContext = contextParts.map((p) => p.text).join("\n\n");
            context.output.parts.unshift({
                type: "text",
                text: `<aeterna_context>\n${combinedContext}\n</aeterna_context>`,
            });
        }
    },
});
//# sourceMappingURL=chat.js.map