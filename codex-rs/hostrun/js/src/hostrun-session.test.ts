import { describe, expect, it } from "vitest";
import { HostrunSession } from "./hostrun-session.js";

describe("HostrunSession", () => {
  it("keeps live ctx objects across evaluations", async () => {
    const session = await HostrunSession.create();
    try {
      await session.eval("ctx.files = ['a.txt', 'probe.txt'];");

      const result = await session.eval(
        "ctx.probes = ctx.files.filter((file) => file.includes('probe')); ctx.probes.length;",
      );

      expect(result.value).toBe(1);
    } finally {
      session.dispose();
    }
  });

  it("keeps ctx alive after normal thrown exceptions", async () => {
    const session = await HostrunSession.create();
    try {
      await session.eval("ctx.counter = { value: 41 };");

      expect(() => session.evalSync("throw new Error('boom');")).toThrow(
        "boom",
      );

      const result = await session.eval("ctx.counter.value += 1;");

      expect(result.value).toBe(42);
    } finally {
      session.dispose();
    }
  });

  it("closes the session after an execution interrupt", async () => {
    const session = await HostrunSession.create({ interruptCycles: 1 });

    expect(() => session.evalSync("while (true) {}")).toThrow();
    expect(() => session.evalSync("1 + 1")).toThrow(
      "Hostrun session is closed",
    );
  });

  it("returns JSON values from approved capability calls", async () => {
    const session = await HostrunSession.create({
      approve: () => ({ type: "approve" }),
      capabilities: {
        "math.add": {
          describe: (args) => ({
            id: "approval-1",
            tool: "math.add",
            summary: `Add ${args.a} and ${args.b}`,
            args,
          }),
          invoke: (args) => {
            const a = Number(args.a);
            const b = Number(args.b);
            return { sum: a + b };
          },
        },
      },
    });

    try {
      const result = await session.eval("tools.math.add({ a: 2, b: 3 }).sum;");

      expect(result.value).toBe(5);
    } finally {
      session.dispose();
    }
  });

  it("throws denial errors back into sandboxed code", async () => {
    const session = await HostrunSession.create({
      approve: () => ({ type: "deny", reason: "write denied" }),
      capabilities: {
        "fs.write": {
          describe: (args) => ({
            id: "approval-1",
            tool: "fs.write",
            summary: `Write ${args.path}`,
            args,
          }),
          invoke: () => ({ ok: true }),
        },
      },
    });

    try {
      const result = await session.eval(`
        try {
          tools.fs.write({ path: "/tmp/files.txt", content: "data" });
          "not reached";
        } catch (error) {
          error.message;
        }
      `);

      expect(result.value).toBe("write denied");
    } finally {
      session.dispose();
    }
  });

  it("pauses evaluation when approval is pending", async () => {
    const session = await HostrunSession.create({
      approve: () => ({ type: "pending" }),
      capabilities: {
        "fs.write": {
          describe: (args) => ({
            id: "approval-1",
            tool: "fs.write",
            summary: `Write ${args.path}`,
            args,
          }),
          invoke: () => ({ ok: true }),
        },
      },
    });

    try {
      const result = await session.eval(
        `tools.fs.write({ path: "/tmp/files.txt", content: "data" });`,
      );

      expect(result).toEqual({
        type: "needs_approval",
        approval: {
          id: "approval-1",
          tool: "fs.write",
          summary: "Write /tmp/files.txt",
          args: {
            path: "/tmp/files.txt",
            content: "data",
          },
        },
      });
    } finally {
      session.dispose();
    }
  });
});
