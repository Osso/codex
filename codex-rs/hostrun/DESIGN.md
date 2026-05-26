# Hostrun Design

## Goal

Hostrun is a proposed replacement path for unreadable ad hoc shell snippets in Codex. It should let Codex run host operations through a persistent, scriptable runtime while presenting a human-readable approval summary before side effects happen.

The first target is clarity: make file reads, writes, command execution, remote deletes, secret use, and saved runtime state visible enough that a user can understand what they are approving. Long term, the safer model is a sandboxed runtime by default, with explicit host capabilities that punch out of the sandbox only after approval.

## Shape

Hostrun should feel like a stateful host notebook rather than a stateless shell. A call can initialize live objects and save them in context:

```typescript
import { ctx } from "hostrun";
import { rclone } from "hostrun/cli";

ctx.files = rclone.lsf(
    "spaces:globalcomix-publisher-uploads",
    { recursive: true },
).lines();

ctx.probes = ctx.files.containing("codex-sftpgo-current-probe");
```

A later call can reuse that live context without rerunning the listing:

```typescript
import { ctx, task } from "hostrun";
import { rclone } from "hostrun/cli";

task("Delete leftover SFTPGo probe files", () => {
    for (const file of ctx.probes) {
        rclone.deletefile(`spaces:globalcomix-publisher-uploads/${file}`).run();
    }
});
```

Semantic collection helpers such as `containing`, `nameContains`, `endsWith`, and `matching` are preferred over arbitrary predicates when they make the code and approval summary clearer.

## Approval Model

The runtime should collect intent from library calls and render it before execution. For the second example above, approval should be closer to:

```text
Using live context:
- ctx.files: 12,481 strings from previous rclone lsf
- ctx.probes: 3 strings filtered from ctx.files

Will:
- Run rclone deletefile 3 times
- Delete 3 objects under spaces:globalcomix-publisher-uploads
```

Raw command strings are still possible as a fallback, but common operations should expose structured approval data.

## Sandbox and Capabilities

The long-term model is closer to extending `just-bash` than replacing Bash with raw host Python. User code should run in a constrained runtime with a virtual filesystem and no direct host authority. Host effects should be exposed as explicit capabilities:

```typescript
host.fs.write("/tmp/files.txt", ctx.files.join("\n"));
host.rclone.deletefile(remotePath);
host.run(["sftp", "-b", batch.path, target]);
```

Each capability call can pause synchronously, render a structured approval prompt, and then either execute on the host or throw back into the sandbox.

## Fork Direction

Hostrun should start by forking or upstream-patching `just-bash` rather than building a sandbox from scratch. The parts that align with Hostrun are:

- in-memory and overlay filesystems;
- custom command registry;
- QuickJS-based `js-exec`;
- `javascript.invokeTool`, which exposes a `tools.*` proxy inside sandboxed JavaScript;
- `@just-bash/executor`, which maps tool definitions into both JavaScript calls and bash namespace commands, with approval and elicitation hooks.

The main required change is persistence. Plain `Bash.exec()` intentionally resets shell state per call, and `js-exec` creates and disposes a fresh QuickJS runtime for each execution. Hostrun needs a persistent QuickJS runtime/context per session so `ctx` can hold live objects across calls.

The target API is a session object:

```typescript
const session = new HostrunSession({ fs, invokeTool, executionLimits });

await session.eval("ctx.files = tools.rclone.lsf(...).lines()");
await session.eval("ctx.probes = ctx.files.containing('probe')");
await session.eval("ctx.probes.deleteAll()");
await session.reset();
```

Normal exceptions should not destroy the session. Catastrophic timeout, memory limit, or sandbox integrity failure may kill the session and require reset. Approval waits must be excluded from the normal JavaScript execution timeout, or they will make manual approval unusable.

## Runtime Principles

- Persistent context is a feature, not an accident.
- Live objects may stay in memory between tool calls.
- Objects that perform side effects must provide approval descriptions.
- Large context values should show count, preview, provenance, and hash.
- Secrets must be redacted in logs and approval summaries.
- Commands should be built as argv, not shell strings, unless shell evaluation is explicitly requested.
- The authoring surface should optimize for readable host automation before public ecosystem adoption.

## Open Questions

- Whether the persistent QuickJS work should live in a Codex-owned fork first or be proposed upstream immediately.
- Whether `HostrunSession` should expose bash compatibility at all, or only sandboxed JavaScript plus capabilities.
- How Codex should host and reset the persistent interpreter process.
- How to represent live context values in the TUI without pretending they are fully serializable.
- Which side effects require declaration before execution and which can be discovered dynamically.
- How much CLI wrapper generation is useful before hand-written resource wrappers become clearer.
