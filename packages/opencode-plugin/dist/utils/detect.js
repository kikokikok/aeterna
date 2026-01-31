const executionHistory = new Map();
const ERROR_PATTERN_THRESHOLD = 3;
export function detectSignificance(input, _context) {
    const sessionId = _context.sessionID ?? "default";
    if (!executionHistory.has(sessionId)) {
        executionHistory.set(sessionId, []);
    }
    const history = executionHistory.get(sessionId);
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
    const lastExecution = toolExecutions[toolExecutions.length - 1];
    if (lastExecution && _context.error) {
        const previousWasError = lastExecution.outcome === "error";
        const currentSuccess = _context.error === undefined;
        if (previousWasError && currentSuccess) {
            return true;
        }
    }
    const outputLength = String(_context.output).length;
    if (outputLength > 500) {
        return true;
    }
    return false;
}
export function recordExecution(sessionId, tool, outcome) {
    if (!executionHistory.has(sessionId)) {
        executionHistory.set(sessionId, []);
    }
    const history = executionHistory.get(sessionId);
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
export function getRepeatedPatterns(sessionId) {
    const history = executionHistory.get(sessionId);
    if (!history)
        return [];
    const toolCounts = new Map();
    for (const exec of history) {
        const count = (toolCounts.get(exec.tool) ?? 0) + 1;
        toolCounts.set(exec.tool, count);
    }
    const patterns = [];
    for (const [tool, count] of toolCounts.entries()) {
        if (count >= ERROR_PATTERN_THRESHOLD) {
            patterns.push(`Repeated ${tool}: ${count} times`);
        }
    }
    return patterns;
}
export function detectNovelApproach(sessionId, tool) {
    const history = executionHistory.get(sessionId);
    if (!history)
        return false;
    const recentExecutions = history.filter((h) => h.tool === tool);
    if (recentExecutions.length < 2)
        return false;
    const lastTwo = recentExecutions.slice(-2);
    if (lastTwo[0].outcome === lastTwo[1].outcome) {
        return false;
    }
    const argsHistory = recentExecutions.map((e) => JSON.stringify(_context.args ?? {}));
    const uniqueArgs = new Set(argsHistory);
    return uniqueArgs.size > 1;
}
export function clearSessionHistory(sessionId) {
    executionHistory.delete(sessionId);
}
//# sourceMappingURL=detect.js.map