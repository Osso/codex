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
- [x] Expose `which(program)` as a lazy command-builder helper for common PATH checks.
- [x] Prefer public `fs.write` and `rclone.deletefile` in contributed instructions instead of their `tools.*` bridge forms.
- [x] Expose `fs.write(path, content)`, `fs.read(path)`, `fs.exists(path)`, and `fs.remove(path)` as approval-gated file helpers.
- [x] `hostrun_eval` executes approved `fs.write`, `fs.read`, `fs.exists`, and `fs.remove` operations after the tool invocation has passed its pre-tool approval layer.
- [x] Expose `fs.write(path, content)` as an approval-gated file-write helper.
- [x] Expose `fs.glob(pattern, options)` as an approval-gated filesystem glob helper with optional file/directory filtering.
- [x] Expose `fs.open(path, options)` as a readable `fs.read` wrapper that parses JSON, JSONL, YAML, CSV, and TSV by extension or explicit format.
- [x] Expose `fs.writeJson(path, value)`, `fs.writeYaml(path, value)`, and `fs.writeCsv(path, rows)` for structured file writes.
- [x] Expose `tmp.file(prefix)` and `tmp.dir(prefix)` with automatic cleanup and explicit `.cleanup()` support.
- [x] Expose `tmp.file(prefix, options)` and `tmp.dir(prefix)` handles with deterministic `/tmp/hostrun-*` paths and approval-gated explicit `.cleanup()`.
- [x] `tmp.file` handles support approval-gated `.write`, `.writeJson`, `.writeYaml`, and `.writeCsv`.
- [x] Approved Hostrun sessions track temp handles and remove existing temp files/dirs when the session is dropped, including after JavaScript evaluation errors.
- [x] Expose `rclone.deletefile(target)` and `rclone.lsf(target, options)` as readable wrappers for common rclone workflows.
- [x] Expose `rclone.deletefile(target)` as an approval-gated rclone delete helper.
- [x] Expose `rclone.lsf(target, options)` as a lazy command-builder wrapper.
- [x] Expose `fd.find`, `fd.files`, and `fd.dirs` as readable wrappers around `fdfind`/`fd`.
- [x] Expose `fd.find`, `fd.files`, and `fd.dirs` as lazy command-builder wrappers.
- [x] Expose `rg.search`, `rg.files`, and `rg.matches` as readable wrappers around ripgrep, including structured match parsing where possible.
- [x] Expose `rg.search`, `rg.files`, and `rg.matches` as lazy command-builder wrappers.
- [x] `rg.files(...).run()` returns matching path strings after approved execution, and `rg.matches(...).run()` parses `rg --json` output into structured match objects.
- [x] Execute approved `cli.<program>` requests on the host and return exit status plus stdout/stderr handles or captured text.

### HTTP client

- [x] Expose `http.request(method, url, options)` as an approval-gated HTTP request builder for common API workflows without curl flags.
- [x] Expose method wrappers: `http.get`, `http.post`, `http.put`, `http.patch`, `http.delete`, and `http.head`.
- [x] Preserve request headers in approval metadata, e.g. `{ headers: { Accept: "application/json" } }`.
- [x] Preserve query params in approval metadata, e.g. `{ query: { q: "hostrun", limit: 20 } }`.
- [x] Validate exactly one body source per request: `json`, `form`, `body`, `file`, or `multipart`.
- [x] `json: value` sends `JSON.stringify(value)` and sets JSON content/accept headers unless overridden.
- [x] `form: object` sends `application/x-www-form-urlencoded`.
- [x] `body: string|bytes` sends the raw request body and leaves content type to the caller.
- [x] `file: path` sends file bytes as the whole request body.
- [x] Preserve `multipart: object` fields and file-part metadata in approval metadata.
- [x] Redact bearer/basic/header-token auth secrets from approval metadata.
- [x] Support timeout, retry policy, redirect policy, and TLS options with readable defaults.
- [x] `throwOnError: true` turns non-2xx HTTP responses into JavaScript evaluation errors.
- [x] Expose response intent helpers `.text()`, `.json()`, `.bytes()`, `.save(path)`, and `.run()` in approval metadata.
- [x] `hostrun_eval` executes approved HTTP requests for common methods with query parameters, headers, bearer/basic auth, JSON/form/raw/file bodies, and response text/json/bytes/save handling.
- [x] Response objects expose `.status`, `.ok`, `.headers`, `.text()`, `.json()`, `.bytes()`, and `.save(path)` after real execution lands.
- [x] `.save(path)` streams the response body to disk and returns metadata including path, status, headers, and byte count after real execution lands.

### Command builder library contract

