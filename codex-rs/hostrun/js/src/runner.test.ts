import { describe, expect, it } from "vitest";
import { runHostrunRequest } from "./runner.js";

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
});
