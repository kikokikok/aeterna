import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createSessionHook, callWithReauth } from "./session.js";

describe("createSessionHook", () => {
  const originalEnv = process.env;

  beforeEach(() => {
    process.env = { ...originalEnv };
    delete process.env.AETERNA_TOKEN;
  });

  afterEach(() => {
    process.env = originalEnv;
    vi.restoreAllMocks();
  });

  it("starts a backend session on session.start", async () => {
    const client = {
      hasRefreshToken: vi.fn().mockReturnValue(false),
      refreshAuth: vi.fn(),
      sessionStart: vi.fn().mockResolvedValue(undefined),
      sessionEnd: vi.fn().mockResolvedValue(undefined),
    } as const;

    const hook = createSessionHook(client as never, null);
    await hook({ event: { type: "session.start" } as never });

    expect(client.sessionStart).toHaveBeenCalledTimes(1);
    expect(client.refreshAuth).not.toHaveBeenCalled();
  });

  it("refreshes auth before starting session when refresh token exists", async () => {
    const client = {
      hasRefreshToken: vi.fn().mockReturnValue(true),
      refreshAuth: vi.fn().mockResolvedValue(undefined),
      sessionStart: vi.fn().mockResolvedValue(undefined),
      sessionEnd: vi.fn().mockResolvedValue(undefined),
    } as const;

    const hook = createSessionHook(client as never, null);
    await hook({ event: { type: "session.start" } as never });

    expect(client.refreshAuth).toHaveBeenCalledTimes(1);
    expect(client.sessionStart).toHaveBeenCalledTimes(1);
    expect(client.refreshAuth.mock.invocationCallOrder[0]).toBeLessThan(
      client.sessionStart.mock.invocationCallOrder[0]
    );
  });

  it("does not attempt refresh when static token is configured", async () => {
    process.env.AETERNA_TOKEN = "static-token";

    const client = {
      hasRefreshToken: vi.fn().mockReturnValue(true),
      refreshAuth: vi.fn().mockResolvedValue(undefined),
      sessionStart: vi.fn().mockResolvedValue(undefined),
      sessionEnd: vi.fn().mockResolvedValue(undefined),
    } as const;

    const hook = createSessionHook(client as never, null);
    await hook({ event: { type: "session.start" } as never });

    expect(client.refreshAuth).not.toHaveBeenCalled();
    expect(client.sessionStart).toHaveBeenCalledTimes(1);
  });

  it("flushes sync and ends session on session.end", async () => {
    const syncEngine = {
      flushOnShutdown: vi.fn().mockResolvedValue(undefined),
    };
    const client = {
      hasRefreshToken: vi.fn().mockReturnValue(false),
      refreshAuth: vi.fn(),
      sessionStart: vi.fn().mockResolvedValue(undefined),
      sessionEnd: vi.fn().mockResolvedValue(undefined),
    } as const;

    const hook = createSessionHook(client as never, syncEngine as never);
    await hook({ event: { type: "session.end" } as never });

    expect(syncEngine.flushOnShutdown).toHaveBeenCalledTimes(1);
    expect(client.sessionEnd).toHaveBeenCalledTimes(1);
  });
});

describe("plugin startup session ownership", () => {
  const originalEnv = process.env;

  beforeEach(() => {
    process.env = { ...originalEnv, AETERNA_LOCAL_ENABLED: "false" };
  });

  afterEach(() => {
    process.env = originalEnv;
    vi.restoreAllMocks();
    vi.resetModules();
  });

  it("does not start backend session during plugin initialization", async () => {
    const sessionStart = vi.fn().mockResolvedValue(undefined);
    const sessionEnd = vi.fn().mockResolvedValue(undefined);
    const setRouter = vi.fn();
    const setLocalManager = vi.fn();
    const setSyncEngine = vi.fn();
    const setAuthTokens = vi.fn();
    const refreshAuth = vi.fn();
    const requestDeviceCode = vi.fn();
    const pollDeviceToken = vi.fn();
    const bootstrapAuth = vi.fn();

    vi.doMock("../client.js", () => ({
      AeternaClient: vi.fn().mockImplementation(() => ({
        sessionStart,
        sessionEnd,
        setRouter,
        setLocalManager,
        setSyncEngine,
        setAuthTokens,
        refreshAuth,
        requestDeviceCode,
        pollDeviceToken,
        bootstrapAuth,
      })),
    }));
    vi.doMock("../local/config.js", () => ({
      parseLocalConfig: vi.fn(() => ({
        enabled: false,
        db_path: "/tmp/aeterna-test.db",
        sync_push_interval_ms: 30000,
        sync_pull_interval_ms: 60000,
        max_cached_entries: 50000,
        session_storage_ttl_hours: 24,
      })),
    }));
    vi.doMock("../local/manager.js", () => ({
      LocalMemoryManager: vi.fn(),
    }));
    vi.doMock("../local/sync.js", () => ({
      SyncEngine: vi.fn(),
    }));
    vi.doMock("../local/router.js", () => ({
      MemoryRouter: vi.fn(),
    }));

    const { aeterna } = await import("../index.js");
    const hooks = await aeterna({
      project: { id: "proj-1" },
      worktree: "/tmp/proj-1",
      directory: "/tmp/proj-1",
    } as never);

    expect(hooks).toBeDefined();
    expect(sessionStart).not.toHaveBeenCalled();
  });
});

describe("callWithReauth", () => {
  it("still retries after refresh on 401", async () => {
    const client = {
      hasRefreshToken: vi.fn().mockReturnValue(true),
      refreshAuth: vi.fn().mockResolvedValue(undefined),
    };

    let callCount = 0;
    const result = await callWithReauth(client as never, async () => {
      callCount++;
      if (callCount === 1) throw new Error("401 Unauthorized");
      return "ok";
    });

    expect(result).toBe("ok");
    expect(client.refreshAuth).toHaveBeenCalledTimes(1);
  });
});
