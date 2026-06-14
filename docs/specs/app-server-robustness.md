# App-server & MCP robustness fixes

This spec covers a cluster of six independent bug fixes to the app-server and MCP layers carried by the osso fork. The fixes address notification backpressure, auth staleness, a debug-only login-issuer escape hatch, TUI status-timer accuracy, runtime feature-flag lookup for goal RPCs, and MCP startup completion forwarding. Primary source lives in `codex-rs/app-server` and `codex-rs/app-server-client`; TUI-side changes touch `codex-rs/tui` and `codex-rs/core`. See `docs/osso_fork.md` for the fork-divergence index; how it works belongs in `docs/wiki/systems/app-server-robustness.md`.

## What it must do

### MCP startup notifications under backpressure
- [ ] When the app-server client is slow to drain its receive buffer, MCP startup status notifications are queued rather than dropped.
- [ ] No startup notification is silently lost when the client channel is at capacity.

### Auth reload before MCP account reads
- [x] App-server reloads auth state before processing MCP account reads, so a freshly logged-in token is visible without a restart.
- [x] Account read after a login cycle returns the new account data (asserted by `codex-rs/app-server/tests/suite/auth.rs` and `codex-rs/app-server/tests/suite/v2/account.rs`).

### Login issuer override gated to debug builds
- [ ] The app-server login issuer override is accepted only in debug builds; release builds reject or ignore the override.
- [ ] Production/release builds cannot be directed to trust a test issuer via the override path.

### TUI status timer reset on MCP startup
- [ ] When MCP startup begins while another task is already running, the bottom-pane status row restarts its elapsed timer from zero rather than continuing the previous task's elapsed time.
- [ ] Timing-only runtime metrics (no non-timing fields populated) are no longer treated as empty, so the timer is correctly reported.

### Runtime goal feature enablement in app-server goal RPCs
- [x] `thread/goal/{get,set,clear}` resolves the current feature-flag state via `ConfigManager` rather than the startup `Config` snapshot, so `/experimental goal` toggled after startup is honored.
- [x] A test asserts that `thread/goal/get` succeeds after goal feature is enabled post-startup (`codex-rs/app-server/tests/suite/v2/thread_resume.rs` — `thread_goal_get_honors_goal_feature_enabled_after_startup`).

### MCP startup completion summaries forwarded through app-server
- [x] After core emits `McpStartupComplete` and the startup join-set settles, app-server converts the summary into terminal `mcpServer/startupStatus/updated` notifications so the TUI clears "Starting MCP servers ..." even when an individual final update was dropped or filtered.
- [x] Regression coverage in `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs`.

## How it works

- `docs/wiki/systems/app-server-robustness.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/app-server-client/src/lib.rs` — client-side channel and drain logic; backpressure queuing for startup notifications.
- `codex-rs/app-server-client/src/remote.rs` — remote transport; coordinates notification delivery under slow drain.
- `codex-rs/app-server/src/in_process.rs` — in-process transport; mirrors backpressure fix for embedded use.
- `codex-rs/app-server/src/message_processor.rs` — handles auth reload before account reads; contains login issuer override gating; routes goal RPCs to `ConfigManager` for live feature resolution.
- `codex-rs/app-server/src/request_processors/thread_goal_processor.rs` — goal RPC processor; reads feature enablement from `ConfigManager` instead of startup snapshot.
- `codex-rs/app-server/src/bespoke_event_handling.rs` — converts `McpStartupComplete` core event into terminal `mcpServer/startupStatus/updated` notifications.
- `codex-rs/tui/src/status_indicator_widget.rs` — bottom-pane timer widget; reset logic for MCP startup rounds.
- `codex-rs/tui/src/bottom_pane/mod.rs` — bottom pane orchestration; passes timer reset signal on MCP startup.
- `codex-rs/tui/src/chatwidget.rs` — top-level TUI event routing; triggers timer reset when MCP startup begins.
- `codex-rs/core/src/telemetry.rs` — runtime metrics; timing-only metrics no longer evaluated as empty.

## Tests asserting this spec

- `codex-rs/app-server/tests/suite/auth.rs` — auth reload before account reads.
- `codex-rs/app-server/tests/suite/v2/account.rs` — account read after login cycle.
- `codex-rs/app-server/tests/suite/v2/thread_resume.rs::thread_goal_get_honors_goal_feature_enabled_after_startup` — runtime goal feature enablement.
- `codex-rs/tui/src/chatwidget/tests/mcp_startup.rs` — MCP startup completion forwarding through app-server.
- `runtime_metrics_summary_is_not_empty_when_only_timing_fields_are_set` — named test not yet found in codebase (test may be planned but not yet written).

## Rebase risk

Upstream owns `codex-rs/app-server/src/message_processor.rs`, `codex-rs/app-server/src/bespoke_event_handling.rs`, and the app-server-client transport files heavily. Carry forward as surgical hunks; watch for upstream notification-pipeline refactors and auth-flow changes that may conflict. Risk level: **Medium**.

## Out of scope

- None noted.
