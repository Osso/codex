# Subagent / inter-agent communication

This spec covers v2 multi-agent UX polish: how the TUI surfaces agent-to-agent
traffic as readable cells, how session plumbing drives reliable delivery and
early-exit semantics, and how the keyboard lets users navigate and close agent
threads without leaving the primary flow. The primary source lives in
`codex-rs/tui/src/inter_agent_message.rs` (pretty-print rendering) and the
`multi_agents_v2` handlers under
`codex-rs/core/src/tools/handlers/multi_agents_v2/` (wait, spawn, message
tool). See `docs/osso_fork.md` for the fork-divergence index; how it works
belongs in `docs/wiki/systems/subagent-communication.md`.

## What it must do

### Rendering

- [x] `InterAgentCommunication` JSON blobs embedded in assistant messages are
  parsed and rendered as a compact `author → recipient` cell instead of raw
  JSON (`pretty_prints_plain_inter_agent_message`,
  `pretty_prints_subagent_notification` in `inter_agent_message.rs`).
- [x] JSON that is not an `InterAgentCommunication` produces no rendered cell —
  `pretty_inter_agent_message` returns `None`
  (`returns_none_for_non_communication_json`).

### Turn plumbing

- [ ] Subagent notifications mark themselves as trigger turns so that
  truncation logic counts them as turn boundaries (trigger-turn propagation in
  `codex-rs/core/src/session/turn.rs`).

### wait_agent semantics

- [x] `wait_agent` returns immediately with a "No agents available" result when
  no descendant agents exist, rather than blocking until timeout
  (`no_agents` branch in `codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs`).
- [ ] `wait_agent` reports active descendant status across nested agents (not
  only direct children) via the `descendant_prefix` filter
  (`multi_agent_v2_wait_agent_accepts_timeout_only_argument` covers the API
  surface but no named test asserts nested depth specifically).

### Spawn delivery

- [ ] Spawned agents receive their initial task exactly once, including when
  the first turn is delivered through queued task plumbing rather than an
  already-running agent loop (`codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs`
  — no named test currently asserts the once-only guarantee).

### Agent navigation

- [ ] Alt+1 switches to the main thread; Alt+2–9 switch to live agents in
  spawn order, consistent with `AgentNavigationState` slot ordering
  (`slot_thread_id_maps_slot_one_to_primary_thread`,
  `slot_thread_id_maps_later_slots_to_agents_in_spawn_order` in
  `agent_navigation.rs` assert the state machine, but no named test asserts the
  key-event dispatch path in `app/input.rs`).
- [x] Closed or `NotLoaded` agent threads are pruned from `/agent` navigation,
  adjacent-thread cycling, and direct slot shortcuts
  (`remove_drops_thread_from_direct_slots` in `agent_navigation.rs`).

### Agent-local close

- [x] Pressing Ctrl+D while focused on an agent thread closes that agent and
  returns focus to its parent or the main thread; Ctrl+D on the main thread
  keeps the normal quit behavior (`close_agent_shortcut_matches_ctrl_d` in
  `platform_actions.rs`).

## How it works

- `docs/wiki/systems/subagent-communication.md` (stub — not yet written).
- `docs/specs/multi-agent-v2.md` — the underlying v2 tool-surface contract
  this polishes.
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/tui/src/inter_agent_message.rs` — self-contained parsing and
  pretty-printing of `InterAgentCommunication` and subagent-notification JSON.
- `codex-rs/tui/src/chatwidget.rs` — integration point: detects inter-agent
  blobs in assistant turns and routes them through the renderer.
- `codex-rs/tui/src/app/agent_navigation.rs` — `AgentNavigationState`: ordered
  thread registry, slot mapping, adjacent-thread cycling, and pruning of closed
  threads.
- `codex-rs/tui/src/app/input.rs` — key-event dispatch for Alt+1–9 agent
  navigation and Ctrl+D close-agent shortcut.
- `codex-rs/tui/src/app/platform_actions.rs` — `close_agent_shortcut_matches`
  predicate and `maybe_close_active_agent_thread` action.
- `codex-rs/tui/src/app/session_lifecycle.rs` — thread startup/teardown hooks
  that update `AgentNavigationState`.
- `codex-rs/tui/src/app/side.rs` — side-pane rendering that reflects current
  agent nav state.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs` — `wait_agent`
  handler: early-return on empty descendants, descendant-prefix filtering,
  timeout loop, and reap-on-exit.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs` — `spawn_agent`
  handler: task delivery, fork-mode resolution, model/reasoning inheritance.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs` —
  `send_message` / `followup_task` routing between threads.
- `codex-rs/core/src/session/turn.rs` — turn-boundary and trigger-turn
  propagation consumed by compaction.
- `codex-rs/core/src/state/session.rs` — session-level thread registry and
  agent-status lookups used by wait and navigation.

## Tests asserting this spec

- `codex-rs/tui/src/inter_agent_message.rs` (inline tests):
  `pretty_prints_subagent_notification`,
  `pretty_prints_plain_inter_agent_message`,
  `returns_none_for_non_communication_json`
  — run via `cargo test -p codex-tui inter_agent_message`.
- `codex-rs/tui/src/app/agent_navigation.rs` (inline tests):
  `remove_drops_thread_from_direct_slots`,
  `slot_thread_id_maps_slot_one_to_primary_thread`,
  `slot_thread_id_maps_later_slots_to_agents_in_spawn_order`,
  `adjacent_thread_id_wraps_in_spawn_order`
  — run via `cargo test -p codex-tui`.
- `codex-rs/tui/src/app/platform_actions.rs` (inline test):
  `close_agent_shortcut_matches_ctrl_d`
  — run via `cargo test -p codex-tui`.
- `codex-rs/core/src/tools/handlers/multi_agents_tests.rs`:
  `multi_agent_v2_wait_agent_accepts_timeout_only_argument`,
  `multi_agent_v2_wait_agent_returns_summary_for_mailbox_activity`,
  `multi_agent_v2_list_agents_omits_closed_agents`
  — run via `cargo test -p codex-core tools::handlers::multi_agents_v2`.
- `codex-rs/core/tests/suite/subagent_notifications.rs`:
  `subagent_notification_is_included_without_wait`,
  `spawned_child_receives_forked_parent_context`
  — run via `cargo test -p codex-core suite::subagent_notifications`.
- Snapshot: `collab_agent_transcript.snap` — currently a `.snap.new` on this
  branch; review and accept with `cargo insta review` before rebase.

## Rebase risk

Medium. The `inter_agent_message.rs` module is self-contained and should
survive upstream merges without conflict. `chatwidget.rs` and the
`wait`/`message` handlers overlap with ongoing upstream multi-agent work and
are the most likely collision points. Resolve by rebasing the inter-agent
rendering hook in `chatwidget.rs` against upstream's version of the same
assistant-message dispatch loop, and re-verify the `wait_agent` early-return
path if upstream changes descendant-enumeration logic.

## Out of scope

- None noted.
