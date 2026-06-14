# Prompt-Context Hooks

Hook binaries registered under `SessionStart`, `UserPromptSubmit`, and `Stop` events can inject additional context into the model prompt by returning `{ "hookSpecificOutput": { "additionalContext": "..." } }` on stdout; this fork prepends that text to the next user message as a distinct content block. The implementation spans `codex-rs/hooks/src/events/` (per-event parsers), `codex-rs/hooks/src/engine/output_parser.rs` (stdout parsing), `codex-rs/core/src/session/turn.rs` (prepend helper and CRLF separator), `codex-rs/core/src/state/session.rs` (pending-stop-hook context storage), and `codex-rs/tui/src/history_cell/hook_cell.rs` (handler-name rendering). See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/prompt-context-hooks.md.

## What it must do

### Context injection

- [x] A `SessionStart` hook whose stdout contains `additionalContext` has that text prepended to the first user message before it reaches the model (`plain_stdout_becomes_model_context` in `codex-rs/hooks/src/events/session_start.rs`).
- [x] A `UserPromptSubmit` hook can prepend additional context to the user's message (`user_prompt_hook_additional_context_is_prepended_to_input` in `codex-rs/core/src/session/tests.rs`).
- [ ] A `Stop` hook returning `additionalContext` carries that text forward so it is prepended to the next user prompt on the following turn.
- [x] When the hook update is a plain text (single-item) replacement, the context is prepended rather than replacing the existing input (`single_text_user_prompt_hook_update_is_prepended_instead_of_replacing_input` in `codex-rs/core/src/session/tests.rs`).
- [x] When the hook update is an array replacement, the input is replaced wholesale rather than prepended (`array_user_prompt_hook_update_still_replaces_input` in `codex-rs/core/src/session/tests.rs`).

### Multiple-block separation

- [x] Multiple simultaneous hook context blocks are separated by CRLF and rendered as distinct content blocks, not concatenated (`prepend_user_text_input_separates_multiple_prepended_context_blocks` in `codex-rs/core/src/session/tests.rs`).
- [x] Prepending inserts the hook context before existing input items (`prepend_user_text_input_adds_context_before_existing_items` in `codex-rs/core/src/session/tests.rs`).

### Hook label rendering

- [ ] Hook handler names (from `config.toml`) are surfaced in the UI so the operator can identify which hook produced the text (see `hook_event_label` / `HookRunCell` in `codex-rs/tui/src/history_cell/hook_cell.rs`).

### Stop-hook context handling

- [x] A `Stop` hook returning `continue=false` blocks the stop and may carry a reason; it does not carry its block reason forward into the next user prompt (`continue_false_overrides_block_decision` in `codex-rs/hooks/src/events/stop.rs`).
- [x] `SessionStart` `continue=false` preserves any context for later turns without immediately blocking (`continue_false_preserves_context_for_later_turns` in `codex-rs/hooks/src/events/session_start.rs`).
- [x] `UserPromptSubmit` `continue=false` preserves context for later turns (`continue_false_preserves_context_for_later_turns` in `codex-rs/hooks/src/events/user_prompt_submit.rs`).

### Input normalization on steer

- [ ] When the user steers mid-turn, the pending input is normalized to strip any previously prepended hook context before re-submission.

## How it works

- `docs/wiki/systems/prompt-context-hooks.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/hooks/src/events/stop.rs` — stop-event parser; surfaces `continue=false` and `additionalContext`.
- `codex-rs/hooks/src/events/session_start.rs` — session-start parser; plain stdout and `additionalContext` become model context.
- `codex-rs/hooks/src/events/user_prompt_submit.rs` — user-prompt-submit parser; `additionalContext` prepended to user message.
- `codex-rs/hooks/src/events/common.rs` — shared parsing utilities and `apply_patch` translation (see docs/specs/apply-patch-claude-translation.md).
- `codex-rs/hooks/src/engine/output_parser.rs` — parses raw hook stdout into typed output structs.
- `codex-rs/hooks/src/schema.rs` — JSON schema definitions for hook I/O contracts.
- `codex-rs/hooks/schema/generated/stop.command.output.schema.json` — generated JSON schema for stop hook output.
- `codex-rs/core/src/session/turn.rs` — `prepend_user_text_input` helper; CRLF spacer between multiple context blocks.
- `codex-rs/core/src/state/session.rs` — stores pending stop-hook additional context between turns.
- `codex-rs/tui/src/history_cell/hook_cell.rs` — renders hook run rows with event-kind label; coalesces fast/quiet runs.

## Tests asserting this spec

- `codex-rs/hooks/src/events/session_start.rs` — `plain_stdout_becomes_model_context`, `continue_false_preserves_context_for_later_turns`, `invalid_json_like_stdout_fails_instead_of_becoming_model_context`
- `codex-rs/hooks/src/events/user_prompt_submit.rs` — `continue_false_preserves_context_for_later_turns`
- `codex-rs/hooks/src/events/stop.rs` — `block_decision_with_reason_sets_continuation_prompt`, `block_decision_without_reason_is_invalid`, `continue_false_overrides_block_decision`
- `codex-rs/core/src/session/tests.rs` — `single_text_user_prompt_hook_update_is_prepended_instead_of_replacing_input`, `array_user_prompt_hook_update_still_replaces_input`, `user_prompt_hook_additional_context_is_prepended_to_input`, `prepend_user_text_input_adds_context_before_existing_items`, `prepend_user_text_input_separates_multiple_prepended_context_blocks`

## Rebase risk

Upstream is actively refactoring the hooks crate. Watch for renames of `additional_context`/`additionalContext` JSON keys; changes to how `session/turn.rs` assembles user input items (the CRLF separator lives there); changes to `history_cell/hook_cell.rs` (upstream tends to rewrite the rendering paths). If upstream adds native Claude-format hook support, reconcile rather than drop this code.

## Out of scope

- None noted.
