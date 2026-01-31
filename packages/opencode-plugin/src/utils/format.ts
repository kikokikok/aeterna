import type { KnowledgeSearchResult, MemorySearchResult, GeneratedNote, HindsightMatch, AssembledContext, GraphQueryResult } from "../types.js";

export function formatKnowledgeContext(knowledge: KnowledgeSearchResult[]): string {
  if (knowledge.length === 0) return "";

  const sections = knowledge
    .filter((k) => k.score > 0.8)
    .map(
      (k, i) => `### ${i + 1}. ${k.knowledge.title} (${k.knowledge.type.toUpperCase()})\n**${k.summary ?? k.knowledge.content.slice(0, 100)}...**\n\nRelevance: ${(k.score * 100).toFixed(0)}%\n\n`
    );

  return `📚 Relevant Knowledge:\n\n${sections.join("")}`;
}

export function formatMemoryContext(memories: MemorySearchResult[]): string {
  if (memories.length === 0) return "";

  const sections = memories
    .filter((m) => m.score > 0.75)
    .map(
      (m, i) => `### ${i + 1}. ${m.memory.content.slice(0, 80)}...\n**Layer: ${m.memory.layer}**\n**Importance: ${m.memory.importance.toFixed(2)}**\n\n`
    );

  return `💭 Relevant Memories:\n\n${sections.join("")}`;
}

export function formatNote(notes: GeneratedNote[]): string {
  if (notes.length === 0) return "";

  return notes
    .map(
      (n, i) => `### Note ${i + 1}: ${n.title}\n${n.content}\n\n${n.tags.map((t) => `#${t}`).join(" ")}\n\n`
    )
    .join("---\n");
}

export function formatHindsightResults(matches: HindsightMatch[]): string {
  if (matches.length === 0) return "No error patterns found.";

  return matches
    .map(
      (m, i) => {
        const note = m.note;
        return `### Error Pattern ${i + 1}: ${note.errorSignature}\n**Type**: ${note.errorType}\n**Resolution**: ${note.resolution}\n**Occurrences**: ${note.occurrences}\n**Success Rate**: ${(note.successRate * 100).toFixed(1)}%\n\nMatched patterns: ${m.matchedPatterns.join(", ")}\n`;
      }
    )
    .join("---\n");
}

export function formatAssembledContext(context: AssembledContext): string {
  const summaryLines = [
    `**Tokens Used**: ${context.tokensUsed}/${context.tokenBudget} (${((context.tokensUsed / context.tokenBudget) * 100).toFixed(1)}%)`,
    `**Truncated**: ${context.truncated ? "Yes" : "No"}`,
  ];

  if (context.sources.length > 0) {
    const topSources = context.sources.slice(0, 5);
    const sourcesList = topSources
      .map((s, i) => `${i + 1}. [${s.id}] ${s.layer}: ${s.relevance.toFixed(2)}`)
      .join("\n");
    summaryLines.push(`\n**Top Sources**:\n${sourcesList}`);
  }

  const layerBreakdown = Object.entries(context.layerBreakdown)
    .map(([layer, tokens]) => `- ${layer}: ${tokens} tokens`)
    .join("\n");

  summaryLines.push(`\n**Layer Breakdown**:\n${layerBreakdown}`);

  return `🧩 Assembled Context:\n\n${summaryLines.join("\n")}`;
}

export function formatGraphResults(result: GraphQueryResult): string {
  const lines = [`**Found ${result.nodes.length} nodes, ${result.edges.length} relationships`];

  if (result.paths && result.paths.length > 0) {
    const pathLines = result.paths.map((p, i) => {
      const pathStr = p.nodes.join(" → ");
      const weight = p.totalWeight?.toFixed(2) ?? "N/A";
      return `  Path ${i + 1}: ${pathStr} (weight: ${weight})`;
    });
    lines.push(`\n**Paths**:\n${pathLines.join("\n")}`);
  }

  if (result.nodes.length > 0) {
    const nodesByType: Record<string, string[]> = {};
    for (const node of result.nodes) {
      if (!nodesByType[node.nodeType]) nodesByType[node.nodeType] = [];
      nodesByType[node.nodeType].push(`  ${node.id}: ${node.label}`);
    }

    for (const [nodeType, nodes] of Object.entries(nodesByType)) {
      lines.push(`\n**${nodeType}** (${nodes.length}):\n${nodes.join("\n")}`);
    }
  }

  return lines.join("\n");
}
