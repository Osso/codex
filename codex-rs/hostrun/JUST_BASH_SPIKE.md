# just-bash Persistent Session Spike

## Purpose

Hostrun needs live `ctx` objects to survive across tool calls. Upstream `just-bash` does not currently provide that: `Bash.exec()` resets shell state per call, and `js-exec` creates and disposes a fresh QuickJS runtime/context for each execution.

This spike tested the smallest viable fork direction: add a persistent `HostrunSession` to `just-bash` that keeps one QuickJS runtime/context alive until explicit disposal.

## Location Tested

- Repository: `/tmp/just-bash`
- Base: `https://github.com/vercel-labs/just-bash.git`
- Package: `packages/just-bash`

## Patch Shape

Added:

- `packages/just-bash/src/hostrun-session.ts`
- `packages/just-bash/src/hostrun-session.test.ts`

Updated:

- `packages/just-bash/src/index.ts`

The spike exports:

```typescript
export interface HostrunEvalResult {
  value: unknown;
}

export class HostrunSessionError extends Error {}

export class HostrunSession {
  static async create(): Promise<HostrunSession>;
  evalSync(code: string): HostrunEvalResult;
  eval(code: string): Promise<HostrunEvalResult>;
  dispose(): void;
}
```

The session initializes:

```typescript
globalThis.ctx = Object.create(null);
```

and keeps the same QuickJS runtime/context across `eval` calls. Normal thrown JavaScript errors are converted to `HostrunSessionError` and do not dispose the session.

## Verified Behavior

The spike added tests proving:

- `ctx` values survive across multiple evaluations;
- live objects stored under `ctx` can be mutated in later evaluations;
- normal thrown exceptions do not destroy the session or clear `ctx`.

Representative test:

```typescript
const session = await HostrunSession.create();
await session.eval("ctx.counter = { value: 41 };");

expect(() => session.evalSync("throw new Error('boom');")).toThrow("boom");

const result = await session.eval("ctx.counter.value += 1;");
expect(result.value).toBe(42);
```

## Verification Commands

```bash
npx pnpm install
npx pnpm --filter just-bash exec vitest run src/hostrun-session.test.ts
npx pnpm --filter just-bash typecheck
```

Observed results:

```text
Test Files  1 passed (1)
Tests       2 passed (2)

just-bash@3.0.1 typecheck
tsc --noEmit
```

## Caveats

- The spike is not yet wired into the existing `js-exec` command path.
- The spike does not yet expose `tools.*` or the worker bridge.
- Timeout and memory-limit failure behavior still need tests.
- Approval waits still need special handling so user approval time is not counted as JavaScript execution time.
- The spike currently lives outside the Codex repo; the next implementation step should create a durable Codex-owned fork or vendored package path.

## Next Step

Move from spike to integration by creating a Codex-owned `just-bash` fork path that keeps the persistent session API and then adds the Hostrun capability bridge on top of `invokeTool`.
