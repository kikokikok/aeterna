import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AeternaClient } from "./client.js";
import type { MemoryEntry, SessionContext, KnowledgeSearchResult } from "./types.js";

function mockFetch(response: unknown, options: { ok?: boolean; status?: number } = {}) {
  const { ok = true, status = 200 } = options;
  return vi.fn().mockResolvedValue({
    ok,
    status,
    statusText: ok ? "OK" : "Error",
    json: () => Promise.resolve(response),
  });
}

function createClient(overrides: Partial<{ serverUrl: string; token: string }> = {}) {
  return new AeternaClient({
    project: "test-project",
    directory: "/tmp/test",
    serverUrl: overrides.serverUrl ?? "http://localhost:8080",
    token: overrides.token ?? "test-token",
    team: "test-team",
    org: "test-org",
    userId: "test-user",
  });
}

describe("AeternaClient", () => {
  const originalFetch = globalThis.fetch;

  afterEach(() => {
    globalThis.fetch = originalFetch;
    vi.restoreAllMocks();
  });

  describe("constructor", () => {
    it("uses provided serverUrl and token", () => {
      const client = createClient({ serverUrl: "http://custom:9090", token: "custom-token" });
      expect(client).toBeDefined();
    });

    it("falls back to defaults when no config provided", () => {
      const client = new AeternaClient({
        project: "p",
        directory: "/d",
      });
      expect(client).toBeDefined();
    });
  });

  describe("sessionStart", () => {
    it("creates session on successful server response", async () => {
      const sessionResponse: SessionContext = {
        sessionId: "sess-123",
        project: "test-project",
        team: "test-team",
        org: "test-org",
        userId: "test-user",
        startedAt: "2025-01-01T00:00:00Z",
      };
      globalThis.fetch = mockFetch(sessionResponse);

      const client = createClient();
      const session = await client.sessionStart();

      expect(session.sessionId).toBe("sess-123");
      expect(session.project).toBe("test-project");
    });

    it("creates local session on server error", async () => {
      globalThis.fetch = mockFetch(
        { code: "SERVER_ERROR", message: "unavailable" },
        { ok: false, status: 500 }
      );

      const client = createClient();
      const session = await client.sessionStart();

      expect(session.sessionId).toBeDefined();
      expect(session.project).toBe("test-project");
      expect(session.team).toBe("test-team");
    });

    it("creates local session on network error", async () => {
      globalThis.fetch = vi.fn().mockRejectedValue(new Error("ECONNREFUSED"));

      const client = createClient();
      const session = await client.sessionStart();

      expect(session.sessionId).toBeDefined();
    });
  });

  describe("sessionEnd", () => {
    it("does nothing when no session exists", async () => {
      globalThis.fetch = mockFetch({});
      const client = createClient();
      await client.sessionEnd();
      expect(globalThis.fetch).not.toHaveBeenCalled();
    });

    it("flushes captures and ends session", async () => {
      const fetchMock = mockFetch({});
      globalThis.fetch = fetchMock;

      const client = createClient();
      await client.sessionStart();
      await client.sessionEnd();

      expect(client.getSessionContext()).toBeNull();
    });
  });

  describe("memoryAdd", () => {
    it("adds memory and returns entry", async () => {
      const memoryEntry: MemoryEntry = {
        id: "mem-1",
        content: "test memory",
        layer: "session",
        importance: 0.75,
        tags: ["test"],
        createdAt: "2025-01-01T00:00:00Z",
        updatedAt: "2025-01-01T00:00:00Z",
      };

      let callCount = 0;
      globalThis.fetch = vi.fn().mockImplementation(() => {
        callCount++;
        return Promise.resolve({
          ok: true,
          status: 200,
          statusText: "OK",
          json: () => Promise.resolve(
            callCount <= 2 ? { sessionId: "s1", project: "p", startedAt: "" } : memoryEntry
          ),
        });
      });

      const client = createClient();
      await client.sessionStart();
      const result = await client.memoryAdd({ content: "test memory", layer: "session", tags: ["test"] });

      expect(result.id).toBe("mem-1");
      expect(result.content).toBe("test memory");
    });

    it("throws on server error", async () => {
      let callCount = 0;
      globalThis.fetch = vi.fn().mockImplementation(() => {
        callCount++;
        if (callCount <= 2) {
          return Promise.resolve({
            ok: true,
            json: () => Promise.resolve({ sessionId: "s1", project: "p", startedAt: "" }),
          });
        }
        return Promise.resolve({
          ok: false,
          status: 500,
          statusText: "Error",
          json: () => Promise.resolve({ code: "ERR", message: "server down" }),
        });
      });

      const client = createClient();
      await client.sessionStart();
      await expect(client.memoryAdd({ content: "fail" })).rejects.toThrow("Failed to add memory");
    });
  });

  describe("memorySearch", () => {
    it("returns search results", async () => {
      const searchResults = [
        { memory: { id: "m1", content: "test", layer: "session", importance: 0.5, tags: [], createdAt: "", updatedAt: "" }, score: 0.9 },
      ];

      let callCount = 0;
      globalThis.fetch = vi.fn().mockImplementation(() => {
        callCount++;
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(callCount <= 2 ? { sessionId: "s1", project: "p", startedAt: "" } : searchResults),
        });
      });

      const client = createClient();
      await client.sessionStart();
      const results = await client.memorySearch({ query: "test" });

      expect(results).toHaveLength(1);
      expect(results[0].score).toBe(0.9);
    });
  });

  describe("memoryGet", () => {
    it("returns memory entry on success", async () => {
      const memoryEntry: MemoryEntry = {
        id: "mem-1",
        content: "found",
        layer: "project",
        importance: 0.8,
        tags: [],
        createdAt: "",
        updatedAt: "",
      };
      globalThis.fetch = mockFetch(memoryEntry);

      const client = createClient();
      const result = await client.memoryGet("mem-1");

      expect(result).not.toBeNull();
      expect(result!.id).toBe("mem-1");
    });

    it("returns null on not found", async () => {
      globalThis.fetch = mockFetch(
        { code: "NOT_FOUND", message: "not found" },
        { ok: false, status: 404 }
      );

      const client = createClient();
      const result = await client.memoryGet("nonexistent");

      expect(result).toBeNull();
    });
  });

  describe("knowledgeQuery", () => {
    it("returns results and caches them", async () => {
      const results: KnowledgeSearchResult[] = [
        {
          knowledge: {
            id: "k1",
            type: "adr",
            title: "Use Postgres",
            content: "content",
            scope: "project",
            tags: [],
            createdAt: "",
            updatedAt: "",
            status: "approved",
          },
          score: 0.9,
        },
      ];

      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: () => Promise.resolve(results),
      });

      const client = createClient();
      const firstCall = await client.knowledgeQuery({ query: "postgres" });
      const secondCall = await client.knowledgeQuery({ query: "postgres" });

      expect(firstCall).toHaveLength(1);
      expect(secondCall).toHaveLength(1);
      expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    });

    it("returns empty array on network error with no cache", async () => {
      globalThis.fetch = vi.fn().mockRejectedValue(new Error("network"));

      const client = createClient();
      const results = await client.knowledgeQuery({ query: "anything" });

      expect(results).toEqual([]);
    });
  });

  describe("getSyncStatus", () => {
    it("returns sync status on success", async () => {
      const status = {
        lastSync: "2025-01-01T00:00:00Z",
        pendingPromotions: 2,
        pendingProposals: 1,
        syncHealth: "healthy" as const,
      };
      globalThis.fetch = mockFetch(status);

      const client = createClient();
      const result = await client.getSyncStatus();

      expect(result.syncHealth).toBe("healthy");
      expect(result.pendingPromotions).toBe(2);
    });

    it("returns error status on failure", async () => {
      globalThis.fetch = mockFetch(
        { code: "ERR", message: "unavailable" },
        { ok: false, status: 500 }
      );

      const client = createClient();
      const result = await client.getSyncStatus();

      expect(result.syncHealth).toBe("error");
      expect(result.errors).toContain("unavailable");
    });
  });

  describe("getGovernanceStatus", () => {
    it("returns governance status on success", async () => {
      const status = {
        activePolicies: 5,
        pendingProposals: 0,
        recentViolations: 1,
        driftDetected: false,
        notifications: [],
      };
      globalThis.fetch = mockFetch(status);

      const client = createClient();
      const result = await client.getGovernanceStatus();

      expect(result.activePolicies).toBe(5);
    });

    it("returns empty governance status on failure", async () => {
      globalThis.fetch = mockFetch(
        { code: "ERR", message: "err" },
        { ok: false, status: 500 }
      );

      const client = createClient();
      const result = await client.getGovernanceStatus();

      expect(result.activePolicies).toBe(0);
      expect(result.notifications).toEqual([]);
    });
  });

  describe("detectSignificance", () => {
    it("returns true for aeterna tools", async () => {
      const client = createClient();
      const result = await client.detectSignificance(
        { tool: "aeterna_memory_add" },
        { output: "ok" }
      );
      expect(result).toBe(true);
    });

    it("returns true for long output", async () => {
      const client = createClient();
      const result = await client.detectSignificance(
        { tool: "other" },
        { output: "x".repeat(501) }
      );
      expect(result).toBe(true);
    });

    it("returns false for non-significant operations", async () => {
      const client = createClient();
      const result = await client.detectSignificance(
        { tool: "other" },
        { output: "short" }
      );
      expect(result).toBe(false);
    });
  });

  describe("enrichToolArgs", () => {
    it("adds sessionId to args when session exists", async () => {
      globalThis.fetch = mockFetch({
        sessionId: "sess-abc",
        project: "p",
        startedAt: "",
      });

      const client = createClient();
      await client.sessionStart();

      const enriched = await client.enrichToolArgs("aeterna_memory_add", { content: "test" });
      expect(enriched.sessionId).toBe("sess-abc");
      expect(enriched.content).toBe("test");
    });
  });

  describe("setPluginConfig", () => {
    it("merges partial config", () => {
      const client = createClient();
      client.setPluginConfig({
        capture: {
          enabled: false,
          sensitivity: "low",
          autoPromote: false,
        },
      });
      expect(client).toBeDefined();
    });
  });

  describe("getSessionContext", () => {
    it("returns null before session start", () => {
      const client = createClient();
      expect(client.getSessionContext()).toBeNull();
    });

    it("returns context after session start", async () => {
      globalThis.fetch = mockFetch({
        sessionId: "sess-x",
        project: "p",
        startedAt: "2025-01-01T00:00:00Z",
      });

      const client = createClient();
      await client.sessionStart();

      const ctx = client.getSessionContext();
      expect(ctx).not.toBeNull();
      expect(ctx!.sessionId).toBe("sess-x");
    });
  });

  describe("contextAssemble", () => {
    it("returns fallback on server error", async () => {
      globalThis.fetch = mockFetch(
        { code: "ERR", message: "err" },
        { ok: false, status: 500 }
      );

      const client = createClient();
      const result = await client.contextAssemble({ query: "test" });

      expect(result.context).toBe("");
      expect(result.tokensUsed).toBe(0);
      expect(result.tokenBudget).toBe(8000);
      expect(result.truncated).toBe(false);
    });
  });

  describe("metaLoopStatus", () => {
    it("returns idle status on server error", async () => {
      globalThis.fetch = mockFetch(
        { code: "ERR", message: "err" },
        { ok: false, status: 500 }
      );

      const client = createClient();
      const result = await client.metaLoopStatus();

      expect(result.phase).toBe("idle");
      expect(result.iteration).toBe(0);
    });
  });

  describe("captureToolExecution", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("debounces captures and flushes after timeout", async () => {
      globalThis.fetch = mockFetch({});

      const client = createClient();
      client.setPluginConfig({
        capture: { enabled: true, sensitivity: "medium", autoPromote: true, debounceMs: 100 },
      });

      await client.captureToolExecution({
        tool: "test_tool",
        sessionId: "s1",
        callId: "c1",
        timestamp: Date.now(),
        success: true,
      });

      expect(globalThis.fetch).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(150);

      expect(globalThis.fetch).toHaveBeenCalled();
    });

    it("does nothing when capture is disabled", async () => {
      globalThis.fetch = mockFetch({});

      const client = createClient();
      client.setPluginConfig({
        capture: { enabled: false, sensitivity: "medium", autoPromote: false },
      });

      await client.captureToolExecution({
        tool: "test_tool",
        sessionId: "s1",
        callId: "c1",
        timestamp: Date.now(),
        success: true,
      });

      await vi.advanceTimersByTimeAsync(1000);

      expect(globalThis.fetch).not.toHaveBeenCalled();
    });
  });
});
