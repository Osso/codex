# TUI customization

This fork adds user-facing tweaks to the Codex TUI that are absent from upstream: two configurable markdown colors (`strong_color`, `code_color`), overlay navigation remaps, a transcript-height cache, a double-Esc backtrack picker, queued/steering message editing, suppressed completion notifications for queued turns, and a keyboard-event flush on terminal restore. Source lives primarily under `codex-rs/tui/src/`, with color configuration threaded through `codex-rs/config/src/types.rs` (`TuiConfig`) and `codex-rs/core/src/config/mod.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/tui-customization.md.

## What it must do

### Colors

- [ ] `TuiConfig.strong_color` accepts an optional hex color string and applies it to bold markdown spans in the TUI.
- [ ] `TuiConfig.code_color` accepts an optional hex color string and applies it to inline code spans in the TUI.
- [ ] A shared hex-parsing helper converts both color strings; invalid values are rejected at config load time.

### Overlay navigation

- [ ] The transcript/pager overlay maps Up/Down keys for line navigation and Esc to quit (replacing any prior binding).
- [ ] The key remaps are expressed in `codex-rs/tui/src/pager_overlay.rs` and `codex-rs/tui/src/app_backtrack.rs`.

### Transcript overlay height cache

- [x] During esc-esc backtrack, transcript row renderables stay alive so wrapped-height calculations remain cached across overlay redraws (`set_highlight_cell_keeps_cached_heights`).

### Double-Esc backtrack picker

- [x] The backtrack picker returns one `SelectionItem` per prior user prompt, oldest-first, with multi-line prompts collapsed to a single line (`backtrack_picker_items_show_one_prompt_per_line_oldest_first`).
- [ ] Double Esc opens the compact bottom-pane `ListSelectionView` instead of the full transcript overlay.
- [ ] The picker is searchable and initially scrolled to the newest prompt.
- [ ] The picker uses a taller per-view row cap (14 rows) without altering the global popup default.
- [ ] Selecting a row triggers the existing rollback + composer-prefill path.
- [ ] Esc or Ctrl+C cancels and clears backtrack state.

### Queued/steering messages

- [ ] The bottom pane advertises Up (not Shift-Left) as the queued-message edit binding in `pending_input_preview.rs`.
- [ ] Pressing Up on a queued turn recalls it into the composer for editing.
- [ ] Re-submitting a recalled steering message re-uses the original `steerId` rather than appending the edited text as a second message.
- [ ] Core replaces a still-pending input instead of appending both original and edited text (`codex-rs/core/src/state/turn.rs`, `codex-rs/core/src/session/mod.rs`).
- [ ] Queued edits and steering messages do not emit a redundant completion notification when the queued turn finishes.

### Terminal restore

- [ ] On terminal restore, buffered keyboard events are flushed (4-line change in `codex-rs/tui/src/tui.rs`).

### Import cleanup

- [ ] `codex-rs/tui/src/lib.rs` uses the `codex_core::Config` import path without redundant aliases or dead imports.

## How it works

- `docs/wiki/systems/tui-customization.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/config/src/types.rs` — `TuiConfig` struct; `strong_color` and `code_color` fields.
- `codex-rs/core/src/config/mod.rs` — config loading; hex-parsing helper wired to color fields.
- `codex-rs/tui/src/lib.rs` — TUI entry point; applies color config to markdown renderer; `codex_core::Config` import.
- `codex-rs/tui/src/pager_overlay.rs` — transcript/pager overlay; Up/Down/Esc key remap; height-cache logic.
- `codex-rs/tui/src/app_backtrack.rs` — backtrack state machine; double-Esc routing to picker; overlay navigation.
- `codex-rs/tui/src/app_backtrack_picker.rs` — builds `SelectionItem` list from `UserHistoryCell` slice; `one_line_prompt_label` helper.
- `codex-rs/tui/src/bottom_pane/list_selection_view.rs` — generic searchable list picker used by the backtrack picker.
- `codex-rs/tui/src/app_event.rs` — `ApplyBacktrackSelection` event variant.
- `codex-rs/tui/src/app/event_dispatch.rs` — dispatch path for backtrack picker selection.
- `codex-rs/tui/src/bottom_pane/pending_input_preview.rs` — Up binding advertisement for queued messages.
- `codex-rs/tui/src/chatwidget.rs` — Up key handling for queued recall; steering message submission.
- `codex-rs/core/src/state/turn.rs` — replace-pending-input logic for steering edits.
- `codex-rs/core/src/session/mod.rs` — session-level handling of recalled steer with original `steerId`.
- `codex-rs/app-server-protocol/src/protocol/v2/turn.rs` — protocol message updated for steering message editing.
- `codex-rs/tui/src/tui.rs` — keyboard event flush on terminal restore.

## Tests asserting this spec

- `codex-rs/tui/src/app_backtrack_picker.rs` — `tests::backtrack_picker_items_show_one_prompt_per_line_oldest_first` (double-Esc picker item shape).
- `codex-rs/tui/src/pager_overlay.rs` — `tests::set_highlight_cell_keeps_cached_heights` (transcript height cache regression).
- `codex-rs/tui/src/snapshots/` — TUI snapshot suite (layout and status rendering changes including queued-message indicator).

## Rebase risk

Upstream restyles the TUI frequently; the two color config entries are additive and usually survive, but the overlay changes touch files upstream also edits. After a rebase, manually smoke-test double Esc from an empty composer: first press shows the hint, second press opens the single-line prompt picker rather than the transcript overlay; the newest prompt should be selected at the bottom. **Risk: Medium.**

## Out of scope

- None noted.
