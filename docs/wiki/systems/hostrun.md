# Hostrun

Hostrun is an experimental stateful JavaScript runtime for readable host
execution. The reusable runtime and stdio MCP server live in the standalone
`https://github.com/Osso/hostrun` repository, with a local checkout at
`/home/osso/Repos/hostrun`. Codex-specific extension/tool integration lives in
`codex-rs/hostrun-adapter`; when the `hostrun` feature is enabled, that adapter
contributes a `hostrun_eval` tool plus model-facing instructions that describe
the Hostrun standard library.

## Runtime Shape

Hostrun embeds QuickJS through `/home/osso/Repos/hostrun/src/session.rs`. A
`HostrunSessionStore` keeps one persistent JavaScript context per Hostrun
session id, so `globalThis.ctx` survives across later `hostrun_eval` calls and
later assistant turns in the same Codex thread.
The shared eval tool path in `/home/osso/Repos/hostrun/src/eval_tool.rs` owns
`hostrun_eval` argument parsing and session dispatch. The Codex adapter maps
that path into Codex tool APIs and native exec/progress display events; the MCP
server maps it into stdio MCP tool responses and MCP logging/progress
notifications.

The JavaScript standard library is bootstrapped from
`/home/osso/Repos/hostrun/src/bootstrap.js`. It defines public helpers such as `fs`,
`tmp`, `cli`, `rclone`, `fd`, `rg`, `sqlite`, `kubectl`, `http`, `github`, `git`, `path`, string
helpers, array helpers, table/field helpers, and structured data helpers.

Codex's built-in `hostrun_eval` does **not** use the installed
`hostrun-mcp`/`codex-hostrun-mcp` binaries. It links the Rust `hostrun` crate via
`codex-rs/Cargo.toml` and `codex-rs/hostrun-adapter`. If a Hostrun bootstrap
change is needed inside Codex, update Codex's `hostrun` dependency and rebuild
`/home/osso/.cargo/bin/codex`; rebuilding only the standalone MCP binaries leaves
Codex sessions on the old embedded runtime.

As of commit `4cbc3b0d24`, the local Codex fork points `hostrun` at the sibling
`/home/osso/Repos/hostrun` checkout so local Hostrun changes such as
`tools.require('sheetjs')` are picked up by `hostrun_eval`. The adapter regression
test is `executor_loads_sheetjs_through_tools_require`.

## Approval Boundary

JavaScript code does not directly touch the host. Host-facing helpers call the
embedded capability bridge:

- `fs.*` operations are implemented by `fs_capability.rs`.
- `http.*` requests are implemented by `http_capability.rs`.
- CLI commands are approved as `cli.<program>` and executed from `session.rs`.
- Temp resources are tracked by `tmp_capability.rs` and cleaned up when an
  approved session is dropped.

In pending-approval mode, host operations return structured approval requests.
The standalone stdio MCP server uses this mode by default. In auto-approved
Codex tool execution mode, the same request payloads are executed after the
outer tool invocation has passed Codex's approval layer.

## Command Builders

`run.<program>(...args)` executes a host command without stdout/stderr capture
by default. `cli.<program>(...args)` returns a lazy command builder for
workflows that need output capture, stdin, redirects, spawn, or piping.
Arguments stay as argv values rather than shell text. Output handling is
explicit:

- `stdout.text()`, `stdout.lines()`, `stdout.json()`, `stdout.jsonl()`,
`stdout.csv()`, `stdout.tsv()`, `stdout.yaml()`, and `stdout.toml()` execute
the command, then capture and parse bounded stdout. Do not chain `.run()`
after these terminal selectors.
- Builder-level shortcuts such as `text()`, `lines()`, and `json()` default to
  stdout and return the selected stdout value directly, so `cli.ls().text()`
  is the preferred form for stdout text.
- `stdout.toFile(path)` writes full output to a file.
- `stdout.tee(path)` writes full output and keeps bounded captured text visible.
- Matching helpers exist for `stderr` and `combined` where applicable.
- `stderr.toStdout()` merges stderr into stdout.
- `complete()` captures stdout, stderr, exit code, and success status.

Stream piping is represented through command-builder stream handles:

```js
const source = cli.rclone("cat", "spaces:bucket/file.txt");
cli.cat().stdin(source.stdout).stdout.text();
```

The current implementation starts producer and consumer commands concurrently,
then includes a `commands` array with per-command status in graph results.
`.spawn()` returns a managed process handle with `id`, `pid`, stdout/stderr
stream handles, `.wait()`, and `.kill()`.

## HTTP

`http.request(method, url, options)` backs `http.get/post/put/patch/delete/head`.
Options support query params, headers, bearer/basic auth, JSON/form/raw/file
bodies, response text/json/bytes/save handling, timeout, retry count, redirect
policy, TLS `acceptInvalidCerts`, and `throwOnError`.

Sensitive auth fields and headers are redacted from approval metadata before
they are shown to the model.

## GitHub

`tools.github.createPR(options)` is a focused wrapper for `gh pr create`. It
keeps the host boundary at the generic `cli.gh` capability, but always sends the
PR body as stdin through `--body-file -` when a body is provided. This avoids the
common shell failure mode where `--body "line one\nline two"` publishes visible
`\n` text instead of Markdown line breaks.

Use `bodyLines: [...]` or a JavaScript template literal for multiline Markdown.
Literal escaped newline sequences in `body` are rejected by default; callers can
set `allowEscapedNewlines: true` only when the visible `\n` text is intentional.

## Git

`tools.git.commit(options)` is a focused wrapper for `git commit --file -`. It
sends the commit message through stdin so agents do not need shell heredocs,
command substitution, or quote-sensitive `git commit -m` chains for multiline
messages.

Use `subject` or `message` for the first line and `bodyLines: [...]` or a
JavaScript template literal for the body. Literal escaped newline sequences are
rejected by default. `paths`/`files` become pathspecs after `--`; the helper does
not run `git add`, so new files still need to be added explicitly before
committing.

## Specs And Tests

The behavioral spec lives in `docs/specs/hostrun.md`. Most behavior is tested in
`/home/osso/Repos/hostrun/src/*_tests.rs`; command execution tests cover captures,
redirects, stdin sources, stream piping, command graph status, and structured
output parsing.
