import { describe, expect, it } from "vitest";
import { HostrunRunnerServer, runHostrunRequest } from "./runner.js";

describe("runHostrunRequest", () => {
  it("evaluates code in a Hostrun session", async () => {
    const result = await runHostrunRequest({
      session_id: "session-1",
      code: "ctx.count = 41; ctx.count + 1;",
    });

    expect(result).toEqual({ type: "completed", value: 42 });
  });

  it("returns pending approvals from fixture capabilities", async () => {
    const result = await runHostrunRequest({
      session_id: "session-1",
      code: `tools.rclone.deletefile({ target: "spaces:bucket/probe.txt" });`,
      capabilities: {
        "rclone.deletefile": {
          approval: {
            id: "approval-1",
            summary: "Delete probe object",
          },
          decision: "pending",
          result: { ok: true },
        },
      },
    });

    expect(result).toEqual({
      type: "needs_approval",
      approval: {
        id: "approval-1",
        tool: "rclone.deletefile",
        summary: "Delete probe object",
        args: {
          target: "spaces:bucket/probe.txt",
        },
      },
    });
  });

  it("rejects requests without code", async () => {
    await expect(
      runHostrunRequest({ session_id: "session-1" }),
    ).rejects.toThrow("Hostrun runner request must include code");
  });

  it("keeps ctx alive across JSONL server requests with the same session id", async () => {
    const server = new HostrunRunnerServer();
    try {
      const first = await server.run({
        session_id: "session-1",
        code: "ctx.count = 41; ctx.count;",
      });
      const second = await server.run({
        session_id: "session-1",
        code: "ctx.count += 1; ctx.count;",
      });

      expect(first).toEqual({ type: "completed", value: 41 });
      expect(second).toEqual({ type: "completed", value: 42 });
    } finally {
      server.dispose();
    }
  });
});
