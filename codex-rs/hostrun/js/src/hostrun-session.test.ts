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
});
