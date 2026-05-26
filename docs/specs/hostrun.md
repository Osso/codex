# Hostrun

Hostrun is Codex's stateful JavaScript host-execution runtime. Its source lives
in `codex-rs/hostrun`, with app-server registration in `codex-rs/app-server`.
It exposes a `hostrun_eval` tool when the experimental `hostrun` feature is
enabled and contributes the model-visible library instructions for that tool at
thread start. How the runtime is wired internally belongs in
`docs/wiki/systems/hostrun.md`.

## What it must do

### Feature gating and prompt contribution

- [x] Hide `hostrun_eval` unless the experimental `hostrun` feature is enabled.
- [x] Register Hostrun as a thread-start contributor so feature state is captured per thread.
- [x] Register Hostrun as a prompt contributor when enabled.
- [x] Contribute Hostrun library instructions only when the feature is enabled.
- [x] Keep the `hostrun_eval` tool schema minimal: the public input is `code`.
- [x] Keep Hostrun API details out of personal rule files; rule files describe when to prefer Hostrun, while contributed instructions describe the Hostrun library surface.

### Persistent JavaScript session

- [x] Evaluate JavaScript in a persistent QuickJS session.
- [x] Keep `globalThis.ctx` live across evaluations in the same session.
- [x] Keep separate `ctx` state per session id.
- [x] Preserve `ctx` after normal JavaScript exceptions.
- [x] Return the executed code in the result for transcript visibility.
- [x] Capture `console.log`, `console.info`, `console.warn`, `console.error`, and `console.debug` in the result.
- [x] Provide `Array.prototype.containing(needle)` for substring filtering.

### Approval-gated host library

- [x] Expose `tools.fs.write({ path, content })` as an approval-gated host file-write request.
- [x] Expose `tools.rclone.deletefile({ target })` as an approval-gated rclone delete request.
- [x] Expose `cli.<program>(...args)` as an approval-gated host command request.
- [x] Preserve `cli.<program>` arguments as argv-style data rather than shell text.
- [x] Include the command program and arguments in the approval request for `cli.<program>`.
- [x] Prefer public `fs.write` and `rclone.deletefile` in contributed instructions instead of their `tools.*` bridge forms.
- [ ] Expose `fs.write(path, content)`, `fs.read(path)`, `fs.exists(path)`, and `fs.remove(path)` as approval-gated file helpers.
- [x] Expose `fs.write(path, content)` as an approval-gated file-write helper.
- [ ] Expose `fs.writeJson(path, value)`, `fs.writeYaml(path, value)`, and `fs.writeCsv(path, rows)` for structured file writes.
- [ ] Expose `tmp.file(prefix)` and `tmp.dir(prefix)` with automatic cleanup and explicit `.cleanup()` support.
- [ ] Expose `rclone.deletefile(target)` and `rclone.lsf(target, options)` as readable wrappers for common rclone workflows.
- [x] Expose `rclone.deletefile(target)` as an approval-gated rclone delete helper.
- [ ] Expose `fd.find`, `fd.files`, and `fd.dirs` as readable wrappers around `fdfind`/`fd`.
- [ ] Expose `rg.search`, `rg.files`, and `rg.matches` as readable wrappers around ripgrep, including structured match parsing where possible.
- [ ] Execute approved `cli.<program>` requests on the host and return exit status plus stdout/stderr handles or captured text.

### HTTP client

- [ ] Expose `http.request(method, url, options)` as an approval-gated HTTP client for common API workflows without curl flags.
- [ ] Expose method wrappers: `http.get`, `http.post`, `http.put`, `http.patch`, `http.delete`, and `http.head`.
- [ ] Support request headers as an object, e.g. `{ headers: { Accept: "application/json" } }`.
- [ ] Support query params as an object, e.g. `{ query: { q: "hostrun", limit: 20 } }`.
- [ ] Support exactly one body source per request: `json`, `form`, `body`, `file`, or `multipart`.
- [ ] `json: value` sends `JSON.stringify(value)` and sets JSON content/accept headers unless overridden.
- [ ] `form: object` sends `application/x-www-form-urlencoded`.
- [ ] `body: string|bytes` sends the raw request body and leaves content type to the caller.
- [ ] `file: path` sends file bytes as the whole request body.
- [ ] `multipart: object` sends form fields and one or more file parts with optional filename/content-type metadata.
- [ ] Support bearer token, bearer token from environment, basic auth, and header-token auth without exposing secrets in transcript output.
- [ ] Support timeout, retry policy, redirect policy, and TLS options with readable defaults.
- [ ] Response objects expose `.status`, `.ok`, `.headers`, `.text()`, `.json()`, `.bytes()`, and `.save(path)`.
- [ ] `.save(path)` streams the response body to disk and returns metadata including path, status, headers, and byte count.

