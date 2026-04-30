# `--permission-prompt-tool` (MCP-delegated approval)

Lets Codex delegate "approve this tool call?" decisions to an external MCP
tool, matching Claude Code's `--permission-prompt-tool` wire protocol so a
single MCP server (e.g. `claude-bash-hook-approval`) works with both
harnesses. Source lives in `codex-rs/core/src/permission_prompt.rs` and
`codex-rs/codex-mcp/src/permission_prompt.rs`. Wire-shape reference:
`claude-agent-sdk-python` →
`src/claude_agent_sdk/_internal/query.py::PermissionResultAllow` and
`src/claude_agent_sdk/types.py::PermissionUpdate`.

## What it must do

### CLI / config surface

- [x] Accept `--permission-prompt-tool <mcp__server__tool>` on `codex` and `codex exec`.
- [x] Accept `permission_prompt_tool` in `~/.codex/config.toml` and via `-c permission_prompt_tool=...` overrides.
- [x] Thread the configured tool through `SessionConfiguration` so it's part of the per-session snapshot.

### Approval loop

- [x] When `AskForApproval::OnRequest` would prompt the user for a shell/exec call AND a permission prompt tool is configured, call the configured MCP tool first and honor its decision.
- [x] No tool configured → fall through to the existing interactive prompt.
- [x] Invalid tool name (not in `mcp__server__tool` shape) → fall through to the interactive prompt.
- [x] Tool errors / timeouts / connection failures → fall through to the interactive prompt.
- [x] Malformed tool response (missing `behavior`, non-JSON, etc.) → fall through to the interactive prompt.

### Decision shapes

- [x] `{"behavior":"allow"}` → command runs without further prompting.
- [x] `{"behavior":"allow","updatedInput":{...}}` → command runs with the rewritten input from `updatedInput`.
- [x] `{"behavior":"deny","message":"..."}` → command is rejected with the supplied reason.
- [x] `updatedPermissions:[{type:"addRules",destination,behavior,rules:[...]}]` is honored when present alongside the decision.

### Rule persistence

- [x] `destination:"session"` + `behavior:"allow"` rule suppresses future prompts for the matching `{toolName, ruleContent}` in the same run.
- [x] `destination:"userSettings"` writes the rule to `~/.codex/config.toml` and suppresses future prompts (across sessions).
- [x] `destination:"projectSettings"` writes to repo-root `.codex/config.toml`.
- [x] `destination:"localSettings"` writes to `.codex/config.local.toml`.
- [x] Persisting via `toml_edit` preserves existing TOML formatting (no full reserialization).
- [x] Non-session rules do NOT suppress in-memory follow-up prompts unless they ALSO match the persisted-rules check.

### Hook import

- [x] `codex hooks import-claude` reads `permissionPromptTool` from the imported Claude settings and writes it to the Codex config.

## How it works

- `docs/wiki/systems/permission-prompt-tool.md` (stub — not yet written).
- `docs/osso_fork.md` §1 covers the rebase-risk surface and the fork-divergence rationale.

## Implementation inventory

- `codex-rs/cli/src/main.rs` — CLI flag plumbing, `Settings` struct field, TOML overlay (`doc["permission_prompt_tool"]`), threading from `InteractiveArgs`/`ExecArgs` to session config.
- `codex-rs/utils/cli/src/shared_options.rs` — shared `--permission-prompt-tool` clap option.
- `codex-rs/exec/src/cli.rs` — exec-subcommand exposure.
- `codex-rs/exec/src/lib.rs` — exec dispatch picks up the option.
- `codex-rs/core/src/codex_thread.rs` — `permission_prompt_tool: Option<String>` on the session config struct.
- `codex-rs/core/src/config/mod.rs` — top-level config field + ConfigToml parsing + `SessionConfiguration` mapping.
- `codex-rs/core/src/config/edit.rs` — TOML-edit-based writer for persistent rule destinations.
- `codex-rs/core/src/permission_prompt.rs` — main module: input building, approval-loop entry point (`maybe_decide_command_approval_with_permission_prompt_tool`), session-rule cache, persistent-rule application, destination-aware writers.
- `codex-rs/codex-mcp/src/permission_prompt.rs` — MCP tool contract (input/output JSON shape, response parsing, decision body validation).
- `codex-rs/codex-mcp/src/lib.rs` — exports the contract.
- `codex-rs/core/src/session/{mod.rs, session.rs}` — session-side wiring of the approval loop into the shell/exec dispatch.
- `codex-rs/core/src/state/service.rs` — state plumbing for in-memory rule cache.
- `codex-rs/app-server/src/codex_message_processor.rs` — exposes the option on the app-server side.
- `codex-rs/tui/src/lib.rs` — TUI configuration touchpoint.

## Tests asserting this spec

- `codex-rs/core/src/permission_prompt.rs` (inline `mod tests`):
  - `no_configured_tool_returns_none`
  - `invalid_configured_tool_returns_none`
  - `allow_decision_returns_approved`
  - `allow_decision_accepts_updated_input_contract`
  - `deny_decision_returns_denied`
  - `call_tool_errors_fall_back_to_interactive_prompt`
  - `malformed_tool_response_falls_back_to_interactive_prompt`
  - `session_allow_rule_suppresses_followup_prompt`
  - `non_session_rule_does_not_suppress_followup_prompt`
  - `user_settings_rule_persists_and_suppresses_followup_prompt`
  - `project_and_local_settings_rules_persist_to_repo_codex_files`
- `codex-rs/core/src/session/tests.rs`:
  - `thread_config_snapshot_includes_permission_prompt_tool`
  - `request_command_approval_permission_prompt_tool_decisions_and_side_effects` (integration: spins up a stub MCP server and drives a shell-tool call through every decision shape)
- `codex-rs/core/src/config/edit_tests.rs`:
  - `blocking_replace_mcp_servers_serializes_tool_approval_overrides`
- `codex-rs/exec/src/cli_tests.rs`:
  - `parses_permission_prompt_tool_flag`

## Known gaps (current cycle)

None — feature is shipped (commit `531e371da6 Add MCP permission prompt approval flow`).

## Out of scope

- Reimplementing Claude Code's full rule-matching syntax. Current implementation does exact `ruleContent` equality only; globbing/regex matching can land later.
- A Codex-native TUI for the approval modal. The existing interactive prompt is the fallback when no MCP tool is configured.
