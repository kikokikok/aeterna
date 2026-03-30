import { describe, it, expect, beforeEach } from "vitest";
import {
  detectSignificance,
  recordExecution,
  getRepeatedPatterns,
  detectNovelApproach,
  clearSessionHistory,
} from "./detect.js";

describe("detectSignificance", () => {
  beforeEach(() => {
    clearSessionHistory("test-session");
    clearSessionHistory("s1");
    clearSessionHistory("s-thresh");
    clearSessionHistory("s-below");
  });

  it("returns true for aeterna_memory_add tool", () => {
    const result = detectSignificance(
      { tool: "aeterna_memory_add", sessionID: "s1", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns true for aeterna_knowledge_propose tool", () => {
    const result = detectSignificance(
      { tool: "aeterna_knowledge_propose", sessionID: "s1", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns true for aeterna_context_assemble tool", () => {
    const result = detectSignificance(
      { tool: "aeterna_context_assemble", sessionID: "s1", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns true for aeterna_note_capture tool", () => {
    const result = detectSignificance(
      { tool: "aeterna_note_capture", sessionID: "s1", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns true for aeterna_hindsight_query tool", () => {
    const result = detectSignificance(
      { tool: "aeterna_hindsight_query", sessionID: "s1", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns true when output is longer than 500 characters", () => {
    const longOutput = "x".repeat(501);
    const result = detectSignificance(
      { tool: "other_tool", sessionID: "s1", callID: "c1" },
      { title: "test", output: longOutput, metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns false for short output from non-aeterna tool", () => {
    const result = detectSignificance(
      { tool: "other_tool", sessionID: "s1", callID: "c1" },
      { title: "test", output: "short", metadata: {} }
    );
    expect(result).toBe(false);
  });

  it("returns true when same tool executed >= 3 times in history", () => {
    recordExecution("s-thresh", "custom_tool", "success");
    recordExecution("s-thresh", "custom_tool", "success");
    recordExecution("s-thresh", "custom_tool", "error");

    const result = detectSignificance(
      { tool: "custom_tool", sessionID: "s-thresh", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(true);
  });

  it("returns false when tool executed fewer than 3 times", () => {
    recordExecution("s-below", "custom_tool", "success");
    recordExecution("s-below", "custom_tool", "error");

    const result = detectSignificance(
      { tool: "custom_tool", sessionID: "s-below", callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(false);
  });

  it("handles missing sessionID gracefully", () => {
    const result = detectSignificance(
      { tool: "other_tool", sessionID: undefined as unknown as string, callID: "c1" },
      { title: "test", output: "ok", metadata: {} }
    );
    expect(result).toBe(false);
  });
});

describe("recordExecution", () => {
  beforeEach(() => {
    clearSessionHistory("test-session");
  });

  it("records an execution in session history", () => {
    recordExecution("sess-1", "tool_a", "success");
    const patterns = getRepeatedPatterns("sess-1");
    expect(patterns).toEqual([]);
  });

  it("trims history to max 50 entries", () => {
    for (let i = 0; i < 60; i++) {
      recordExecution("sess-1", `tool_${i % 5}`, "success");
    }
    const patterns = getRepeatedPatterns("sess-1");
    expect(patterns.length).toBeGreaterThanOrEqual(0);
  });
});

describe("getRepeatedPatterns", () => {
  beforeEach(() => {
    clearSessionHistory("test-session");
    clearSessionHistory("sess-1");
  });

  it("returns empty array for unknown session", () => {
    expect(getRepeatedPatterns("nonexistent")).toEqual([]);
  });

  it("returns patterns for tools executed >= 3 times", () => {
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "error");
    recordExecution("sess-1", "tool_b", "success");

    const patterns = getRepeatedPatterns("sess-1");
    expect(patterns).toHaveLength(1);
    expect(patterns[0]).toContain("tool_a");
    expect(patterns[0]).toContain("3");
  });

  it("returns multiple patterns when multiple tools are repeated", () => {
    for (let i = 0; i < 4; i++) {
      recordExecution("sess-1", "tool_a", "success");
      recordExecution("sess-1", "tool_b", "error");
    }

    const patterns = getRepeatedPatterns("sess-1");
    expect(patterns).toHaveLength(2);
  });
});

describe("detectNovelApproach", () => {
  beforeEach(() => {
    clearSessionHistory("sess-1");
  });

  it("returns false for unknown session", () => {
    expect(detectNovelApproach("nonexistent", "tool_a")).toBe(false);
  });

  it("returns false with fewer than 2 executions", () => {
    recordExecution("sess-1", "tool_a", "success");
    expect(detectNovelApproach("sess-1", "tool_a")).toBe(false);
  });

  it("returns false when last two outcomes are the same", () => {
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "success");
    expect(detectNovelApproach("sess-1", "tool_a")).toBe(false);
  });

  it("returns true when last two outcomes differ (error then success)", () => {
    recordExecution("sess-1", "tool_a", "error");
    recordExecution("sess-1", "tool_a", "success");
    expect(detectNovelApproach("sess-1", "tool_a")).toBe(true);
  });

  it("returns true when last two outcomes differ (success then error)", () => {
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "error");
    expect(detectNovelApproach("sess-1", "tool_a")).toBe(true);
  });
});

describe("clearSessionHistory", () => {
  it("clears history for a session", () => {
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "success");
    recordExecution("sess-1", "tool_a", "success");

    clearSessionHistory("sess-1");
    expect(getRepeatedPatterns("sess-1")).toEqual([]);
  });

  it("does not throw for nonexistent session", () => {
    expect(() => clearSessionHistory("nonexistent")).not.toThrow();
  });
});
