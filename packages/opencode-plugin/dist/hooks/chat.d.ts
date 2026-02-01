import type { Part, UserMessage } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
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
export declare const createChatHook: (client: AeternaClient) => (_input: ChatMessageInput, output: ChatMessageOutput) => Promise<void>;
export {};
//# sourceMappingURL=chat.d.ts.map