- [x] `cli.<program>(...args)` returns a lazy command builder rather than eagerly requesting approval.
- [x] Command builder `.run()` preserves the existing approval request shape for `cli.<program>`.
- [x] Command builders include requested stdout/stderr/stdin/combined handling in approval metadata.
- [x] `.run()` executes a command builder and returns structured status for each command in the execution graph.
- [x] `.complete()` runs a single command and captures stdout, stderr, exit code, and success status without hiding nonzero exits.
- [x] The internal approved execution path can run scalar-argv commands and return `{ program, args, exitCode, success }`.
- [x] `hostrun_eval` uses the approved execution path for `cli.*` after the tool invocation has passed its pre-tool approval layer.
- [x] `.spawn()` starts a command and returns process/stream handles.
- [x] Spawned process handles expose `id`, `pid`, `stdout`, `stderr`, `.wait()`, and `.kill()`.
- [x] Spawned processes are tracked per Hostrun session and cleaned up when the session drops.
- [x] `stdout.capture()` and `stderr.capture()` capture bounded text for model-visible results.
- [x] `stdout.toFile(path)` and `stderr.toFile(path)` redirect output to host files.
- [x] `stdout.tee(path)`, `stderr.tee(path)`, and `combined.tee(path)` write full output to a file while keeping bounded captured text visible in the result.
- [x] `stderr.toStdout()`, `combined.capture()`, and `combined.toFile(path)` support common stderr/stdout composition.
- [x] `stdout.text()` returns captured stdout text.
- [x] `stdout.lines()` returns captured stdout split into lines.
- [x] `stdin.text(str)`, `stdin.file(path)`, `stdin.json(value)`, `stdin.yaml(value)`, `stdin.csv(rows)`, and `stdin.lines(values)` provide explicit stdin sources.
- [x] The approved `cli.*` execution path supports stdout text, stdout lines, stdout file redirects, stderr text, combined capture, stderr-to-stdout composition, line-based stdin, and structured JSON/YAML/CSV/JSONL stdin.
- [x] Command builders preserve explicit `stdin.yaml`, `stdin.csv`, `stdin.tsv`, `stdin.jsonLines`, and `stdin.jsonl` metadata in approval requests.
- [x] The approved `cli.*` execution path can feed a downstream command from an upstream command's stdout or stderr stream handle.
- [x] A downstream command can pipe from an upstream stream handle, e.g. `cli.cat().stdin(cli.rclone(...).stdout).run()`.
- [x] Named upstream command handles can be reused for piping, e.g. `const result = cli.rclone(...); cli.cat().stdin(result.stdout).run()`.
- [x] A downstream command can pipe either upstream stdout or upstream stderr into stdin.
- [x] Piped command graphs start producer and consumer commands concurrently.
- [x] Approval text for command graphs includes argv and redirect/pipe shape in a readable form without using a shell internally.
- [x] Command graph results include every command's exit code and fail the graph if any command fails unless explicitly configured otherwise.
- [x] Captured stdout/stderr/combined output have bounded size and explicit `{bytes, capturedBytes, truncated}` metadata.

### Structured data and collections

