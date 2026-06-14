# Worktree Startup Option

The `-w`/`--worktree <NAME>` startup flag creates or reuses a sibling Git worktree before launching TUI or exec mode, letting fork work begin in an isolated checkout without manual `git worktree` setup. New worktrees are created from `origin/master` and named after the provided value. The flag is wired through shared CLI option structs (`codex-rs/utils/cli/src/shared_options.rs`) into both binary entry points (`codex-rs/cli/src/main.rs`, `codex-rs/exec/src/lib.rs`), with the worktree lifecycle logic isolated in `codex-rs/git-utils/src/worktree.rs`. TUI startup in `codex-rs/tui/src/lib.rs` receives the resolved worktree path. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/worktree-startup-option.md.

## What it must do

### CLI surface
- [x] Accept `-w`/`--worktree <NAME>` as a flag on both `codex` and `codex-exec` binaries.

### Worktree resolution
- [ ] If a worktree named `<NAME>` already exists as a sibling directory, reuse it without re-creating.
- [ ] If no such worktree exists, create one from `origin/master` named `<NAME>`.

### Startup integration
- [ ] TUI launch path (`codex-rs/tui/src/lib.rs`) receives the resolved worktree path and sets it as the working directory.
- [ ] Exec mode launch path (`codex-rs/exec/src/lib.rs`) receives the resolved worktree path and sets it as the working directory.

## How it works

- `docs/wiki/systems/worktree-startup-option.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/utils/cli/src/shared_options.rs` — defines the shared `-w`/`--worktree` CLI option struct used by both binaries.
- `codex-rs/git-utils/src/worktree.rs` — worktree creation and reuse logic.
- `codex-rs/cli/src/main.rs` — passes worktree option into TUI startup.
- `codex-rs/tui/src/lib.rs` — receives resolved worktree path at TUI init.
- `codex-rs/exec/src/lib.rs` — receives resolved worktree path at exec init.

## Tests asserting this spec

- `codex-rs/exec/src/cli_tests.rs` — `parses_worktree_flag`: asserts `-w feature-a` is parsed and stored correctly.

## Rebase risk

Medium. Shared CLI option plumbing and TUI/exec startup are active upstream surfaces; keep the behavior isolated in git-utils and re-check both binary entry points after merge resolution.

## Out of scope

- None noted.
