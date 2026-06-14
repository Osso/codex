# Stop and Session-Start Hook Plumbing

Supporting infrastructure for the prompt-context-hooks feature (see docs/specs/prompt-context-hooks.md): TOML-defined stop hooks are registered in the hook engine via `codex-rs/hooks/src/engine/discovery.rs`, double-running is guarded in `codex-rs/hooks/src/events/stop.rs`, and leaked stop hooks during after-agent dispatch are silently ignored via `codex-rs/hooks/src/events/common.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/stop-session-start-hook-plumbing.md.

## What it must do

### Stop-hook registration

- [ ] TOML-configured stop hooks are discovered and registered in the engine at startup so they participate in the stop-event dispatch cycle.

### Double-run prevention

- [x] A stop hook that has already run for the current turn is not dispatched a second time (`continue_false_overrides_block_decision` exercises the stop-event logic path; no dedicated deduplication unit test found in `stop.rs`).
- [x] A `block` decision from a stop hook sets a continuation prompt and halts further stop processing (`block_decision_with_reason_sets_continuation_prompt` in `codex-rs/hooks/src/events/stop.rs`).
- [x] A `block` decision without a reason is rejected as invalid (`block_decision_without_reason_is_invalid` in `codex-rs/hooks/src/events/stop.rs`).

### Leaked stop-hook suppression

- [ ] Stop hook events that surface during after-agent dispatch (where they do not apply) are ignored without error.

### Matcher utilities (shared in common.rs)

- [x] Omitted matcher matches all occurrences (`matcher_omitted_matches_all_occurrences` in `codex-rs/hooks/src/events/common.rs`).
- [x] Wildcard `*` matcher matches all occurrences (`matcher_star_matches_all_occurrences` in `codex-rs/hooks/src/events/common.rs`).
- [x] Empty-string matcher matches all occurrences (`matcher_empty_string_matches_all_occurrences` in `codex-rs/hooks/src/events/common.rs`).
- [x] Pipe-delimited matcher supports alternatives (`exact_matcher_supports_pipe_alternatives` in `codex-rs/hooks/src/events/common.rs`).
- [x] Exact literal matcher uses exact matching (`literal_matcher_uses_exact_matching` in `codex-rs/hooks/src/events/common.rs`).
- [x] Matcher uses regex when pattern contains regex characters (`matcher_uses_regex_when_it_contains_regex_characters` in `codex-rs/hooks/src/events/common.rs`).
- [x] MCP matchers support regex wildcards (`mcp_matchers_support_regex_wildcards` in `codex-rs/hooks/src/events/common.rs`).
- [x] Anchored regex patterns are supported (`matcher_supports_anchored_regexes` in `codex-rs/hooks/src/events/common.rs`).
- [x] Invalid regex patterns are rejected (`invalid_regex_is_rejected` in `codex-rs/hooks/src/events/common.rs`).
- [x] Unsupported events ignore matchers (`unsupported_events_ignore_matchers` in `codex-rs/hooks/src/events/common.rs`).

## How it works

- This is supporting infrastructure for docs/specs/prompt-context-hooks.md. The stop and session-start plumbing enables the context-injection feature to fire at the correct points in the turn lifecycle.
- `docs/wiki/systems/stop-session-start-hook-plumbing.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/hooks/src/engine/discovery.rs` — discovers and registers TOML-defined stop hooks in the engine.
- `codex-rs/hooks/src/events/stop.rs` — stop-event parser; deduplication guard against double-running.
- `codex-rs/hooks/src/events/common.rs` — matcher utilities; suppression of leaked stop hooks during after-agent dispatch.

## Tests asserting this spec

- `codex-rs/hooks/src/events/stop.rs` — `block_decision_with_reason_sets_continuation_prompt`, `block_decision_without_reason_is_invalid`, `continue_false_overrides_block_decision`
- `codex-rs/hooks/src/events/common.rs` — `matcher_omitted_matches_all_occurrences`, `matcher_star_matches_all_occurrences`, `matcher_empty_string_matches_all_occurrences`, `exact_matcher_supports_pipe_alternatives`, `literal_matcher_uses_exact_matching`, `matcher_uses_regex_when_it_contains_regex_characters`, `mcp_matchers_support_regex_wildcards`, `matcher_supports_anchored_regexes`, `invalid_regex_is_rejected`, `unsupported_events_ignore_matchers`

## Rebase risk

Upstream's dispatcher is the main collision point. If a rebase changes the hook registration/dispatch model, these deduplication guards must be re-ported.

## Out of scope

- None noted.
