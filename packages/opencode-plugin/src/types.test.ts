import { describe, it, expect } from "vitest";
import { DEFAULT_CONFIG } from "./types.js";

describe("DEFAULT_CONFIG", () => {
  it("has capture enabled by default", () => {
    expect(DEFAULT_CONFIG.capture.enabled).toBe(true);
    expect(DEFAULT_CONFIG.capture.sensitivity).toBe("medium");
    expect(DEFAULT_CONFIG.capture.autoPromote).toBe(true);
    expect(DEFAULT_CONFIG.capture.sampleRate).toBe(1.0);
    expect(DEFAULT_CONFIG.capture.debounceMs).toBe(500);
  });

  it("has knowledge injection enabled by default", () => {
    expect(DEFAULT_CONFIG.knowledge.injectionEnabled).toBe(true);
    expect(DEFAULT_CONFIG.knowledge.maxItems).toBe(3);
    expect(DEFAULT_CONFIG.knowledge.threshold).toBe(0.75);
    expect(DEFAULT_CONFIG.knowledge.cacheTtlSeconds).toBe(60);
    expect(DEFAULT_CONFIG.knowledge.timeoutMs).toBe(200);
  });

  it("has governance notifications enabled by default", () => {
    expect(DEFAULT_CONFIG.governance.notifications).toBe(true);
    expect(DEFAULT_CONFIG.governance.driftAlerts).toBe(true);
  });

  it("has session defaults", () => {
    expect(DEFAULT_CONFIG.session.storageTtlHours).toBe(24);
    expect(DEFAULT_CONFIG.session.useRedis).toBe(false);
  });

  it("has experimental features enabled by default", () => {
    expect(DEFAULT_CONFIG.experimental.systemPromptHook).toBe(true);
    expect(DEFAULT_CONFIG.experimental.permissionHook).toBe(true);
  });
});
