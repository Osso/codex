# Osso fork: features built on top of upstream

This document enumerates every behavioral change the Osso fork carries on top of
`origin/main` (upstream `openai/codex`). Use it as a checklist when rebasing
onto a new upstream: each section lists what the feature does, where it lives,
how to exercise it, and what to re-verify after the rebase lands.

Baseline: diff range `origin/main..HEAD`. Regenerate the commit list with
`git log --oneline origin/main..HEAD` when refreshing this doc.

---

## 1. Claude-compatible prompt context hooks

**Purpose.** Let external hook binaries inject context into the model prompt
the same way Claude Code's `SessionStart` / `UserPromptSubmit` / `Stop` hooks
do. Hook stdout returning `{ "hookSpecificOutput": { "additionalContext": "..." } }`
is prepended to the next user message as text content.

**Behavior details worth preserving.**
- Multiple simultaneous hook text blocks are separated by CRLF and rendered as
  distinct content blocks rather than concatenated.
- Hook labels (handler names from `config.toml`) are surfaced in event
  rendering so the UI shows *which* hook produced the text.
- A `Stop` hook that returns `continue=false` does **not** carry its block
  reason forward into the next user prompt.
- When the user steers mid-turn, the pending input is normalized to strip any
  previously prepended hook context before being re-submitted.

**Key files.**
- `codex-rs/hooks/src/events/stop.rs`
- `codex-rs/hooks/src/events/session_start.rs`
- `codex-rs/hooks/src/events/user_prompt_submit.rs`
- `codex-rs/hooks/src/events/common.rs` (shared parsing, apply_patch translation)
- `codex-rs/hooks/src/engine/output_parser.rs`
- `codex-rs/hooks/src/schema.rs`
- `codex-rs/hooks/generated/stop.command.output.schema.json`
- `codex-rs/core/src/session/turn.rs` (prepend helper, CRLF spacer)
- `codex-rs/core/src/state/session.rs`
- `codex-rs/tui/src/history_cell/hook_cell.rs` (handler-name rendering)

**Tests to re-run after rebase.**
- `cargo test -p codex-hooks`
- `cargo test -p codex-core session::tests`
- TUI snapshot: `*_hook_events_render_snapshot.snap`

**Rebase risk.** Upstream is actively refactoring the hooks crate. Watch for:
renames of `additional_context`/`additionalContext` JSON keys; changes to how
`session/turn.rs` assembles user input items (the CRLF separator lives there);
changes to `history_cell/hook_cell.rs` (upstream tends to rewrite the rendering
paths). If upstream adds native Claude-format hook support, reconcile rather
than drop this code.

---

## 2. apply_patch → Claude Write payload translation

**Purpose.** Existing hook binaries (pre/post tool use) expect Claude's `Write`
tool payload shape. Codex emits `apply_patch` instead. The translation layer
detects single-file `apply_patch` inputs in `codex-hooks` and synthesizes the
`{ file_path, content }` shape hook scripts already understand.

**Key files.**
- `codex-rs/hooks/src/events/common.rs` (translation function + matcher inputs)
- `codex-rs/hooks/src/events/pre_tool_use.rs`
- `codex-rs/hooks/src/events/post_tool_use.rs`

**Tests to re-run.**
- `cargo test -p codex-hooks pre_tool_use`
- `cargo test -p codex-hooks post_tool_use`

**Rebase risk.** If upstream changes the `apply_patch` tool input schema (e.g.
moves from `command` string to structured form), the translator must be
updated. Multi-file patches are intentionally not translated — do not silently
"fix" this.

---

## 3. Session-end transcript parser

**Purpose.** Hook binaries that need the transcript path (for post-run
summarization, auditing, etc.) receive it via JSON on stdin. This fork ships a
tested parser plus a thin CLI binary `session_end_transcript_path` that prints
the path so shell hooks can read it with a single `jq`-free invocation.

**Key files.**
- `codex-rs/hooks/src/session_end.rs` (`session_end_transcript_path_from_json`)
- `codex-rs/hooks/src/bin/session_end_transcript_path.rs`
- `codex-rs/hooks/src/lib.rs` (re-export)

**Behavior details.** Parses nested `hook_event.transcript_path` (new format)
*and* falls back to top-level `transcript_path` (legacy). Top-level takes
precedence when both are present.

