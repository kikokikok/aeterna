export const createPermissionHook = (client) => ({
    "permission.ask": async (input, context) => {
        if (!input.tool?.startsWith("aeterna_knowledge_propose")) {
            context.status = "allow";
            return;
        }
        const canPropose = await client.checkProposalPermission();
        if (!canPropose) {
            context.status = "deny";
            context.message = "You do not have permission to propose knowledge to this scope. Contact your team lead or architect.";
        }
        else {
            context.status = "allow";
        }
    },
});
//# sourceMappingURL=permission.js.map