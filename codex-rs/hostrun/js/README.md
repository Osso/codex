# Codex Hostrun JS

This package is the Codex-owned fork path for Hostrun's sandboxed JavaScript runtime.

The first exported API is `HostrunSession`, a persistent QuickJS session that keeps `globalThis.ctx` live across evaluations. Normal JavaScript exceptions do not clear session state. Fatal QuickJS failures, such as execution interrupts, close the session so later evaluations fail fast.

## Commands

Run from the repository root:

```bash
npx pnpm --filter @openai/codex-hostrun-js test
npx pnpm --filter @openai/codex-hostrun-js typecheck
```

## Current Scope

- Persistent QuickJS runtime/context per session.
- Live `ctx` object shared across evaluations.
- Session closes after execution interrupts.
- Approval-gated host capabilities exposed as `tools.*`.
- Capability calls can return JSON, throw denial errors into sandboxed code, or pause evaluation with a structured approval request.

## Next Scope

- Codex tool integration that renders approval prompts from capability requests.