**Tests to re-run.** `cargo test -p codex-hooks session_end`.

**Rebase risk.** Low — pure additive crate surface. Just make sure the binary
target still resolves in `codex-rs/hooks/Cargo.toml` after any workspace
reorganization.

---

## 4. Stop / session_start hook plumbing

**Purpose.** Supporting infrastructure for section 1: register TOML-defined
stop hooks in the engine, avoid double-running stop hooks, and ignore leaked
stop hooks during after-agent dispatch.

**Key files.**
- `codex-rs/hooks/src/engine/discovery.rs`
- `codex-rs/hooks/src/events/stop.rs`
- `codex-rs/hooks/src/events/common.rs`

**Rebase risk.** Upstream's dispatcher is the main collision point. If a rebase
changes the hook registration/dispatch model, these deduplication guards must
be re-ported.

---

## 5. User rules loader (`$CODEX_HOME/rules/*.md`)

**Purpose.** Mirror of Claude Code's `~/.claude/rules/` directory. Every `.md`
file in `$CODEX_HOME/rules/` is loaded at startup, sorted by filename, joined
with double newlines, and injected into user instructions.

**Key files.**
- `codex-rs/core/src/config/rules.rs` (loader, 82 lines, self-contained)
- `codex-rs/core/src/config/mod.rs` (invocation site — single call)
- `codex-rs/core/src/agents_md.rs` (integration into instruction assembly)

**Tests.** `cargo test -p codex-core config::rules` — covers missing dir, empty
dir, sorted ordering, whitespace trimming.

**Rebase risk.** Low. Watch for upstream adding a competing "global rules"
feature — if so, migrate rather than duplicate.

---

## 6. `/run-plan` slash command

**Purpose.** Walks the checklist in `PLAN.md` (or a user-specified file),
finds the first unchecked `- [ ]` item, and submits it as the next prompt.
Exports `PLAN_FILE=<name>` (or `PLAN_FILE=1` for the default) so downstream
hooks and child processes can honor a non-default plan filename.

