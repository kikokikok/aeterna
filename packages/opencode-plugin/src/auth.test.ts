/**
 * Plugin-side auth lifecycle tests
 *
 * Covers:
 * - requestDeviceCode: sends client_id+scope to GitHub, returns device payload
 * - requestDeviceCode: throws on GitHub error
 * - pollDeviceToken: resolves when access_token arrives
 * - pollDeviceToken: honours slow_down and authorization_pending
 * - pollDeviceToken: throws on unrecoverable error
 * - pollDeviceToken: throws on expiry
 * - bootstrapAuth: sends github_access_token, stores Aeterna tokens
 * - bootstrapAuth: throws on server error
 * - refreshAuth: rotates tokens
 * - refreshAuth: clears refresh token and throws when server rejects
 * - refreshAuth: throws immediately when no refresh token held
 * - logoutAuth: sends revocation request and clears local state
 * - logoutAuth: no-op when no refresh token held
 * - setAuthTokens / getAccessToken / hasRefreshToken
 * - Static token precedence preserved
 * - callWithReauth retries after 401
 */

import { describe, it, expect, vi, afterEach } from "vitest";
import { AeternaClient } from "./client.js";
import { callWithReauth } from "./hooks/session.js";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createClient(token?: string) {
  return new AeternaClient({
    project: "auth-test",
    directory: "/tmp/auth-test",
    serverUrl: "http://localhost:18080",
    token,
  });
}

function bootstrapResponse(overrides?: object) {
  return {
    access_token: "aeterna.access.jwt",
    refresh_token: "refresh-uuid-1234",
    expires_in: 3600,
    github_login: "octocat",
    github_email: "octocat@github.com",
    ...overrides,
  };
}

function refreshResponse(overrides?: object) {
  return {
    access_token: "aeterna.access.jwt.v2",
    refresh_token: "refresh-uuid-5678",
    expires_in: 3600,
    github_login: "octocat",
    github_email: "octocat@github.com",
    ...overrides,
  };
}

function deviceCodeResponse(overrides?: object) {
  return {
    device_code: "dc-abc-123",
    user_code: "ABCD-1234",
    verification_uri: "https://github.com/login/device",
    expires_in: 900,
    interval: 5,
    ...overrides,
  };
}

const originalFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = originalFetch;
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// requestDeviceCode
// ---------------------------------------------------------------------------

describe("AeternaClient.requestDeviceCode", () => {
  it("sends client_id and scope to GitHub device/code endpoint", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(deviceCodeResponse()),
    });
    globalThis.fetch = fetchMock;

    const client = createClient();
    const resp = await client.requestDeviceCode("my-client-id", "read:user user:email");

    expect(resp.device_code).toBe("dc-abc-123");
    expect(resp.user_code).toBe("ABCD-1234");
    expect(resp.verification_uri).toBe("https://github.com/login/device");
    expect(resp.interval).toBe(5);

    const [url, opts] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("https://github.com/login/device/code");
    expect(opts.method).toBe("POST");
    expect(opts.headers).toMatchObject({
      Accept: "application/json",
      "Content-Type": "application/x-www-form-urlencoded",
    });
    const body = new URLSearchParams(opts.body as string);
    expect(body.get("client_id")).toBe("my-client-id");
    expect(body.get("scope")).toBe("read:user user:email");
  });

  it("uses default scope when none provided", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(deviceCodeResponse()),
    });
    globalThis.fetch = fetchMock;

    const client = createClient();
    await client.requestDeviceCode("cid");

    const body = new URLSearchParams((fetchMock.mock.calls[0] as [string, RequestInit])[1].body as string);
    expect(body.get("scope")).toBe("read:user user:email");
  });

  it("throws on non-ok response from GitHub", async () => {
    globalThis.fetch = vi.fn().mockResolvedValueOnce({
      ok: false,
      status: 422,
      json: () => Promise.resolve({ error: "unsupported", message: "client_id invalid" }),
    });

    const client = createClient();
    await expect(client.requestDeviceCode("bad-id")).rejects.toThrow("Device code request failed");
  });
});

// ---------------------------------------------------------------------------
// pollDeviceToken
// ---------------------------------------------------------------------------

describe("AeternaClient.pollDeviceToken", () => {
  it("resolves with access_token when GitHub returns one", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ error: "authorization_pending" }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ access_token: "gho_ghtoken123" }),
      });
    globalThis.fetch = fetchMock;

    const client = createClient();
    const token = await client.pollDeviceToken("cid", "dc-abc", 0.01, 30);

    expect(token).toBe("gho_ghtoken123");
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });

  it("increases interval on slow_down response", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ error: "slow_down", interval: 0.02 }),
      })
      .mockResolvedValueOnce({
        ok: true,
        json: () => Promise.resolve({ access_token: "gho_tok" }),
      });
    globalThis.fetch = fetchMock;

    const client = createClient();
    const token = await client.pollDeviceToken("cid", "dc", 0.01, 30);
    expect(token).toBe("gho_tok");
  });

  it("throws on unrecoverable error from GitHub", async () => {
    globalThis.fetch = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ error: "access_denied" }),
    });

    const client = createClient();
    await expect(
      client.pollDeviceToken("cid", "dc", 0.01, 30)
    ).rejects.toThrow("Device token polling failed: access_denied");
  });

  it("throws when device code expires", async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      json: () => Promise.resolve({ error: "authorization_pending" }),
    });

    const client = createClient();
    await expect(
      client.pollDeviceToken("cid", "dc", 0.001, 0.01)
    ).rejects.toThrow("Device code expired");
  });
});

