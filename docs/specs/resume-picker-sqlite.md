# Resume Picker SQLite-First Listing

The resume picker SQLite-first listing keeps `codex resume` startup latency proportional to a SQLite row query instead of parsing JSONL rollout heads for every displayed row. Because a thread's first user message and preview are immutable once captured, the picker trusts state DB metadata for initial row rendering and reserves JSONL reads for fallback, repair, transcript preview, and full session resume of the selected thread. The feature lives in `codex-rs/thread-store/src/local/list_threads.rs`, `codex-rs/rollout/src/state_db.rs`, `codex-rs/rollout/src/recorder.rs`, and `codex-rs/tui/src/resume_picker.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/resume-picker-sqlite.md.

## What it must do

### SQLite-first listing

- [x] Normal local thread listing queries SQLite first even when the caller did not set `use_state_db_only`.
- [x] If SQLite returns a non-empty page, a cursor page, or the caller explicitly requested state-DB-only behavior, the thread-store returns that page without scanning rollout heads.
- [x] If SQLite cannot answer or returns an empty first page, the existing rollout scan/repair path is used as a fallback, keeping older or incomplete stores functional.
- [x] SQLite-supplied preview text is used for picker rows without requiring a rollout head scan.

### Fast targeted lookup

- [ ] `codex resume --last` remains a fast targeted lookup unaffected by this change.
- [ ] Plain `codex resume` no longer parses early JSONL lines for every picker row when state DB metadata exists.

### Correctness constraints

- [x] SQLite title search results are preserved (not silently dropped by the fallback path).
- [x] Active/archived collection selection is respected when the state DB answers the query.
- [ ] Full selected-session resume still loads the complete rollout history for the chosen thread; this feature only covers picker/list discovery latency.

## How it works

- `docs/wiki/systems/resume-picker-sqlite.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry (§19).

## Implementation inventory

- `codex-rs/thread-store/src/local/list_threads.rs` — `list_threads` entry point; `list_state_db_threads` fast path; `should_return_state_db_page` predicate; `list_rollout_threads` retained as fallback.
- `codex-rs/rollout/src/state_db.rs` — SQLite metadata store; provides thread rows for picker without JSONL access.
- `codex-rs/rollout/src/recorder.rs` — writes immutable first-message and preview metadata into the state DB at record time.
- `codex-rs/tui/src/resume_picker.rs` — picker UI; renders rows from thread-store; scroll/footer/progress helpers.

## Tests asserting this spec

- `codex-rs/thread-store/src/local/list_threads.rs` — `list_threads_uses_sqlite_preview_without_rollout_head_scan`, `list_threads_preserves_sqlite_title_search_results`, `list_threads_selects_active_or_archived_collection`, `list_threads_uses_default_provider_when_rollout_omits_provider`, `list_threads_returns_local_rollout_summary`, `list_threads_rejects_invalid_cursor`

Run: `cargo test -p codex-thread-store local::list_threads` and `cargo test -p codex-tui resume_picker`.

## Rebase risk

Medium. Upstream may keep the rollout recorder filesystem-first repair policy. Preserve the thread-store boundary behavior: picker/list callers must get DB-backed rows first, and rollout head parsing must be a fallback or lazy visible-row operation — not the initial rendering path.

## Out of scope

- Full selected-session resume behavior (this covers picker/list discovery latency only).
- Remote thread listing or provider-side pagination changes.