### Command builder library contract

- [ ] `cli.<program>(...args)` returns a lazy command builder rather than eagerly running the command.
- [ ] `.run()` executes a command builder and returns structured status for each command in the execution graph.
- [ ] `.spawn()` starts a command and returns process/stream handles.
- [ ] `stdout.capture()` and `stderr.capture()` capture bounded text for model-visible results.
- [ ] `stdout.toFile(path)` and `stderr.toFile(path)` redirect output to host files.
- [ ] `stderr.toStdout()`, `combined.capture()`, and `combined.toFile(path)` support common stderr/stdout composition.
- [ ] `stdout.text()` returns captured stdout text.
- [ ] `stdout.lines()` returns captured stdout split into lines.
- [ ] `stdin.text(str)`, `stdin.file(path)`, `stdin.json(value)`, `stdin.yaml(value)`, `stdin.csv(rows)`, and `stdin.lines(values)` provide explicit stdin sources.
- [ ] A downstream command can pipe from an upstream stream handle, e.g. `cli.cat().stdin(cli.rclone(...).stdout).run()`.
- [ ] Named upstream command handles can be reused for piping, e.g. `const result = cli.rclone(...); cli.cat().stdin(result.stdout).run()`.
- [ ] A downstream command can pipe either upstream stdout or upstream stderr into stdin.
- [ ] Piped command graphs start producer and consumer commands concurrently.
- [ ] Approval text for command graphs includes argv and redirect/pipe shape in a readable form without using a shell internally.
- [ ] Command graph results include every command's exit code and fail the graph if any command fails unless explicitly configured otherwise.
- [ ] Captured stdout/stderr have bounded size and explicit truncation metadata.

### Structured data and collections

- [ ] Keep JSON manipulation deliberately small: native `JSON.parse` / `JSON.stringify`, `.stdout.json()`, `str.json()`, HTTP response `.json()`, and JSON stdin/file serialization.
- [ ] Support JSONL, YAML, and CSV parsing from command output and strings.
- [ ] Support JSONL, YAML, and CSV serialization to stdin and files.
- [ ] Support conversion helpers between JSON-compatible values, YAML, CSV, JSONL, arrays, and table objects.
- [ ] Provide non-mutating string-array helpers: `containing`, `notContaining`, `startsWith`, `endsWith`, `matching`, `notMatching`, `glob`, `notGlob`, `first`, `last`, `take`, `unique`, `sort`, `reverse`, `lengths`, `bytes`, `lower`, and `upper`.
- [ ] Provide scalar helpers where they improve agent readability: `lines`, `bytes`, `lower`, `upper`, `length`, and `chars`.
- [ ] Provide whitespace field parsing with 1-based fields: `lines.fields(separator = /\s+/)`.
- [ ] Provide template formatting for field rows: `lines.fields().format("user:{1} prefix:{3|substr:0,7}")`.
- [ ] Provide object template formatting for field rows: `lines.fields().format({ user: "{1}", prefix: "{3|substr:0,7}" })`.
- [ ] Template transforms include `trim`, `lower`, `upper`, `substr`, `replace`, `basename`, and `dirname`.
- [ ] Table helpers include `groupBy`, `sortBy`, `uniqueBy`, and `countBy`.

### Transcript and UX

- [x] Emit exec-style begin/end transcript events for `hostrun_eval`.
- [x] Show the JavaScript code being evaluated in the transcript, similar to how shell commands are shown.
- [x] Default missing `session_id` to the thread's default Hostrun session.
- [x] Keep accepting `session_id` internally for compatibility even though the public schema only requires `code`.

## How it works

- `docs/wiki/systems/hostrun.md` - intended system overview and runtime architecture.
- `codex-rs/hostrun/JUST_BASH_SPIKE.md` - historical research notes from the just-bash fork investigation.