// ---------------------------------------------------------------------------
// bootstrapAuth (device-flow: sends github_access_token)
// ---------------------------------------------------------------------------

describe("AeternaClient.bootstrapAuth", () => {
  it("sends github_access_token and stores Aeterna tokens", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(bootstrapResponse()),
    });
    globalThis.fetch = fetchMock;

    const client = createClient();
    expect(client.getAccessToken()).toBe("");
    expect(client.hasRefreshToken()).toBe(false);

    const tokens = await client.bootstrapAuth("gho_ghtoken123");

    expect(tokens.accessToken).toBe("aeterna.access.jwt");
    expect(tokens.refreshToken).toBe("refresh-uuid-1234");
    expect(tokens.expiresIn).toBe(3600);
    expect(tokens.githubLogin).toBe("octocat");
    expect(tokens.githubEmail).toBe("octocat@github.com");

    expect(client.getAccessToken()).toBe("aeterna.access.jwt");
    expect(client.hasRefreshToken()).toBe(true);

    const [url, opts] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("http://localhost:18080/api/v1/auth/plugin/bootstrap");
    expect(opts.method).toBe("POST");

    const body = JSON.parse(opts.body as string);
    expect(body.provider).toBe("github");
    expect(body.github_access_token).toBe("gho_ghtoken123");
    expect(body).not.toHaveProperty("code");
    expect(body).not.toHaveProperty("redirect_uri");
  });

  it("throws on non-ok response and leaves tokens unchanged", async () => {
    globalThis.fetch = vi.fn().mockResolvedValueOnce({
      ok: false,
      status: 401,
      json: () => Promise.resolve({ error: "github_exchange_failed", message: "bad_token" }),
    });

    const client = createClient();
    await expect(client.bootstrapAuth("bad-gh-token")).rejects.toThrow("Plugin auth bootstrap failed");

    expect(client.getAccessToken()).toBe("");
    expect(client.hasRefreshToken()).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// refreshAuth
// ---------------------------------------------------------------------------

describe("AeternaClient.refreshAuth", () => {
  it("exchanges refresh token for new pair and rotates tokens", async () => {
    globalThis.fetch = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(bootstrapResponse()) })
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(refreshResponse()) });

    const client = createClient();
    await client.bootstrapAuth("gho_tok");

    const tokens = await client.refreshAuth();

    expect(tokens.accessToken).toBe("aeterna.access.jwt.v2");
    expect(tokens.refreshToken).toBe("refresh-uuid-5678");
    expect(client.getAccessToken()).toBe("aeterna.access.jwt.v2");
    expect(client.hasRefreshToken()).toBe(true);
  });

  it("sends the current refresh token in the request body", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(bootstrapResponse()) })
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(refreshResponse()) });
    globalThis.fetch = fetchMock;

    const client = createClient();
    await client.bootstrapAuth("gho_tok");
    await client.refreshAuth();

    const [url, opts] = fetchMock.mock.calls[1] as [string, RequestInit];
    expect(url).toBe("http://localhost:18080/api/v1/auth/plugin/refresh");
    const body = JSON.parse(opts.body as string);
    expect(body.refresh_token).toBe("refresh-uuid-1234");
  });

  it("clears refresh token and throws when server rejects", async () => {
    globalThis.fetch = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(bootstrapResponse()) })
      .mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: "invalid_refresh_token", message: "expired" }),
      });

    const client = createClient();
    await client.bootstrapAuth("gho_tok");

    await expect(client.refreshAuth()).rejects.toThrow("Plugin auth refresh failed");

    expect(client.hasRefreshToken()).toBe(false);
    expect(client.getAccessToken()).toBe("aeterna.access.jwt");
  });

  it("throws immediately when no refresh token is held", async () => {
    const client = createClient();
    await expect(client.refreshAuth()).rejects.toThrow("No refresh token available");
  });
});

// ---------------------------------------------------------------------------
// logoutAuth
// ---------------------------------------------------------------------------

