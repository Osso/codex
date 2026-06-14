# PreToolUse Command Rewrites

PreToolUse command rewrites let external hooks transparently substitute a different shell command before Codex dispatches it. When a hook returns `hookSpecificOutput.updatedInput`, the invocation's parsed JSON value is mutated in-place and the rewritten command is what actually runs — enabling aliases like `rtk` to expand without Codex or the model knowing. The feature lives in `codex-rs/hooks/src/engine/output_parser.rs`, `codex-rs/hooks/src/events/pre_tool_use.rs`, `codex-rs/core/src/hook_runtime.rs`, `codex-rs/core/src/tools/registry.rs`, `codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs`, and `codex-rs/core/src/unified_exec/process_manager.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/pre-tool-use-rewrites.md.

## What it must do

### Permission decisions and rewrites

- [x] `permissionDecision: allow` approves the current tool invocation.
- [x] `permissionDecision: allow` paired with `updatedInput` also rewrites the invocation before dispatch.
- [x] `permissionDecision: ask` is accepted as "continue, but do not approve"; the normal Codex permission flow still runs.
- [x] `permissionDecision: ask` paired with `updatedInput` rewrites the invocation before that permission flow.
- [ ] A rewrite is dropped when the same hook also blocks; block wins.
- [x] The hook that finishes last (by completion order) wins when multiple hooks each supply an `updatedInput`; hooks still see the original input.
- [x] `permissionDecision: allow` without `updatedInput` grants approval and leaves the invocation unchanged.

### Shell payload shapes

- [ ] `ShellHandler` (`Vec<String>` argv) splits the rewritten command string via `shlex::split` and replaces the argv vector.
- [ ] `ShellCommandHandler` (single `String`) copies the rewritten value verbatim.

### Error handling

- [ ] A rewrite that cannot be applied (missing `command` field, `shlex::split` failure, unsupported payload variant) surfaces as `"PreToolUse hook rewrite failed: <reason>"` — block-style, not silent.

### Approval bypass

- [ ] When a hook grants approval (`permissionDecision: allow`), the command approval prompt is skipped for that invocation without creating a persistent exec-policy rule.

### Protocol compatibility

- [ ] Rewrites mutate the invocation's parsed `serde_json::Value` directly rather than re-serializing typed params, preserving any unknown fields the model emitted.

## How it works

- `docs/wiki/systems/pre-tool-use-rewrites.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry (§18).
- Cross-links: `docs/specs/approval-system.md` (the broader approval surface) and `docs/specs/permission-prompt-tool.md` (the permission-prompt-tool approval path). This spec covers the hook-driven command-rewrite path that runs alongside those surfaces.

## Implementation inventory

- `codex-rs/hooks/src/engine/output_parser.rs` — parses `updatedInput` from hook stdout; preserves `permissionDecision: allow` as approval signal; accepts `permissionDecision: ask` as valid non-approval decision.
- `codex-rs/hooks/src/events/pre_tool_use.rs` — threads `updated_input` and approval through `PreToolUseOutcome`; implements completion-order rewrite selection (`latest_updated_input`).
- `codex-rs/core/src/hook_runtime.rs` — `PreToolUseHookResult` carries either a block message or `updated_input` plus approval; rewrite and approval are dropped on block.
- `codex-rs/core/src/tools/registry.rs` — `with_updated_hook_input` trait method on `ToolHandler`/`AnyToolHandler`; default no-op; dispatch carries approval and approval-required bits.
- `codex-rs/core/src/tools/handlers/unified_exec/exec_command.rs` — `with_updated_hook_input` implementation for the unified exec handler.
- `codex-rs/core/src/unified_exec/process_manager.rs` — `allow` skips the command approval prompt for the current invocation without creating a persistent exec-policy rule.

## Tests asserting this spec

- `codex-rs/hooks/src/engine/output_parser.rs` — `tests::pre_tool_use_accepts_claude_allow_with_updated_input`
- `codex-rs/hooks/src/events/pre_tool_use.rs` — `tests::permission_decision_allow_can_update_input`, `tests::last_completed_updated_input_wins`, `tests::permission_decision_allow_without_updated_input_grants_approval`, `tests::permission_decision_ask_continues_with_approval_required`, `tests::permission_decision_ask_can_update_input_with_approval_required`, `tests::permission_decision_deny_blocks_processing`

Run: `cargo test -p codex-hooks output_parser` and `cargo test -p codex-hooks pre_tool_use`.

## Rebase risk

Medium. The `with_updated_hook_input` trait addition on `ToolHandler` is this fork's extension point; if upstream renames the dispatcher or splits the trait, re-port the default no-op and the `unified_exec` impl. The `output_parser` schema relaxation is the most fragile piece — upstream may re-tighten unsupported-field validation, in which case re-add the explicit `permissionDecision: allow` approval behavior, the `permissionDecision: ask` non-approval behavior, and their optional `updatedInput` rewrite pairing.

## Out of scope

- Persistent exec-policy rule creation (approval is per-invocation only).
- Rewrites on any hook event other than `PreToolUse`.
