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
- [ ] Execute approved `cli.<program>` requests on the host and return exit status plus stdout/stderr handles or captured text.

### Command builder library contract

- [ ] `cli.<program>(...args)` returns a lazy command builder rather than eagerly running the command.
- [ ] `.run()` executes a command builder and returns structured status for each command in the execution graph.
- [ ] `.spawn()` starts a command and returns process/stream handles.
- [ ] `stdout.capture()` and `stderr.capture()` capture bounded text for model-visible results.
- [ ] `stdout.toFile(path)` and `stderr.toFile(path)` redirect output to host files.
- [ ] `stdout.text()` returns captured stdout text.
- [ ] `stdout.lines()` returns captured stdout split into lines.
- [ ] `stdin.text(str)`, `stdin.file(path)`, `stdin.json(value)`, and `stdin.lines(values)` provide explicit stdin sources.
- [ ] A downstream command can pipe from an upstream stream handle, e.g. `cli.cat().stdin(cli.rclone(...).stdout).run()`.
- [ ] Piped command graphs start producer and consumer commands concurrently.
- [ ] Approval text for command graphs includes argv and redirect/pipe shape in a readable form without using a shell internally.
- [ ] Command graph results include every command's exit code and fail the graph if any command fails unless explicitly configured otherwise.

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

- [ ] Implement the command builder API for `cli.<program>` so stdout/stderr redirects and stdin piping are real runtime behavior.
- [ ] Add tests for `stdout.capture`, `stdout.toFile`, `stderr.toFile`, `stdin.text`, `stdin.file`, `stdin.json`, `stdin.lines`, and stream-handle piping.
- [ ] Update contributed Hostrun instructions after the command builder API is implemented, keeping instructions aligned with tested behavior.
- [ ] Add `docs/wiki/systems/hostrun.md` with architecture details once the command builder design stabilizes.

## Out of scope

- Shell compatibility. Hostrun command execution is argv/graph based and should not try to parse arbitrary shell syntax.
- Replacing direct repo-native commands such as `cargo test`, `git`, package managers, deploy scripts, or project CLIs when direct execution is clearer.
- A security sandbox for arbitrary JavaScript libraries. Hostrun host effects must go through explicit, approval-gated capabilities.
