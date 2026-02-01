interface ExecutionHistory {
  tool: string;
  timestamp: number;
  outcome: "success" | "error";
}

interface ToolExecuteAfterInput {
  tool: string;
  sessionID: string;
  callID: string;
}

interface ToolExecuteAfterOutput {
  title: string;
  output: string;
  metadata: unknown;
}

const executionHistory: Map<string, ExecutionHistory[]> = new Map();
const ERROR_PATTERN_THRESHOLD = 3;

export function detectSignificance(
  input: ToolExecuteAfterInput,
  output: ToolExecuteAfterOutput
): boolean {
  const sessionId = input.sessionID ?? "default";

  if (!executionHistory.has(sessionId)) {
    executionHistory.set(sessionId, []);
  }

  const history = executionHistory.get(sessionId)!;

  const patterns = [
    "aeterna_memory_add",
    "aeterna_knowledge_propose",
    "aeterna_context_assemble",
    "aeterna_note_capture",
    "aeterna_hindsight_query",
  ];

  if (patterns.some((p) => input.tool?.startsWith(p))) {
    return true;
  }

  const toolExecutions = history.filter((h) => h.tool === input.tool);
  const recentCount = toolExecutions.length;

  if (recentCount >= ERROR_PATTERN_THRESHOLD) {
    return true;
  }

  const outputLength = String(output.output).length;
  if (outputLength > 500) {
    return true;
  }

  return false;
}

export function recordExecution(
  sessionId: string,
  tool: string,
  outcome: "success" | "error"
): void {
  if (!executionHistory.has(sessionId)) {
    executionHistory.set(sessionId, []);
  }

  const history = executionHistory.get(sessionId)!;
  history.push({
    tool,
    timestamp: Date.now(),
    outcome,
  });

  history.sort((a, b) => b.timestamp - a.timestamp);

  if (history.length > 50) {
    history.splice(0, history.length - 50);
  }

  executionHistory.set(sessionId, history);
}

export function getRepeatedPatterns(sessionId: string): string[] {
  const history = executionHistory.get(sessionId);
  if (!history) return [];

  const toolCounts = new Map<string, number>();
  for (const exec of history) {
    const count = (toolCounts.get(exec.tool) ?? 0) + 1;
    toolCounts.set(exec.tool, count);
  }

  const patterns: string[] = [];
  for (const [tool, count] of toolCounts.entries()) {
    if (count >= ERROR_PATTERN_THRESHOLD) {
      patterns.push(`Repeated ${tool}: ${count} times`);
    }
  }

  return patterns;
}

export function detectNovelApproach(sessionId: string, tool: string): boolean {
  const history = executionHistory.get(sessionId);
  if (!history) return false;

  const recentExecutions = history.filter((h) => h.tool === tool);

  if (recentExecutions.length < 2) return false;

  const lastTwo = recentExecutions.slice(-2);
  if (lastTwo[0].outcome === lastTwo[1].outcome) {
    return false;
  }

  return true;
}

export function clearSessionHistory(sessionId: string): void {
  executionHistory.delete(sessionId);
}
