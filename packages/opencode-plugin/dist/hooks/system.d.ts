import type { AeternaClient } from "../client.js";
type SystemTransformInput = {
    sessionID: string;
};
type SystemTransformOutput = {
    system: string[];
};
export declare const createSystemHook: (client: AeternaClient) => (_input: SystemTransformInput, output: SystemTransformOutput) => Promise<void>;
export {};
//# sourceMappingURL=system.d.ts.map