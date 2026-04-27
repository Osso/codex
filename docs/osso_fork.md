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
helper matches both `- [ ]` and `* [ ]` unchecked markers.

**Tests.** TUI unit tests in `slash_dispatch.rs`.

**Rebase risk.** Upstream rewrites the slash-command dispatch regularly. If the
enum dispatch changes shape, re-port `dispatch_run_plan`.

---

## 7. Multi-agent v2 as default + legacy v1 gated

**Purpose.** The fork defaults to the v2 multi-agent surface (exec_command +
`MultiAgentV2` handlers: `WaitAgent`, `SendMessage`, `MessageTool`, etc.). v1
handlers and legacy shell aliases are kept behind the
`multi-agent-v1`/deprecated feature flags so upstream tests still run.

**Key files.**
- `codex-rs/features/src/lib.rs` + `codex-rs/features/src/tests.rs`
  (flag definitions)
- `codex-rs/tools/src/tool_registry_plan.rs` (registry defaulting)
- `codex-rs/tools/src/tool_config.rs`
- `codex-rs/core/src/tools/handlers/multi_agents_v2/*`
- `codex-rs/core/src/tools/handlers/multi_agents/send_input.rs` (v1 compat)
- `codex-rs/app-server/src/bespoke_event_handling.rs`
- `codex-rs/app-server-protocol/src/protocol/thread_history.rs`
- `codex-rs/app-server-protocol/src/protocol/v2.rs`
- Generated schemas under `codex-rs/app-server-protocol/schema/json/v2/`
- `codex-rs/tui/src/chatwidget.rs`, `codex-rs/tui/src/multi_agents.rs`

**Tests to re-run.**
- `cargo test -p codex-core tools::spec_tests`
- `cargo test -p codex-tools tool_registry_plan_tests`
- `cargo test -p codex-app-server suite::v2::turn_start`

**Rebase risk. HIGH.** This is the fork's largest divergence area. Upstream
actively changes the tool registry, protocol v2 schemas, and the collab
interaction tool kinds. After a rebase:
1. Re-generate schemas (`just generate` / whatever the current invocation is).
2. Re-diff `tool_registry_plan.rs` by hand — a three-way merge almost never
   produces correct output here.
3. Run the full `app-server` suite before declaring victory.

---

## 8. Unified exec as the default public shell tool

**Purpose.** Public shell tools (`shell`, `bash`) collapse to the unified exec
tool surface. The model-visible tool spec is `exec_command`/`write_stdin`, and
runtime dispatch routes through `UnifiedExecHandler`. Legacy shell aliases are
retained only behind the compatibility gate.

**Key files.**
- `codex-rs/core/src/tools/handlers/shell.rs`
- `codex-rs/core/src/tools/spec.rs` (handler registration)
- `codex-rs/core/src/tools/runtimes/shell.rs`
- `codex-rs/core/src/tools/runtimes/shell/unix_escalation.rs` (3-line trim)
- `codex-rs/core/src/tools/runtimes/shell/zsh_fork_backend.rs`
- `codex-rs/tools/src/local_tool.rs` (`exec_command`, `write_stdin`, and
  `request_permissions` tool spec factories retained)
- `codex-rs/tools/src/local_tool_tests.rs` (retained for those factories)

**Rebase risk.** HIGH — upstream is mid-refactor on shell tooling and the
legacy `shell` / `shell_command` constructors may be restored upstream.
Conflict resolution should prefer unified exec as the public shell surface while
retaining `local_tool.rs` for the unified exec tool-spec helpers.

---

## 9. Subagent / inter-agent communication polish

**Purpose.** Three related improvements to v2 multi-agent UX:

1. **Inter-agent message pretty-printing.** Subagent notifications and
   inter-agent messages arrive as JSON `InterAgentCommunication` blobs embedded
   in assistant messages. The TUI parses them and renders a compact
   `author → recipient` cell instead of raw JSON.
2. **Trigger-turn notifications.** Subagent notifications now mark themselves
   as trigger turns so truncation logic counts them as turn boundaries.
3. **`wait_agent` early return.** If no descendant agents exist, `wait_agent`
   returns immediately ("No agents available yet.") rather than blocking until
   timeout.

**Key files.**
- `codex-rs/tui/src/inter_agent_message.rs` (new module, self-contained)
- `codex-rs/tui/src/chatwidget/realtime.rs`
- `codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs`
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
- **Pending steer pop on Up.** Pending steering messages can be popped back into
  the composer with Up, with focused coverage in composer submission tests.
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

**Purpose.** Three independent bug fixes:

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

**Tests.** `cargo test -p codex-app-server suite::auth suite::v2::account`.

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

## 13. Local deploy script + Osso branding

**Purpose.** Build and install a local fork build with the `-osso` suffix,
independent of upstream release machinery.

**Components.**
- **`deploy.sh`** at repo root: `cargo install` into `$CODEX_INSTALL_ROOT`
  with `--locked --force`; also installs the MCP server.
