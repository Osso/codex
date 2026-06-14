# Session-End Transcript Parser

Hook binaries that need the session transcript path (for post-run summarization, auditing, etc.) receive it via JSON on stdin. This fork ships a tested parser `session_end_transcript_path_from_json` in `codex-rs/hooks/src/session_end.rs` plus a thin CLI binary `session_end_transcript_path` at `codex-rs/hooks/src/bin/session_end_transcript_path.rs` that prints the path to stdout, so shell hooks can read it with a single invocation and no `jq` dependency. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/session-end-transcript-parser.md.

## What it must do

### Path extraction

- [x] Parses the nested `hook_event.transcript_path` field (new format) and returns it (`reads_codex_nested_transcript_path` in `codex-rs/hooks/src/session_end.rs`).
- [x] Falls back to the top-level `transcript_path` field (legacy format) when the nested path is absent (`reads_legacy_top_level_transcript_path` in `codex-rs/hooks/src/session_end.rs`).
- [x] When both fields are present, the top-level `transcript_path` takes precedence (`prefers_top_level_transcript_path_when_both_exist` in `codex-rs/hooks/src/session_end.rs`).
- [x] Returns `None` when neither field is present (`returns_none_for_missing_transcript_path` in `codex-rs/hooks/src/session_end.rs`).
- [x] Returns `None` for invalid or truncated JSON input (`returns_none_for_invalid_json` in `codex-rs/hooks/src/session_end.rs`).

### CLI binary

- [ ] The `session_end_transcript_path` binary reads stdin, calls the parser, and prints the transcript path to stdout (or exits silently if not found), enabling jq-free shell hook use.

## How it works

- `docs/wiki/systems/session-end-transcript-parser.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/hooks/src/session_end.rs` — `session_end_transcript_path_from_json` parser with full test suite.
- `codex-rs/hooks/src/bin/session_end_transcript_path.rs` — CLI binary; thin wrapper around the parser.
- `codex-rs/hooks/src/lib.rs` — re-exports the parser so it is accessible to external consumers.

## Tests asserting this spec

- `codex-rs/hooks/src/session_end.rs` — `reads_legacy_top_level_transcript_path`, `reads_codex_nested_transcript_path`, `prefers_top_level_transcript_path_when_both_exist`, `returns_none_for_missing_transcript_path`, `returns_none_for_invalid_json`

## Rebase risk

Low — pure additive crate surface. Just make sure the binary target still resolves in `codex-rs/hooks/Cargo.toml` after any workspace reorganization.

## Out of scope

- None noted.
