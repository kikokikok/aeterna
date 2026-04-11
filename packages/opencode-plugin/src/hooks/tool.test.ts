import { describe, it, expect, vi, beforeEach } from "vitest";
import { createToolHooks } from "./tool.js";
import { clearSessionHistory, getRepeatedPatterns } from "../utils/detect.js";

describe("createToolHooks", () => {
  beforeEach(() => {
    clearSessionHistory("sess-1");
  });

  it("enriches args for aeterna tools in before hook", async () => {
    const client = {
      enrichToolArgs: vi.fn().mockResolvedValue({ sessionId: "sess-1", extra: true }),
    };

    const hooks = createToolHooks(client as never);
    const output = { args: { query: "test" } };

    await hooks.before(
      { tool: "aeterna_memory_search", sessionID: "sess-1", callID: "c1" },
      output
    );

    expect(client.enrichToolArgs).toHaveBeenCalledWith(
      "aeterna_memory_search",
      expect.objectContaining({ query: "test" })
    );
    expect(output.args).toEqual({ query: "test", sessionId: "sess-1", extra: true });
  });

  it("captures executed args in after hook", async () => {
    const client = {
      getSessionContext: vi.fn().mockReturnValue({ sessionId: "sess-1" }),
      captureToolExecution: vi.fn().mockResolvedValue(undefined),
      flagForPromotion: vi.fn().mockResolvedValue(undefined),
    };

    const hooks = createToolHooks(client as never);
    await hooks.after(
      {
        tool: "custom_tool",
        sessionID: "sess-1",
        callID: "c1",
        args: { file: "src/main.ts", limit: 5 },
      },
      {
        title: "Run custom tool",
        output: "ok",
        metadata: {},
      }
    );

    expect(client.captureToolExecution).toHaveBeenCalledTimes(1);
    expect(client.captureToolExecution).toHaveBeenCalledWith(
      expect.objectContaining({
        args: { file: "src/main.ts", limit: 5 },
      })
    );
  });

  it("records execution history before repeated-pattern detection", async () => {
    const client = {
      getSessionContext: vi.fn().mockReturnValue({ sessionId: "sess-1" }),
      captureToolExecution: vi.fn().mockResolvedValue(undefined),
      flagForPromotion: vi.fn().mockResolvedValue(undefined),
    };

    const hooks = createToolHooks(client as never);

    await hooks.after(
      { tool: "custom_tool", sessionID: "sess-1", callID: "c1", args: {} },
      { title: "run", output: "ok", metadata: {} }
    );
    await hooks.after(
      { tool: "custom_tool", sessionID: "sess-1", callID: "c2", args: {} },
      { title: "run", output: "ok", metadata: {} }
    );
    await hooks.after(
      { tool: "custom_tool", sessionID: "sess-1", callID: "c3", args: {} },
      { title: "run", output: "ok", metadata: {} }
    );

    expect(getRepeatedPatterns("sess-1")).toContain("Repeated custom_tool: 3 times");
    expect(client.flagForPromotion).toHaveBeenCalledTimes(1);
    expect(client.flagForPromotion).toHaveBeenCalledWith("sess-1", "c3");
  });
});