- **Branding split.** `CODEX_CLI_VERSION` (machine-readable, tracks upstream
  semver) vs `CODEX_CLI_DISPLAY_VERSION` (e.g. `0.120.0-osso`). Update checks
  use the former, UI uses the latter.
  - `codex-rs/tui/src/version.rs`
  - `codex-rs/cli/src/main.rs`
  - `codex-rs/tui/src/update_prompt.rs`
  - `announcement_tip.toml`
- **Release profile tuning** (`codex-rs/Cargo.toml`):
  - LTO disabled for faster local builds.
  - Release codegen units raised to 4.
- **`shell-tool-mcp/package.json` retained.** Upstream deleted this in
  `e89e5136bd`; the fork keeps it for the deploy script.

**Tests.** `cargo test -p codex-tui version`.

**Rebase risk.** Medium. Every upstream version bump will conflict with the
branding split; walk through `version.rs` manually each time. If upstream
deletes `shell-tool-mcp/` again, restore it or update `deploy.sh` accordingly.

---

## 14. Model prompt + generated-file hygiene

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

## 15. Permission prompt approval tool (WIP)

**Status. WIP.** This feature is mid-integration and should not be treated as
stable until the approval-tool flow has been reviewed end-to-end and the
approval-loop tests pass after the final upstream rebase.

**Purpose.** Route shell/exec approval decisions through an MCP-hosted
permission prompt tool so approval policy can come from the user's configured
prompt provider instead of only the built-in TUI approval surface. Allow
decisions can be cached for the session and persisted back to user, project, or
local config.

**Key files.**
- `codex-rs/codex-mcp/src/permission_prompt.rs` (MCP contract)
- `codex-rs/core/src/permission_prompt.rs` (approval decision flow, caching,
  persistence)
- `codex-rs/core/src/session/mod.rs`
- `codex-rs/core/src/session/session.rs`
- `codex-rs/core/src/session/tests.rs`
- `codex-rs/core/src/config/edit.rs`
- `codex-rs/cli/src/main.rs`
- `codex-rs/exec/src/cli.rs`
- `codex-rs/tui/src/lib.rs`
- `codex-rs/utils/cli/src/shared_options.rs`

**Behavior details still in flight.**
- `--permission-prompt-tool` CLI plumbing exists, but the rebase is still
  resolving command-surface conflicts.
- MCP prompt decisions can approve one command, allow future matching commands
  for the session, or persist allow rules into config.
- Approval-loop coverage and CLI/help docs exist, but this is still marked WIP
  until the behavior is validated through the full approval surface.

**Tests to re-run after WIP lands.**
- `cargo test -p codex-core permission_prompt`
- `cargo test -p codex-core session::tests`
- `cargo test -p codex-exec cli`
- CLI help snapshot/fixture tests touched by `--permission-prompt-tool`

**Rebase risk. HIGH.** This touches the approval loop, config editing, MCP
tool surface, and shared CLI options. Resolve by preserving the explicit
permission-prompt-tool contract and then re-checking every approval path, not
just command parsing.

---

## 16. OAuth image scope (net-zero)

Added (`47dcbd4af6`) then removed (`22b6bafafa`) — the current branch does
**not** request `api.model.images.request`. Listed here only so a rebase
doesn't accidentally resurrect one half of the pair.

---

## 17. PreToolUse command rewrites

**Purpose.** Honor Claude's `hookSpecificOutput.updatedInput` field on
`PreToolUse` so external hooks (notably `claude-bash-hook` aliases like
`rtk`) can rewrite a shell command before codex dispatches it. Without this
the only options were allow / block; with it a hook can transparently
substitute `git status` for `rtk git status`.

**Behavior details worth preserving.**
- `permissionDecision: allow` is now treated as a no-op rather than as an
  unsupported field, so a hook can return Claude's full schema unchanged
  (codex's normal permission flow still runs). Only `Ask` is rejected.
- A rewrite is dropped if the same hook also blocks; block wins.
- First hook in the run that emits `updatedInput` wins. Subsequent hooks
  see the original input, not the rewrite (chained rewrites would require
  a stable ordering policy we don't expose).
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
  relax `Allow` rejection)
- `codex-rs/hooks/src/events/pre_tool_use.rs` (thread `updated_input`
  through `PreToolUseOutcome`, "first rewrite wins" rule)
- `codex-rs/core/src/hook_runtime.rs` (`PreToolUseHookResult` carries
  both `block_message` and `updated_input`; rewrite is dropped on block)
- `codex-rs/core/src/tools/registry.rs` (`apply_pre_tool_use_rewrite`
  trait method on `ToolHandler` / `AnyToolHandler`; default no-op)
- `codex-rs/core/src/tools/handlers/shell.rs` (impls for `ShellHandler` /
  `ShellCommandHandler` plus the JSON `Value` mutation helpers)

**Tests to re-run.**
- `cargo test -p codex-hooks output_parser`
- `cargo test -p codex-hooks pre_tool_use`
- `cargo test -p codex-core tools::handlers::shell_tests`

**Rebase risk.** Medium. The trait addition on `ToolHandler` is upstream's
extension point; if upstream renames the dispatcher or splits the trait,
re-port the default no-op and the two shell impls. The output_parser
schema relaxation is the most fragile piece — upstream may re-tighten
unsupported-field validation, in which case re-add an explicit allow-list
for `updatedInput` and `permissionDecision: allow`.

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