**Key files.**
- `codex-rs/tui/src/slash_command.rs` (enum variant)
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` (`dispatch_run_plan`,
  `find_next_plan_item`, `PLAN_FILE` export)

**Behavior details.** Accepts optional inline argument via
`prepare_inline_args_submission`, so the composer clears after dispatch. The
helper matches both `- [ ]` and `* [ ]` unchecked markers. Missing plan files
are reported as command errors instead of surfacing as silent no-op submissions.

**Tests.** TUI unit tests in `slash_dispatch.rs`.

**Rebase risk.** Upstream rewrites the slash-command dispatch regularly. If the
enum dispatch changes shape, re-port `dispatch_run_plan`.

---

## 7. Multi-agent v2 only — v1 removed

**Purpose.** The fork ships only v2 multi-agent tools. v1 was removed
entirely in commit `583ed3144a` (`Stage::Deprecated`, default-off, behind
double-opt-in). See `docs/specs/multi-agent-v2.md` for the contract that
today's tests assert.

**Key files.**
- `codex-rs/core/src/tools/handlers/multi_agents_v2/*`
- `codex-rs/core/src/tools/handlers/multi_agents_common.rs` (shared helpers)
- `codex-rs/tools/src/tool_registry_plan.rs` (registry defaulting)
- `codex-rs/tools/src/tool_config.rs`
- `codex-rs/features/src/lib.rs` (`Feature::MultiAgentV2`,
  `Feature::LegacyMultiAgentV1` — gone)
- `codex-rs/app-server-protocol/src/protocol/v2.rs`
- Generated schemas under `codex-rs/app-server-protocol/schema/json/v2/`

**Tests to re-run.**
- `cargo test -p codex-core tools::handlers::multi_agents_tests`
- `cargo test -p codex-tools tool_registry_plan_tests`
- `cargo test -p codex-app-server suite::v2::turn_start`

**Rebase risk. HIGH.** Upstream still ships v1 alongside v2. After a rebase:
1. Re-delete the v1 directory + handler imports + 5 `ToolHandlerKind::*V1`
   variants + `Feature::LegacyMultiAgentV1`.
2. Verify v2 spawn / send_message / wait / list / close still pass after
   the registry-plan three-way merge.
3. Re-generate schemas if upstream touched protocol/v2.rs.
4. Cross-check `docs/specs/multi-agent-v2.md` "What it must do" bullets
   against the post-rebase test names.

---

## 8. Unified exec is the only public shell tool

**Purpose.** The fork registers only `exec_command` / `write_stdin` (backed
by `UnifiedExecHandler`). The four legacy aliases (`shell`,
`container.exec`, `local_shell`, `shell_command`) and their
`ShellHandler` / `ShellCommandHandler` implementations were removed in
commit `b1c1c6e350`. The deprecated `Feature::LegacyShellCompat` flag is
gone.

**Key files.**
- `codex-rs/core/src/tools/handlers/shell.rs` — **deleted**
- `codex-rs/core/src/tools/handlers/shell_tests.rs` — **deleted**
- `codex-rs/core/src/tools/spec.rs` (only registers UnifiedExec)
- `codex-rs/core/src/tools/runtimes/shell.rs`
- `codex-rs/core/src/tools/runtimes/shell/unix_escalation.rs`
- `codex-rs/core/src/tools/runtimes/shell/zsh_fork_backend.rs`
- `codex-rs/tools/src/tool_registry_plan.rs` (no `legacy_shell_compat` block)
- `codex-rs/core/src/tools/parallel.rs` (parallel-eligibility list narrowed
  to `unified_exec | exec_command | write_stdin`)

**Rebase risk. HIGH.** Upstream still ships the legacy shell aliases.
After a rebase: re-delete `shell.rs` + `shell_tests.rs`, drop
`ToolHandlerKind::{Shell,ShellCommand}` variants, drop
`Feature::LegacyShellCompat`, drop the
`if config.legacy_shell_compat { ... }` block in `tool_registry_plan.rs`,
and trim the parallel-eligibility list back to the three current names.

---

## 9. Subagent / inter-agent communication polish

**Purpose.** Five related improvements to v2 multi-agent UX:

1. **Inter-agent message pretty-printing.** Subagent notifications and
   inter-agent messages arrive as JSON `InterAgentCommunication` blobs embedded
   in assistant messages. The TUI parses them and renders a compact
   `author → recipient` cell instead of raw JSON.
2. **Trigger-turn notifications.** Subagent notifications now mark themselves
   as trigger turns so truncation logic counts them as turn boundaries.
3. **`wait_agent` early return.** If no descendant agents exist, `wait_agent`
   returns immediately ("No agents available yet.") rather than blocking until
   timeout.
4. **Spawn delivery reliability.** Spawned agents receive their initial task
   exactly once, including when the first turn is delivered through queued task
   plumbing rather than an already-running agent loop.
5. **Descendant-aware wait status.** `wait_agent` reports active descendant
   status across nested agents instead of only looking at direct children.

**Key files.**
- `codex-rs/tui/src/inter_agent_message.rs` (new module, self-contained)
- `codex-rs/tui/src/chatwidget/realtime.rs`
- `codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs`
- `codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs`
- `codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs`
- `codex-rs/core/src/session/turn.rs` (trigger-turn propagation)
- `codex-rs/core/src/state/session.rs`

**Tests.**
- `cargo test -p codex-tui inter_agent_message`
- `cargo test -p codex-core tools::handlers::multi_agents_v2`
- `cargo test -p codex-core suite::subagent_notifications`
- Snapshot: `collab_agent_transcript.snap` (currently a `.snap.new` on this
  branch — review and accept before rebase).

**Rebase risk.** Medium. The inter-agent module is isolated and should survive,
but `chatwidget/realtime.rs` and the wait/message handlers collide with
upstream multi-agent work.

---

## 10. TUI customization

**Purpose.** User-facing TUI tweaks.

- **Configurable `strong_color`** for bold markdown text
  (`codex-rs/core/src/config/mod.rs`, `codex-rs/config/src/types.rs` →
  `TuiConfig.strong_color`, applied in `codex-rs/tui/src/lib.rs`).
- **Configurable `code_color`** for inline code spans (same three files;
  includes a shared hex-parsing helper).
- **Overlay navigation remap.** Transcript/pager overlay uses Up/Down for
  navigation, Esc to quit (`codex-rs/tui/src/pager_overlay.rs`,
  `codex-rs/tui/src/app_backtrack.rs`).
- **Transcript overlay height cache.** During esc-esc backtrack, transcript
  row renderables stay alive so wrapped-height calculations remain cached
  (`codex-rs/tui/src/pager_overlay.rs`). Has a regression test.
- **Queued-message edit on Up.** The bottom pane advertises and handles Up as
  the queued-message edit binding instead of the older Shift-Left wording
  (`codex-rs/tui/src/bottom_pane/pending_input_preview.rs`,
  `codex-rs/tui/src/chatwidget.rs`, snapshot updates).
- **Editable pending steers.** Pending steering messages can be recalled with
  Up, edited, and re-submitted with the same `steerId`. Core replaces the
  still-pending input instead of appending both original and edited text
  (`codex-rs/core/src/state/turn.rs`, `codex-rs/core/src/session/mod.rs`,
  `codex-rs/tui/src/chatwidget.rs`, `codex-rs/app-server-protocol/src/protocol/v2/turn.rs`).
- **Suppress queued-turn completion notifications.** Queued edits and steering
  messages do not emit a redundant completion notification when the queued turn
  finishes.
- **Flush queued keyboard events on terminal restore**
  (`codex-rs/tui/src/tui.rs`, 4-line change).
- **`codex_core::Config` import cleanup** (`codex-rs/tui/src/lib.rs`).

**Tests.** TUI snapshot suite under `codex-rs/tui/src/**/snapshots/`. Several
snapshots touched — re-review after rebase.

**Rebase risk.** Medium. Upstream restyles the TUI frequently; the two color
config entries are additive and usually survive, but the overlay changes
touch a file upstream also edits.

---

## 11. App-server & MCP robustness fixes

**Purpose.** Six independent bug fixes:

- **Preserve MCP startup status notifications under backpressure.** When the
  app-server client is slow to drain, startup notifications are queued rather
  than dropped (`codex-rs/app-server-client/src/lib.rs`,
  `codex-rs/app-server-client/src/remote.rs`,
  `codex-rs/app-server/src/in_process.rs`).
- **Reload app-server auth before MCP account reads**
  (`codex-rs/app-server/src/codex_message_processor.rs`; tests in
  `app-server/tests/suite/auth.rs` and `suite/v2/account.rs`).
- **Gate login issuer override to debug builds.** The app-server login issuer
  override is only honored in debug builds so production/release builds do not
  accidentally trust a test issuer (`codex-rs/app-server/src/codex_message_processor.rs`).
- **Reset the TUI status timer for MCP startup rounds.** When MCP startup begins
  while another task is already running, the bottom-pane status row now restarts
  its elapsed timer instead of showing the older task's elapsed time as
  `Booting MCP server: ... (18s · esc to interrupt)`. Direct `regex-replace`
  stdio checks confirmed the server starts and lists tools in ~3-4ms; the
  visible delay was inherited UI state, not MCP startup latency. Key files:
  `codex-rs/tui/src/status_indicator_widget.rs`,
  `codex-rs/tui/src/bottom_pane/mod.rs`,
  `codex-rs/tui/src/chatwidget.rs`, and
  `codex-rs/core/src/telemetry.rs` (timing-only runtime metrics are no longer
  treated as empty).
- **Honor runtime goal feature enablement in app-server goal RPCs.** The TUI
  can enable `/goal` via `/experimental` after app-server startup, so
  `thread/goal/{get,set,clear}` now resolves the latest feature state through
  `ConfigManager` instead of checking the app-server startup `Config` snapshot
  (`codex-rs/app-server/src/request_processors/thread_goal_processor.rs`,
  `codex-rs/app-server/src/message_processor.rs`; regression coverage in
  `codex-rs/app-server/tests/suite/v2/thread_resume.rs`).
- **Forward MCP startup completion summaries through app-server.** Core emits
  `McpStartupComplete` after the startup join-set settles; app-server now
  converts that summary into terminal `mcpServer/startupStatus/updated`
  notifications so the TUI clears `Starting MCP servers ...` even if an
  individual final update was dropped or filtered
  (`codex-rs/app-server/src/bespoke_event_handling.rs`; regression coverage in
  `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs`).

**Tests.**
- `cargo test -p codex-app-server suite::auth suite::v2::account`
- `cargo test -p codex-tui`
- `cargo test -p codex-core runtime_metrics_summary_is_not_empty_when_only_timing_fields_are_set`
- `cargo check -p codex-app-server --lib`

**Rebase risk.** Medium — upstream owns both files heavily. Carry forward as
surgical hunks.

---

## 12. Skip `PWD` in exported shell env

**Purpose.** When exporting the shell environment to child processes, strip
`PWD`. Stale PWD leaks working-directory state from the parent codex process
into spawned shells and confuses path-sensitive tools.

**Key files.**
- `codex-rs/config/src/shell_environment.rs`
- `codex-rs/core/src/exec_env_tests.rs` (regression tests)

**Rebase risk.** Low — 16-line change plus tests.

---

## 13. Worktree startup option

**Purpose.** Add a `-w/--worktree <NAME>` startup option that creates or reuses
a sibling Git worktree before launching TUI or exec mode, so fork work can start
in an isolated checkout without manual `git worktree` setup. New worktrees are
created from `origin/master` and named after the provided value.

**Key files.**
- `codex-rs/utils/cli/src/shared_options.rs`
- `codex-rs/git-utils/src/worktree.rs`
- `codex-rs/cli/src/main.rs`
- `codex-rs/tui/src/lib.rs`
- `codex-rs/exec/src/lib.rs`

**Tests.** `cargo test -p codex-exec worktree`.

**Rebase risk.** Medium. Shared CLI option plumbing and TUI/exec startup are
active upstream surfaces; keep the behavior isolated in `git-utils` and
re-check both binary entry points after merge resolution.

---

## 14. Local deploy script + Osso branding

**Purpose.** Build and install a local fork build with the `-osso` suffix,
independent of upstream release machinery.

**Components.**
- **`deploy.sh`** at repo root: builds `codex-cli` from the Rust workspace with
  `cargo build -p codex-cli --bin codex --release --locked`, then installs the
  resulting binary into `$CODEX_INSTALL_ROOT/bin`.
- **Branding split.** `CODEX_CLI_VERSION` (machine-readable, tracks upstream
  semver) vs `CODEX_CLI_DISPLAY_VERSION` (e.g. `0.131.0-alpha.8-osso`). Update checks
  use the former, UI uses the latter.
  - `codex-rs/tui/src/version.rs`
  - `codex-rs/cli/src/main.rs`
  - `codex-rs/tui/src/update_prompt.rs`
  - `announcement_tip.toml`
- **Release profile tuning** (`codex-rs/Cargo.toml`):
  - LTO disabled for faster local builds.
  - Release codegen units raised to 4.
**Tests.** `cargo test -p codex-tui version`.

**Rebase risk.** Medium. Every upstream version bump will conflict with the
branding split; walk through `version.rs` manually each time.

---

## 15. Model prompt + generated-file hygiene

**Purpose.** Keep the fork's model metadata and generated local artifacts aligned
with Osso workflow expectations.

- **Relax GPT-5.4 `apply_patch` wording.** `codex-rs/models-manager/models.json`
  shortens and relaxes the model-specific `apply_patch` instruction so the model
  is encouraged to use the patch tool without over-constraining every edit path.
- **Generated snapshot ignore.** `.gitignore` ignores generated `*.snap.new`
  files so transient `insta` output does not get mixed into fork commits by
  accident.

**Tests.** Re-run the tests that consume updated model metadata when touching
`models.json`; review pending snapshots directly before accepting them.

**Rebase risk.** Low to medium. Upstream frequently regenerates
`models-manager/models.json`, so inspect the model entry by hand instead of
assuming a JSON merge preserved the intended prompt text.

---

## 16. Permission prompt approval tool

Route shell/exec approval decisions through an MCP-hosted permission prompt
tool so approval policy can come from the user's configured server (e.g.
`claude-bash-hook-approval`) instead of only the built-in TUI approval
surface. Allow decisions can be cached for the session and persisted back
to user, project, or local config.

Shipped in commit `531e371da6`. **Contract: `docs/specs/permission-prompt-tool.md`**
(every "What it must do" bullet maps to a backing test).

**Tests to re-run after rebase.**
- `cargo test -p codex-core permission_prompt`
- `cargo test -p codex-core session::tests`
- `cargo test -p codex-exec cli_tests`
- `cargo test -p codex-core config::edit_tests`

**Rebase risk. HIGH.** Touches the approval loop, config editing, MCP tool
surface, and shared CLI options. Use the spec as the conformance checklist
after resolving merges; if any spec bullet fails its test, the merge is
not done.

---

## 17. OAuth image scope (net-zero)

Added (`47dcbd4af6`) then removed (`22b6bafafa`) — the current branch does
**not** request `api.model.images.request`. Listed here only so a rebase
doesn't accidentally resurrect one half of the pair.

---

## 18. PreToolUse command rewrites

**Purpose.** Honor Claude's `hookSpecificOutput.updatedInput` field on
`PreToolUse` so external hooks (notably `claude-bash-hook` aliases like
`rtk`) can rewrite a shell command before codex dispatches it. Without this
the only options were allow / block; with it a hook can transparently
substitute `git status` for `rtk git status`.

**Behavior details worth preserving.**
- `permissionDecision: allow` approves the current tool invocation. When paired
  with `updatedInput`, it also rewrites the invocation before dispatch.
- A rewrite is dropped if the same hook also blocks; block wins.
- The rewrite from the hook that finishes last wins. Hooks still see the
  original input, not another hook's rewrite.
- Two shell payload shapes are supported:
  - `ShellHandler` (`Vec<String>` argv) splits the rewritten command via
    `shlex::split`.
  - `ShellCommandHandler` (single `String`) copies verbatim.
- Rewrites mutate the invocation's parsed JSON `Value` directly rather
  than re-serializing typed params (the protocol params types only derive
  `Deserialize`), which has the side benefit of preserving any unknown
  fields the model emitted.
- A rewrite that fails to apply (missing `command` field, `shlex::split`
  failure, unsupported payload variant) surfaces as
  `PreToolUse hook rewrite failed: <reason>` — block-style, not silent.

**Key files.**
- `codex-rs/hooks/src/engine/output_parser.rs` (parse `updatedInput`,
  preserve `permissionDecision: allow` as an approval signal)
- `codex-rs/hooks/src/events/pre_tool_use.rs` (thread `updated_input`
  and approval through `PreToolUseOutcome`, completion-order rewrite selection)
- `codex-rs/core/src/hook_runtime.rs` (`PreToolUseHookResult` carries
  either a block message or `updated_input` plus approval; rewrite/approval is
  dropped on block)
- `codex-rs/core/src/tools/registry.rs` (`apply_pre_tool_use_rewrite`
  trait method on `ToolHandler` / `AnyToolHandler`; default no-op; dispatch
  carries the approval bit into the invocation)
- `codex-rs/core/src/tools/handlers/unified_exec.rs` (the rewrite impl
  lives here now; `shell.rs` was deleted in `b1c1c6e350`)
- `codex-rs/core/src/unified_exec/process_manager.rs` (`allow` skips the
  command approval prompt for the current invocation without creating a
  persistent exec-policy rule)

**Tests to re-run.**
- `cargo test -p codex-hooks output_parser`
- `cargo test -p codex-hooks pre_tool_use`
- `cargo test -p codex-core tools::handlers::unified_exec_tests`

**Rebase risk.** Medium. The trait addition on `ToolHandler` is upstream's
extension point; if upstream renames the dispatcher or splits the trait,
re-port the default no-op and the unified_exec impl. The output_parser
schema relaxation is the most fragile piece — upstream may re-tighten
unsupported-field validation, in which case re-add the explicit
`permissionDecision: allow` approval behavior and its optional `updatedInput`
rewrite pairing.

---

## 19. Aggressive upstream-feature removals

The fork has subtracted ~170K lines of upstream code that the OSS Linux-only
fork doesn't ship. Each rebase has to re-delete these because upstream
keeps shipping them.

**Build infrastructure (deleted entirely):**
- Bazel build system: `MODULE.bazel`, `MODULE.bazel.lock`, `.bazelrc`,
  `.bazelversion`, root `BUILD.bazel`, `defs.bzl`, `rbe.bzl`,
  `workspace_root_test_launcher.{bat,sh}.tpl`, all 99 nested
  `BUILD.bazel` files, all 27 patches in `patches/`, `bazel.yml` /
  `rusty-v8-release.yml` / `v8-canary.yml` / `Dockerfile.bazel`
  workflows, `setup-bazel-ci` / `setup-rusty-v8-musl` /
  `prepare-bazel-ci` actions, `third_party/v8/`. Commit `16386babdc`.
- SDK packages: `sdk/python`, `sdk/python-runtime`, `sdk/typescript`,
  `sdk.yml` workflow, `codex-sdk` package path in
  `codex-cli/scripts/build_npm_package.py`. Commit `16386babdc`.
- `argument-comment-lint` Dylint tool + 4 workflows. Commit `c89a4c527b`.
- `shell-tool-mcp/` stub. Commit `16386babdc`.

**Workspace crates removed:**
- `responses-api-proxy` (`1972aa32dc`)
- `cloud-tasks` + `cloud-tasks-client` + `cloud-tasks-mock-client` (`3ea96713f2`)
- `realtime-webrtc` + voice/audio TUI (`314426231e`)
- `windows-sandbox-rs` + `core/src/windows_sandbox*.rs` (`fb67fde605`,
  `b6883dda50`)
- `lmstudio` (`46627d46a0`)
- `chatgpt` (`b0959759a0`) + TUI connector listing UI (`3f2215deff`)
- `mcp-server` (Codex-as-MCP-server, the inverse of `codex-mcp`) (`c17284cbdc`)
- `feedback` + TUI feedback view + `feedback/upload` v2 RPC
  (`4d5cce1b35`, `59084856f0`)
- `otel` (kept stub types in `core/src/telemetry.rs`) (`5d0c63fd45`)
- `cloud-requirements` (`f17766b20e`)
- `aws-auth` + Amazon Bedrock provider (`e866b86470`)
- `analytics` gutted in-place to no-ops (`f2e7ab2a46`)
- `rollout-trace` inlined into `codex-core` (`af03000a8a`, `95e180c2ac`)
- `exec-server` remote/WebSocket backend (`e212445f28`)

**Feature flags removed (`Stage::Removed` or `Stage::Deprecated`):**
- `RemoteModels` (`cf84f8b07a`)
- `WebSearchRequest`, `WebSearchCached`, `SearchTool` (`f5249a4570`)
- `UseLegacyLandlock` + the legacy Landlock code path (`6a713150b7`)
- `LegacyMultiAgentV1` (`583ed3144a`)
- `LegacyShellCompat` (`b1c1c6e350`)
- `Sqlite`, `WindowsSandbox`, and `WindowsSandboxElevated`, plus their
  corresponding legacy config/schema keys (`251ea266ff`)

**Workspace plumbing:**
- `default-members = ["cli"]` in workspace `Cargo.toml` so `cargo build`
  only touches the shipped binary.
- mold linker + `target-cpu=native` + `-Z share-generics=y` in
  `codex-rs/.cargo/config.toml`. Requires `RUSTC_BOOTSTRAP=1` in env.
- Dev profile: `debug = "line-tables-only"`, `split-debuginfo = "unpacked"`.
- `docs/gpt-5.4-underperformance-fix.md` records the local model-instruction
  benchmark note added with the feature-flag cleanup commit.

**Rebase risk. HIGHEST.** This is mechanical but voluminous — every rebase
will reintroduce all of the above. Strategy:
1. After the rebase merges, run `git log --oneline origin/main..HEAD` and
   look for the deletion commits listed above.
2. If a deletion commit was lost in the merge, cherry-pick it back.
3. If upstream introduced a NEW peripheral feature on top of a removed
   one (e.g. cloud-tasks v2), assess whether it falls under the same
   "fork doesn't ship" criterion and remove it too — then add a new
   deletion commit and update this section.
4. For deps: re-run `cargo machete` after every rebase to catch newly
   introduced unused crate-level deps.

---

## Rebase checklist

Before declaring a rebase clean, walk this list:

1. `git log --oneline origin/main..HEAD` — every commit here should map to a
   section above. New commits need new sections.
2. Resolve all files currently flagged `UU`; use
   `git status --short --untracked-files=all` as the source of truth.
3. Accept or reject any pending `*.snap.new` files after reading the rendered
   diff directly.
4. Re-run per-section test suites listed above, plus:
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets`
   - `cargo test --workspace`
5. Regenerate protocol schemas if anything under
   `codex-rs/app-server-protocol/schema/` diverged.
6. Smoke-test the fork's own features end-to-end: run a session with a
   `SessionStart` hook, trigger `/run-plan`, spawn a subagent, exercise the WIP
   permission prompt approval tool, and confirm `deploy.sh` still installs
   cleanly.
