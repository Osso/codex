# just-bash Persistent Session Spike

## Purpose

Hostrun needs live `ctx` objects to survive across tool calls. Upstream `just-bash` does not currently provide that: `Bash.exec()` resets shell state per call, and `js-exec` creates and disposes a fresh QuickJS runtime/context for each execution.

This spike tested the smallest viable fork direction: add a persistent `HostrunSession` to `just-bash` that keeps one QuickJS runtime/context alive until explicit disposal.

The spike has now been made durable in the Codex repo as package:

```text
codex-rs/hostrun/js
```

## Location Tested

- Repository: `/tmp/just-bash`
- Base: `https://github.com/vercel-labs/just-bash.git`
- Package: `packages/just-bash`

## Patch Shape

Codex durable package:

- `codex-rs/hostrun/js/src/hostrun-session.ts`
- `codex-rs/hostrun/js/src/hostrun-session.test.ts`
- `codex-rs/hostrun/js/src/index.ts`

Original `/tmp/just-bash` spike:

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

export interface HostrunSessionOptions {
  interruptCycles?: number;
  memoryLimitBytes?: number;
}

export class HostrunSessionError extends Error {}

export class HostrunSession {
  static async create(options?: HostrunSessionOptions): Promise<HostrunSession>;
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
- execution interrupts close the session so later evaluations fail fast instead of running against questionable state.

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
npx pnpm --filter @openai/codex-hostrun-js test
npx pnpm --filter @openai/codex-hostrun-js typecheck

# Original /tmp just-bash spike
npx pnpm --filter just-bash exec vitest run src/hostrun-session.test.ts
npx pnpm --filter just-bash typecheck
```

Observed results:

```text
@openai/codex-hostrun-js test
Test Files  1 passed (1)
Tests       3 passed (3)

@openai/codex-hostrun-js typecheck
tsc --noEmit
```

## Caveats

- The spike is not yet wired into the existing `js-exec` command path.
- The spike does not yet expose `tools.*` or the worker bridge.
- Memory-limit failure behavior still needs tests.
- Approval waits still need special handling so user approval time is not counted as JavaScript execution time.
- The durable Codex package is still a focused fork path, not a full just-bash vendor.

## Next Step

Add the Hostrun capability bridge on top of the persistent session, starting with approval-gated JSON tool calls.
