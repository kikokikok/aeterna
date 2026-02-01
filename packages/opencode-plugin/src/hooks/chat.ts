import type { Part, UserMessage } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
import { formatKnowledgeContext, formatMemoryContext } from "../utils/format.js";

type ChatMessageInput = {
  sessionID: string;
  agent?: string;
  model?: {
    providerID: string;
    modelID: string;
  };
  messageID?: string;
  variant?: string;
};

type ChatMessageOutput = {
  message: UserMessage;
  parts: Part[];
};

export const createChatHook = (client: AeternaClient) => {
  return async (_input: ChatMessageInput, output: ChatMessageOutput): Promise<void> => {
    const sessionContext = client.getSessionContext();
    if (!sessionContext) return;

    const textParts = output.parts.filter(
      (part): part is Part & { type: "text"; text: string } => part.type === "text"
    );
    const userMessage = textParts.map((p) => p.text).join("\n");

    if (!userMessage) return;

    const [knowledge, memories] = await Promise.all([
      client.queryRelevantKnowledge(userMessage, {
        limit: 3,
        threshold: 0.75,
      }),
      client.searchSessionMemories(userMessage, { limit: 5 }),
    ]);

    if (knowledge.length === 0 && memories.length === 0) return;

    const contextParts: string[] = [];

    if (knowledge.length > 0) {
      contextParts.push(formatKnowledgeContext(knowledge));
    }

    if (memories.length > 0) {
      contextParts.push(formatMemoryContext(memories));
    }

    if (contextParts.length > 0) {
      const combinedContext = contextParts.join("\n\n");
      output.parts.unshift({
        type: "text",
        text: `<aeterna_context>\n${combinedContext}\n</aeterna_context>`,
      } as Part);
    }
  };
};