- [x] Keep JSON manipulation deliberately small: native `JSON.parse` / `JSON.stringify`, `.stdout.json()`, `str.json()`, HTTP response `.json()`, and JSON stdin/file serialization.
- [x] Provide string helpers for `str.json()` and `str.jsonLines()`.
- [x] Support JSONL, YAML, and CSV parsing from command output and strings.
- [x] Support JSON, JSONL, CSV, and TSV parsing from command stdout/stderr/combined output.
- [x] Support JSONL, YAML, and CSV serialization to stdin and files.
- [x] Support `str.jsonl()` as an alias for JSONL parsing.
- [x] Support CSV and TSV parsing from strings with `str.csv()` and `str.tsv()`.
- [x] Support TSV and JSONL serialization to files with `fs.writeTsv`, `fs.writeJsonLines`, and `fs.writeJsonl`.
- [x] Support conversion helpers between JSON-compatible values, YAML, CSV, JSONL, arrays, and table objects.
- [x] Provide object/table projection helpers for common `nu`/`jq` workflows: `get`, `select`, `reject`, `rename`, `insert`, `update`, `merge`, `columns`, `values`, and entry iteration.
- [x] Object/table projection helpers are non-mutating and support dotted paths for nested field access.
- [x] Provide collection cleanup/shape helpers: `flatten`, `compact`, `default`, `wrap`, `transpose`, and `enumerate`.
- [x] Provide predicates and reducers: `isEmpty`, `isNotEmpty`, `any`, `all`, `sum`, `avg`, `min`, `max`, and `round`.
- [x] Array `any` and `all` support truthiness, exact-value matching, and callback predicates.
- [x] Arrays provide generic `groupBy`, `countBy`, `uniqueBy`, and `sortBy` helpers for object rows and projected values.
- [x] Provide text helpers for common shell replacements: `splitRow`, `splitColumn`, `splitWords`, `joinText`, `trimmed`, `replaceText`, `lineCount`, `head`, and `tail`.
- [x] Arrays provide `head`, `tail`, and `joinText` helpers for line-list workflows.
- [x] Provide path helpers for common filesystem text transforms: `path.join`, `path.basename`, `path.dirname`, and `path.parse`.
- [x] Provide byte helpers for binary inspection: UTF-8 byte arrays, byte length, and byte ranges over strings and byte arrays.
- [x] Provide numeric byte decoding helpers for common binary inspection: `u16le`, `u16be`, `u32le`, `u32be`, `i32le`, and `i32be`.
- [x] Provide date helpers for common shell replacements: `date.now`, `date.parse`, `date.format`, and `date.humanize`.
- [x] Provide non-mutating string-array helpers: `containing`, `notContaining`, `startsWith`, `endsWith`, `matching`, `notMatching`, `glob`, `notGlob`, `first`, `last`, `take`, `unique`, `sorted`, `reversed`, `lengths`, `bytes`, `lower`, and `upper`.
- [x] Provide string-array helpers for `containing`, `notContaining`, `startsWith`, `endsWith`, `matching`, `notMatching`, `glob`, `notGlob`, `first`, `last`, `take`, `unique`, `lengths`, `bytes`, `lower`, `upper`, `sorted`, and `reversed`.
- [x] `glob` and `notGlob` use case-sensitive path-glob matching with `*`, `?`, and `**`, without shell expansion.
- [x] Provide scalar helpers where they improve agent readability: `lines`, `bytes`, `lower`, `upper`, and `chars`; use JavaScript's native `.length` property for string length.
- [x] Provide scalar helpers for `lines`, `bytes`, `lower`, `upper`, and `chars`.
- [x] `str.lines(start, end)` returns 1-based inclusive line ranges for sed-style line selection.
- [x] Arrays provide `.lineRange(start, end)` for 1-based inclusive ranges over existing line arrays.
- [x] Provide whitespace field parsing with 1-based fields: `lines.fields(separator = /\s+/)`.
- [x] Provide template formatting for field rows: `lines.fields().format("user:{1} prefix:{3|substr:0,7}")`.
- [x] Provide object template formatting for field rows: `lines.fields().format({ user: "{1}", prefix: "{3|substr:0,7}" })`.
- [x] Template transforms include `trim`, `lower`, `upper`, `substr`, `replace`, `basename`, and `dirname`.
- [x] Provide whitespace field parsing, `field(n)`, string template formatting, object template formatting, and template transforms.
- [x] Table helpers include `groupBy`, `sortBy`, `uniqueBy`, and `countBy`; selectors accept 1-based field numbers and field templates with transforms.
- [x] `groupBy` returns `{ key, rows }`, `countBy` returns `{ key, count }`, and grouped/count/unique helpers preserve first-appearance order.

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
- `codex-rs/hostrun/src/session_tests.rs`:
  - `array_helpers_filter_and_transform_strings_without_mutating`
  - `fields_helper_formats_text_and_object_templates`
  - `fields_helper_groups_counts_uniques_and_sorts_by_selectors`
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
- [ ] Implement public file, temp, rclone, fd, rg, and HTTP helpers with approval-aware host execution; basic `fs.write/read/exists/remove` approvals are already public.
- [ ] Implement the command builder API for `cli.<program>` so stdout/stderr redirects and stdin piping are real runtime behavior.
- [ ] Add tests for stdout/stderr capture, redirects, stderr/stdout composition, stdin sources, stream-handle piping, and command graph approval text.
- [ ] Add tests for HTTP query params, headers, auth redaction, JSON/form/raw/file bodies, response save-to-file, timeout metadata, retry metadata, redirect disabling, non-2xx handling, and multipart metadata/execution.
- [ ] Add tests for JSON/YAML/CSV/JSONL parse/serialize helpers; JSON/JSONL/CSV/TSV string and command-output parsing now have focused tests, while YAML parsing remains open.
- [ ] Add tests for remaining collection and table helpers, including reverse aliases, fd/rg structured output parsing, and error behavior.
- [ ] Add tests for temp resource cleanup on success and failure.
- [ ] Update contributed Hostrun instructions after the command builder API is implemented, keeping instructions aligned with tested behavior.
- [x] Add `docs/wiki/systems/hostrun.md` with architecture details once the command builder design stabilizes.

## Out of scope

- Shell compatibility. Hostrun command execution is argv/graph based and should not try to parse arbitrary shell syntax.
- Replacing direct repo-native commands such as `cargo test`, `git`, package managers, deploy scripts, or project CLIs when direct execution is clearer.
- A security sandbox for arbitrary JavaScript libraries. Hostrun host effects must go through explicit, approval-gated capabilities.