describe("AeternaClient.logoutAuth", () => {
  it("sends revocation request and clears local auth state", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(bootstrapResponse()) })
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve({ message: "Logged out successfully" }) });
    globalThis.fetch = fetchMock;

    const client = createClient();
    await client.bootstrapAuth("gho_tok");

    await client.logoutAuth();

    const [url, opts] = fetchMock.mock.calls[1] as [string, RequestInit];
    expect(url).toBe("http://localhost:18080/api/v1/auth/plugin/logout");
    const body = JSON.parse(opts.body as string);
    expect(body.refresh_token).toBe("refresh-uuid-1234");

    expect(client.getAccessToken()).toBe("");
    expect(client.hasRefreshToken()).toBe(false);
  });

  it("is a no-op when no refresh token is held (no fetch call)", async () => {
    const fetchMock = vi.fn();
    globalThis.fetch = fetchMock;

    const client = createClient();
    await client.logoutAuth();

    expect(fetchMock).not.toHaveBeenCalled();
    expect(client.getAccessToken()).toBe("");
  });

  it("still clears local state even when logout network request fails", async () => {
    globalThis.fetch = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(bootstrapResponse()) })
      .mockRejectedValueOnce(new Error("network error"));

    const client = createClient();
    await client.bootstrapAuth("gho_tok");

    await expect(client.logoutAuth()).resolves.toBeUndefined();
    expect(client.hasRefreshToken()).toBe(false);
    expect(client.getAccessToken()).toBe("");
  });
});

// ---------------------------------------------------------------------------
// setAuthTokens / getAccessToken / hasRefreshToken
// ---------------------------------------------------------------------------

describe("AeternaClient token state accessors", () => {
  it("getAccessToken returns empty string for fresh client with no static token", () => {
    const client = createClient();
    expect(client.getAccessToken()).toBe("");
  });

  it("getAccessToken returns static token when constructed with one", () => {
    const client = createClient("static-tok");
    expect(client.getAccessToken()).toBe("static-tok");
  });

  it("hasRefreshToken is false for fresh client", () => {
    const client = createClient();
    expect(client.hasRefreshToken()).toBe(false);
  });

  it("setAuthTokens injects an external token pair", () => {
    const client = createClient();
    client.setAuthTokens("my-access", "my-refresh");
    expect(client.getAccessToken()).toBe("my-access");
    expect(client.hasRefreshToken()).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Static token precedence
// ---------------------------------------------------------------------------

describe("Static token precedence", () => {
  it("static AETERNA_TOKEN is used as-is and needs no auth flow", () => {
    const client = createClient("static-aeterna-token");
    expect(client.getAccessToken()).toBe("static-aeterna-token");
    expect(client.hasRefreshToken()).toBe(false);
  });

  it("dynamic token replaces static token after bootstrapAuth", async () => {
    const fetchMock = vi.fn().mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve(bootstrapResponse()),
    });
    globalThis.fetch = fetchMock;

    const client = createClient("old-static-token");
    expect(client.getAccessToken()).toBe("old-static-token");

    await client.bootstrapAuth("gho_tok");
    expect(client.getAccessToken()).toBe("aeterna.access.jwt");
  });
});

// ---------------------------------------------------------------------------
// callWithReauth helper
// ---------------------------------------------------------------------------

describe("callWithReauth", () => {
  it("returns result directly when no auth error occurs", async () => {
    const client = createClient();
    const result = await callWithReauth(client, async () => "success");
    expect(result).toBe("success");
  });

  it("refreshes token and retries on 401 error", async () => {
    const fetchMock = vi.fn()
      .mockResolvedValueOnce({ ok: true, json: () => Promise.resolve(refreshResponse()) });
    globalThis.fetch = fetchMock;

    const client = createClient();
    client.setAuthTokens("old-access", "valid-refresh");

    let callCount = 0;
    const result = await callWithReauth(client, async () => {
      callCount++;
      if (callCount === 1) throw new Error("401 Unauthorized");
      return "retried-ok";
    });

    expect(result).toBe("retried-ok");
    expect(callCount).toBe(2);
    expect(client.getAccessToken()).toBe("aeterna.access.jwt.v2");
  });

  it("returns null when refresh fails after 401", async () => {
    globalThis.fetch = vi.fn().mockResolvedValueOnce({
      ok: false,
      status: 401,
      json: () => Promise.resolve({ error: "invalid_refresh_token", message: "expired" }),
    });

    const client = createClient();
    client.setAuthTokens("old-access", "expired-refresh");

    const result = await callWithReauth(client, async () => {
      throw new Error("401 Unauthorized");
    });

    expect(result).toBeNull();
    expect(client.hasRefreshToken()).toBe(false);
  });

  it("re-throws non-auth errors without attempting refresh", async () => {
    const client = createClient();

    await expect(
      callWithReauth(client, async () => {
        throw new Error("Internal Server Error 500");
      })
    ).rejects.toThrow("Internal Server Error 500");
  });

  it("re-throws when no refresh token held (cannot recover)", async () => {
    const client = createClient();

    await expect(
      callWithReauth(client, async () => {
        throw new Error("401 Unauthorized");
      })
    ).rejects.toThrow("401 Unauthorized");
  });
});
