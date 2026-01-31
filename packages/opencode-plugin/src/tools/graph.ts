import type { tool } from "@opencode-ai/plugin/tool.js";
import { z } from "zod";
import type { AeternaClient } from "../client.js";

export const createGraphTools = (client: AeternaClient) => ({
  aeterna_graph_query: tool({
    description: "Query memory relationships and traversals in the knowledge graph",
    args: {
      startNodeId: z.string().describe("Starting memory/node ID"),
      relations: z.array(z.string()).optional()
        .describe("Relationship types to follow (e.g., 'references', 'implements')"),
      depth: z.number().min(1).max(5).optional()
        .describe("Traversal depth (default: 2)"),
      limit: z.number().min(1).max(100).optional()
        .describe("Max results (default: 50)"),
      direction: z.enum(["outgoing", "incoming", "both"] as const).optional()
        .describe("Direction of traversal (default: outgoing)"),
    },
    async execute(args) {
      const result = await client.graphQuery({
        startNodeId: args.startNodeId,
        relations: args.relations,
        depth: args.depth ?? 2,
        limit: args.limit ?? 50,
        direction: args.direction ?? "outgoing",
      });

      if (result.nodes.length === 0) {
        return `No related nodes found for: ${args.startNodeId}`;
      }

      const nodesList = result.nodes.map((n) => `  [${n.id}] ${n.label}: ${Object.keys(n.properties).join(", ")}`);
      const edgesList = result.edges.map((e) => `  ${e.source} -> ${e.target} (${e.relation})`);
      const pathsSection = result.paths
        ? result.paths.map(
            (p, i) => `Path ${i + 1}: ${p.nodes.join(" -> ")} (weight: ${p.totalWeight})`
          )
            .join("\n")
        : "";

      return `Found ${result.nodes.length} nodes:\n${nodesList}\n\nRelationships:\n${edgesList}\n\n${pathsSection}`;
    },
  }),

  aeterna_graph_neighbors: tool({
    description: "Find memories directly related to a given memory",
    args: {
      nodeId: z.string().describe("Memory/node ID"),
      relations: z.array(z.string()).optional()
        .describe("Filter by relationship types"),
      depth: z.number().min(1).max(3).optional()
        .describe("Search depth (default: 2)"),
      limit: z.number().min(1).max(50).optional()
        .describe("Max results (default: 20)"),
    },
    async execute(args) {
      const result = await client.graphNeighbors({
        nodeId: args.nodeId,
        relations: args.relations,
        depth: args.depth ?? 2,
        limit: args.limit ?? 20,
      });

      if (result.nodes.length === 0) {
        return `No neighbors found for: ${args.nodeId}`;
      }

      return result.nodes
        .map((n) => `- [${n.id}] ${n.label}: ${n.properties.description ?? "No description"}`)
        .join("\n");
    },
  }),

  aeterna_graph_path: tool({
    description: "Find the shortest path between two memories in the knowledge graph",
    args: {
      sourceId: z.string().describe("Source memory/node ID"),
      targetId: z.string().describe("Target memory/node ID"),
      maxDepth: z.number().min(1).max(10).optional()
        .describe("Maximum path depth (default: 5)"),
      relations: z.array(z.string()).optional()
        .describe("Relationship types to traverse"),
    },
    async execute(args) {
      const result = await client.graphPath({
        sourceId: args.sourceId,
        targetId: args.targetId,
        maxDepth: args.maxDepth ?? 5,
        relations: args.relations,
      });

      if (result.nodes.length === 0) {
        return `No path found between ${args.sourceId} and ${args.targetId}`;
      }

      const pathStr = result.nodes.map((n) => n.id).join(" -> ");
      const weightStr = result.totalWeight.toFixed(2);
      return `Path found (${result.length} hops, weight: ${weightStr}):\n${pathStr}`;
    },
  }),
});