## Implementation inventory

- `codex-rs/hostrun/src/lib.rs` - public Hostrun crate types and re-exports.
- `codex-rs/hostrun/src/session.rs` - embedded QuickJS session, `ctx`, console capture, `tools.*`, and `cli.*` approval request generation.
- `codex-rs/hostrun/src/tool_bundle.rs` - `hostrun_eval` tool schema and executor.
- `codex-rs/hostrun/src/tool_contributor.rs` - feature-gated tool and prompt contribution.
- `codex-rs/core/src/tools/handlers/extension_tools.rs` - transcript event handling for extension tools, including Hostrun eval display.
- `codex-rs/app-server/src/app.rs` - app-server extension registry wiring for the experimental Hostrun feature.
- `codex-rs/hostrun/js/src/hostrun-session.ts` - earlier JavaScript QuickJS session prototype and tests.
- `codex-rs/hostrun/js/src/runner.ts` - earlier JSON/JSONL runner prototype.

## Tests asserting this spec

- `codex-rs/hostrun/src/session.rs`:
  - `keeps_live_ctx_objects_across_evaluations`
  - `keeps_ctx_alive_after_normal_exception`
  - `builds_fs_write_approval_request`
  - `builds_cli_command_approval_request`
  - `preserves_cli_command_arguments`
  - `captures_console_messages_and_echoes_executed_code`
  - `store_keeps_ctx_per_session`
- `codex-rs/hostrun/src/tool_bundle.rs`:
  - `hostrun_eval_tool_spec_accepts_session_id_and_code`
  - `missing_code_returns_model_visible_error`
  - `executor_returns_quickjs_eval_json`
  - `executor_defaults_missing_session_id_to_thread_session`
  - `executor_returns_approval_request_json`
  - `executor_returns_cli_approval_request_json`
  - `executor_captures_console_messages`
- `codex-rs/hostrun/src/tool_contributor.rs`:
  - `contributor_returns_hostrun_eval_bundle`
  - `install_adds_hostrun_tool_contributor`
  - `managed_lifecycle_uses_existing_built_runner`
  - `feature_gated_install_hides_hostrun_when_disabled`
  - `feature_gated_install_contributes_tool_and_instructions_when_enabled`
- `codex-rs/core/src/tools/handlers/extension_tools.rs`:
  - `hostrun_extension_tool_emits_exec_events_with_code`
- `codex-rs/hostrun/js/src/*.test.ts` - prototype tests for persistent `ctx`, approval flow, and fake rclone workflows.

## Known gaps (current cycle)

- [ ] Update contributed Hostrun instructions to use public `fs.*`, `rclone.*`, `fd.*`, `rg.*`, `http.*`, and `cli.*` APIs; `fs.write` and `rclone.deletefile` are already public.
- [ ] Implement public file, temp, rclone, fd, rg, and HTTP helpers with approval-aware host execution.
- [ ] Implement the command builder API for `cli.<program>` so stdout/stderr redirects and stdin piping are real runtime behavior.
- [ ] Add tests for stdout/stderr capture, redirects, stderr/stdout composition, stdin sources, stream-handle piping, and command graph approval text.
- [ ] Add tests for HTTP query params, headers, auth redaction, JSON/form/raw/file/multipart bodies, response save-to-file, timeouts, retries, and non-2xx handling.
- [ ] Add tests for JSON/YAML/CSV/JSONL parse/serialize helpers.
- [ ] Add tests for collection and table helpers, including templates, transforms, grouping, sorting, unique values, reverse order, lengths, bytes, lower, and upper.
- [ ] Add tests for temp resource cleanup on success and failure.
- [ ] Update contributed Hostrun instructions after the command builder API is implemented, keeping instructions aligned with tested behavior.
- [ ] Add `docs/wiki/systems/hostrun.md` with architecture details once the command builder design stabilizes.

## Out of scope

- Shell compatibility. Hostrun command execution is argv/graph based and should not try to parse arbitrary shell syntax.
- Replacing direct repo-native commands such as `cargo test`, `git`, package managers, deploy scripts, or project CLIs when direct execution is clearer.
- A security sandbox for arbitrary JavaScript libraries. Hostrun host effects must go through explicit, approval-gated capabilities.
