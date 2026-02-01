export const createPermissionHook = (client) => {
    return async (input, output) => {
        const toolName = input.tool;
        if (!toolName?.startsWith("aeterna_knowledge_propose")) {
            output.status = "allow";
            return;
        }
        const canPropose = await client.checkProposalPermission();
        if (!canPropose) {
            output.status = "deny";
        }
        else {
            output.status = "allow";
        }
    };
};
//# sourceMappingURL=permission.js.map