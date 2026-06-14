# Apply-Patch → Claude Write Translation

Existing hook binaries wired to pre-tool-use and post-tool-use events expect the Claude `Write` tool payload shape (`{ file_path, content }`). Codex emits `apply_patch` instead. This fork adds a translation layer in `codex-rs/hooks/src/events/common.rs` that detects single-file `apply_patch` inputs and synthesizes the `Write`-compatible shape so those hook scripts work without modification. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/apply-patch-claude-translation.md.

## What it must do

### Detection and translation

- [x] When a `PreToolUse` event carries `tool_name = "apply_patch"` and the patch touches exactly one file, the hook receives a synthesized `{ file_path, content }` input matching the Claude `Write` tool shape (`command_input_translates_single_file_apply_patch_for_claude_write_hooks` in `codex-rs/hooks/src/events/pre_tool_use.rs`).
- [ ] When a `PostToolUse` event carries `tool_name = "apply_patch"` and the patch touches exactly one file, the same translation applies (translation path confirmed in `codex-rs/hooks/src/events/post_tool_use.rs` lines 147, 342, 350; no dedicated async test found for post-tool-use translation).
- [ ] Multi-file `apply_patch` inputs are intentionally not translated; hooks receive the raw `apply_patch` input unchanged.

### Matcher inputs

- [ ] The translated event still carries the canonical `tool_name` (`"apply_patch"`) so matchers that filter by tool name continue to work correctly.

## How it works

- `docs/wiki/systems/apply-patch-claude-translation.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/hooks/src/events/common.rs` — `apply_patch_tool_input` translation function; matcher utilities shared across events.
- `codex-rs/hooks/src/events/pre_tool_use.rs` — calls translation before serialising command input for pre-tool-use hooks.
- `codex-rs/hooks/src/events/post_tool_use.rs` — calls translation before serialising command input for post-tool-use hooks.

## Tests asserting this spec

- `codex-rs/hooks/src/events/pre_tool_use.rs` — `command_input_translates_single_file_apply_patch_for_claude_write_hooks`

## Rebase risk

If upstream changes the `apply_patch` tool input schema (e.g. moves from command string to structured form), the translator must be updated. Multi-file patches are intentionally not translated — do not silently "fix" this.

## Out of scope

- Multi-file patch translation.
