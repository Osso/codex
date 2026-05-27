# Hostrun

Hostrun is Codex's experimental stateful JavaScript runtime for readable host
execution. When the `hostrun` feature is enabled, Codex contributes a
`hostrun_eval` tool plus model-facing instructions that describe the Hostrun
standard library.

## Runtime Shape

Hostrun embeds QuickJS through `codex-rs/hostrun/src/session.rs`. A
`HostrunSessionStore` keeps one persistent JavaScript context per Hostrun
session id, so `globalThis.ctx` survives across tool calls in the same thread.
The public tool schema exposes only `code`; the default session id is supplied
by the tool executor.

The JavaScript standard library is bootstrapped from
`codex-rs/hostrun/src/bootstrap.js`. It defines public helpers such as `fs`,
`tmp`, `cli`, `rclone`, `fd`, `rg`, `sqlite`, `kubectl`, `http`, `path`, string
helpers, array helpers, table/field helpers, and structured data helpers.

## Approval Boundary

JavaScript code does not directly touch the host. Host-facing helpers call the
embedded capability bridge:

- `fs.*` operations are implemented by `fs_capability.rs`.
- `http.*` requests are implemented by `http_capability.rs`.
- CLI commands are approved as `cli.<program>` and executed from `session.rs`.
- Temp resources are tracked by `tmp_capability.rs` and cleaned up when an
  approved session is dropped.

In pending-approval mode, host operations return structured approval requests.
In auto-approved test/tool execution mode, the same request payloads are
executed after the outer tool invocation has passed Codex's approval layer.

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

## Specs And Tests

The behavioral spec lives in `docs/specs/hostrun.md`. Most behavior is tested in
`codex-rs/hostrun/src/*_tests.rs`; command execution tests cover captures,
redirects, stdin sources, stream piping, command graph status, and structured
output parsing.
