# Run-Plan Slash Command

The `/run-plan` slash command automates checklist-driven work by walking `PLAN.md` (or a user-specified file), finding the first unchecked item, and submitting it as the next prompt. On dispatch it exports `PLAN_FILE=<name>` (or `PLAN_FILE=1` for the default) into the process environment so that downstream hooks and child processes can read which plan file is active. The `RunPlan` enum variant is declared in `codex-rs/tui/src/slash_command.rs`; dispatch logic, plan-file parsing, and environment export live in `codex-rs/tui/src/chatwidget/slash_dispatch.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/run-plan-command.md.

## What it must do

### Command dispatch
- [ ] The `/run-plan` slash command is recognized as a valid `SlashCommand::RunPlan` variant.
- [ ] An optional inline filename argument is accepted (via `prepare_inline_args_submission`); the composer is cleared after dispatch.
- [ ] When no argument is supplied, `PLAN.md` in the working directory is used as the default plan file.

### Environment export
- [ ] `PLAN_FILE` is set to the supplied filename when a non-default plan file is given.
- [ ] `PLAN_FILE=1` is set when the default `PLAN.md` is used, signaling the default to downstream hooks.

### Plan-item extraction
- [x] The first `- [ ]` or `* [ ]` line (unchecked item) is found and returned as the next prompt.
- [x] Already-checked items (`- [x]`, `* [x]`) are skipped.
- [x] Returns `None` (no submission) when all items are checked.
- [x] Returns an `io::Error` (reported as a command error, not a silent no-op) when the plan file does not exist.

### Error reporting
- [x] A missing plan file produces a visible command error rather than a silent no-op submission.

## How it works

- `docs/wiki/systems/run-plan-command.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/tui/src/slash_command.rs` — `SlashCommand::RunPlan` enum variant and its help text.
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` — `dispatch_run_plan`: sets `PLAN_FILE`, calls `find_next_plan_item`, submits the prompt; `find_next_plan_item`: opens the plan file and returns the first unchecked `- [ ]` / `* [ ]` line.

## Tests asserting this spec

- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` — `tests::finds_first_unchecked_plan_item`
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` — `tests::returns_none_without_unchecked_plan_item`
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` — `tests::returns_error_for_missing_plan_file`

## Rebase risk

Upstream rewrites the slash-command dispatch regularly. If the enum dispatch changes shape, re-port `dispatch_run_plan`.

## Out of scope

- None noted.
