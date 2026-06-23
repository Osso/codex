# Approval System

The approval system controls when Codex asks before running tools, who reviews
those requests, and how no-prompt modes behave. The core contract lives in
`codex-rs/protocol/src/protocol.rs`, shared approval presets live in
`codex-rs/utils/approval-presets`, and approval execution is coordinated by
`codex-rs/core/src/tools/orchestrator.rs`. Implementation details belong in
`docs/wiki/systems/approval-system.md`.

## What it must do

### Approval policies

- [ ] Support normal ask mode as `on-request`, where approval-required actions
  are routed to the configured approval reviewer.
- [ ] Support no-prompt reject mode as `never`, where approval-required actions
  are rejected or returned to the model instead of being shown to any reviewer.
- [ ] Support no-prompt approve mode as `auto-approve`, where
  approval-required actions are treated as approved without user or LLM review.
- [ ] Keep `never` and `auto-approve` distinct on the wire, in config, in CLI
  parsing, in app-server protocol types, and in TUI status/history surfaces.
- [ ] Map `--dangerously-bypass-approvals-and-sandbox` to disabled
  sandbox/full access only; it must not change the configured approval mode.

### Approval reviewers

- [ ] Support user-reviewed approvals for `on-request`.
- [ ] Support LLM-approved approvals as `on-request` with the approval reviewer
  set to the auto-reviewer/guardian path.
- [x] Show LLM-approved mode as an explicit `/approvals` choice, separate from
  both `never` and `auto-approve`.
- [ ] Do not run the LLM-approved reviewer when an action has already been
  approved by a hook, cached approval, or explicit policy decision.

### Presets and UI

- [x] Provide `/approvals` as a slash command that opens the approval preset
  selector.
- [x] Provide `/sandbox` as a slash command that opens the sandbox/profile
  selector without changing approval behavior.
- [x] Remove `/permissions` from the slash-command surface so approval behavior
  and sandbox access are not presented as one combined mode.
- [x] Include at least these user-visible choices in the approval selector:
  Ask Me, LLM Approved, Never Ask/Deny, and Auto Approve.
- [x] Include at least these user-visible choices in the sandbox selector:
  Read Only where supported, Default/Workspace Write, and Full Access.
- [x] Avoid labels that make `never` sound like approval by default.

### Approval and sandbox matrix

For shell actions that need permissions beyond the active sandbox, Codex must
take this decision:

| `/sandbox` mode         | Ask Me    | LLM Approved    | Never Ask/Deny           | Auto Approve    |
| ----------------------- | --------- | --------------- | ------------------------ | --------------- |
| Read Only               | Ask user. | Ask LLM.        | Reject without prompt.   | Run as approved. |
| Default/Workspace Write | Ask user. | Ask LLM.        | Reject without prompt.   | Run as approved. |
| Full Access             | Run.      | Run.            | Run.                     | Run.            |

- [x] Test the full matrix above in core sandbox approval tests.
- [x] Test that the LLM Approved selector maps to the same core approval
  requirement as Ask Me while setting the auto-reviewer.
- [x] Test that `--dangerously-bypass-approvals-and-sandbox` only changes the
  sandbox decision to Full Access and does not change `/approvals`.

### Hooks and external approval engines

- [ ] Preserve `claude-bash-hook` as a rule/preclassification engine that can
  return allow, ask, or deny decisions before Codex prompts.
- [ ] Treat hook allow decisions as already approved so Codex does not ask a
  human or run the LLM-approved reviewer again.
- [ ] Ensure hook compatibility never maps Codex `never` to Claude
  `bypassPermissions`.
- [ ] Allow hook compatibility to map Codex `auto-approve` to
  `bypassPermissions`.
- [x] Treat Bash hook passthrough as "defer to Codex approval": Ask Me and LLM
  Approved require approval, Never Ask/Deny rejects, and Auto Approve runs
  without prompting.

## How it works

