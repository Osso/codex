# User Rules Loader

The user-rules-loader feature mirrors Claude Code's `~/.claude/rules/` directory convention inside Codex. At startup, every `*.md` file found under `$CODEX_HOME/rules/` is read in sorted filename order, trimmed of surrounding whitespace, joined with double newlines, and injected into the assembled user instructions. The loader lives in `codex-rs/core/src/config/rules.rs` (82 lines, self-contained); it is invoked from `codex-rs/core/src/agents_md.rs`, which integrates the resulting string into the instruction block that the agent receives. Configuration plumbing lives in `codex-rs/core/src/config/mod.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/user-rules-loader.md.

## What it must do

### Directory discovery
- [x] Returns `None` when `$CODEX_HOME/rules/` does not exist (no error raised).
- [x] Returns `None` when the directory exists but contains no non-empty `*.md` files.

### File loading
- [x] Loads only `*.md` files; non-markdown files in the directory are ignored.
- [x] Files are sorted by filename before reading, producing deterministic ordering.
- [x] Each file's content is trimmed of leading/trailing whitespace before inclusion.
- [x] Files whose trimmed content is empty are silently skipped.
- [x] Non-empty trimmed contents are joined with a double newline (`\n\n`).

### Instruction assembly integration
- [ ] The concatenated rules string is appended to the user instructions block assembled in `codex-rs/core/src/agents_md.rs`.
- [ ] When no rules are found, the instruction block is unmodified.

## How it works

- `docs/wiki/systems/user-rules-loader.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/core/src/config/rules.rs` — `load_rules_from_dir`: reads, sorts, trims, and joins `*.md` files from a given directory path.
- `codex-rs/core/src/agents_md.rs` — calls `load_rules_from_dir` with `$CODEX_HOME/rules` and appends the result to the instruction assembly output.
- `codex-rs/core/src/config/mod.rs` — declares the `rules` submodule (`pub(crate) mod rules`).

## Tests asserting this spec

- `codex-rs/core/src/config/rules.rs` — `tests::no_dir_returns_none`
- `codex-rs/core/src/config/rules.rs` — `tests::empty_dir_returns_none`
- `codex-rs/core/src/config/rules.rs` — `tests::loads_md_files_sorted_by_name`
- `codex-rs/core/src/config/rules.rs` — `tests::skips_empty_files`

Run with: `cargo test -p codex-core config::rules`

## Rebase risk

Low. Watch for upstream adding a competing "global rules" feature — if so, migrate rather than duplicate.

## Out of scope

- None noted.
