export const createSystemHook = (client) => ({
    "experimental.chat.system.transform": async (input, context) => {
        const sessionContext = client.getSessionContext();
        const projectContext = await client.getProjectContext();
        const policiesText = projectContext.policies
            .map((p) => `- ${p.name}: ${p.summary}`)
            .join("\n");
        const memoriesText = projectContext.recentMemories
            .map((m) => `- [${m.id}] ${m.summary}`)
            .join("\n");
        const guidance = `
## Aeterna Integration

You are integrated with Aeterna memory and knowledge system. This enhances your context with:

**Available Tools:**
- \`aeterna_memory_add\` - Capture learnings and solutions
- \`aeterna_memory_search\` - Find relevant context
- \`aeterna_context_assemble\` - Assemble hierarchical context
- \`aeterna_graph_query\` - Explore memory relationships
- \`aeterna_hindsight_query\` - Find error resolution patterns

**Best Practices:**
- Capture important patterns with \`aeterna_memory_add\`
- Use \`aeterna_context_assemble\` for complex tasks requiring broader context
- Check \`aeterna_hindsight_query\` before retrying failed approaches
- Use \`aeterna_graph_query\` to find related code and decisions
`;
        const systemContext = `
## Aeterna Project Context

Project: ${projectContext.project.name}
${projectContext.team ? `Team: ${projectContext.team.name}` : ""}
${projectContext.org ? `Organization: ${projectContext.org.name}` : ""}

### Active Policies

${policiesText || "No policies configured"}

### Recent Learnings

${memoriesText || "No recent learnings"}

${guidance}
`;
        context.output.system.push(systemContext);
    },
});
//# sourceMappingURL=system.js.map