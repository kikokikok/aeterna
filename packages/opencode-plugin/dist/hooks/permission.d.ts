import type { Permission } from "@opencode-ai/sdk";
import type { AeternaClient } from "../client.js";
type PermissionOutput = {
    status: "ask" | "deny" | "allow";
};
export declare const createPermissionHook: (client: AeternaClient) => (input: Permission, output: PermissionOutput) => Promise<void>;
export {};
//# sourceMappingURL=permission.d.ts.map