- See `docs/wiki/systems/approval-system.md` for approval flow internals.
- See `docs/specs/permission-prompt-tool.md` for the MCP permission prompt tool
  contract.

## Implementation inventory

- `codex-rs/protocol/src/protocol.rs` — core `AskForApproval` wire/config enum.
- `codex-rs/app-server-protocol/src/protocol/v2/shared.rs` — app-server v2
  mirror of the approval enum.
- `codex-rs/utils/cli/src/approval_mode_cli_arg.rs` — CLI approval-policy
  values.
- `codex-rs/utils/approval-presets/src/lib.rs` — shared built-in approval and
  permission presets.
- `codex-rs/cli/src/main.rs` — top-level CLI flag conflict handling and
  dangerous-bypass override mapping.
- `codex-rs/core/src/tools/orchestrator.rs` — central approval, reviewer,
  sandbox attempt, and retry flow.
- `codex-rs/core/src/tools/sandboxing.rs` — approval requirement model and
  pre-tool hook approval application.
- `codex-rs/core/src/hook_runtime.rs` and `codex-rs/core/src/session/turn.rs`
  — hook permission-mode compatibility decisions, including Bash passthrough
  handling.
- `codex-rs/core/src/exec_policy.rs` — command policy decisions and prompt
  rejection semantics.
- `codex-rs/core/src/tools/runtimes/shell/unix_escalation.rs` — shell
  escalation prompt handling.
- `codex-rs/tui/src/slash_command.rs` — `/approvals` and `/sandbox` command
  registration.
- `codex-rs/tui/src/chatwidget/slash_dispatch.rs` — slash-command dispatch to
  the approvals and sandbox popups.
- `codex-rs/tui/src/chatwidget.rs` — approval and sandbox selector rendering
  and selection actions.
- `/syncthing/Sync/Projects/claude/claude-bash-hook/src/main.rs` — hook-side
  Codex approval-policy compatibility.
- `/syncthing/Sync/Projects/claude/claude-bash-hook/src/tool_handlers.rs` —
  hook-side edit-mode decisions for non-Bash tools.

## Tests asserting this spec

- `codex-rs/core/src/exec_policy_tests.rs`
- `codex-rs/core/src/hook_runtime.rs`
- `codex-rs/core/src/tools/sandboxing_tests.rs`
- `codex-rs/core/src/tools/runtimes/shell/unix_escalation_tests.rs`
- `codex-rs/tui/src/chatwidget/tests/permissions.rs`
- `codex-rs/tui/src/slash_command.rs`
- `/syncthing/Sync/Projects/claude/claude-bash-hook/src/access_mode_tests.rs`

## Known gaps (current cycle)

- [x] Add or update tests for `auto-approve` vs `never` core behavior.
- [x] Add or update tests for `/approvals` and the explicit LLM Approved preset.
- [ ] Add or update tests proving hook-approved actions skip LLM-approved review.
- [ ] Regenerate config, app-server, hook, and TUI snapshot fixtures after the
  final approval contract is implemented.

## Rebase risk

**HIGH.** The approval contract spans the core `AskForApproval` wire/config enum, the app-server v2 mirror, CLI approval-policy parsing, the shared approval presets, the orchestrator approval/reviewer/sandbox flow, and the `claude-bash-hook` compatibility layer — all surfaces upstream edits frequently. After a rebase, re-verify the `never` vs `auto-approve` split survives across protocol, config, CLI, app-server, and TUI; that `--dangerously-bypass-approvals-and-sandbox` only disables sandboxing and does not rewrite approval mode; and that hook `allow` decisions still short-circuit both the human prompt and the LLM-approved reviewer. Related: `docs/specs/permission-prompt-tool.md` and `docs/specs/pre-tool-use-rewrites.md`.

## Out of scope

- Replacing the hook rule engine with Codex-native rule matching.
- Changing the internal name of the auto-reviewer/guardian subsystem; this spec
  only requires the user-facing approval preset to be understandable.
