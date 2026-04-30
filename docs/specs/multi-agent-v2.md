# MultiAgentV2 (sub-agent collaboration tools)

The shipped multi-agent tool surface that lets a parent agent spawn,
message, list, wait on, and close child agents. Source lives in
`codex-rs/core/src/tools/handlers/multi_agents_v2/`,
`codex-rs/core/src/tools/handlers/multi_agents_common.rs`, and
`codex-rs/tools/src/agent_tool.rs`. Gated behind
`Feature::MultiAgentV2` (Stage::Stable, default-enabled).

## What it must do

### Tool surface (six tools)

- [x] `spawn_agent` — spawn a child with a `task_name`, optional fork
      mode, and optional agent-type/model overrides; returns the
      child's canonical agent path.
- [x] `send_message` — send a message to an existing child or to root.
- [x] `followup_task` — assign a follow-up task; interrupts a busy
      child without losing the message.
- [x] `wait_agent` — block until mailbox activity / timeout; returns
      a summary of changed senders.
- [x] `list_agents` — enumerate live agents with status + last task
      message; supports relative-path-prefix filtering.
- [x] `close_agent` — terminate a child by task-name target.

### Spawn semantics

- [x] Requires a `task_name`.
- [x] `fork_turns` accepts `none` / `all` / positive integer string.
      Rejects invalid strings and zero.
- [x] `fork_turns:all` is the default and rejects child model
      overrides.
- [x] `fork_turns:none` and partial-fork variants allow agent-type
      overrides.
- [x] Rejects the legacy v1 `items` field and the legacy
      `fork_context` field.
- [x] Returns the child's path; `send_message` accepts that path
      verbatim or as a relative form.
- [x] Omits `agent_id` from the response when the child is named.
- [x] Surfaces task-name validation errors back to the model.
- [x] Build config preserves turn-context values and base user
      instructions; resume builds clear base instructions.

### Message routing

- [x] `send_message` accepts a root target from a child.
- [x] `send_message` rejects the legacy `items` field and an
      `interrupt` parameter.
- [x] `followup_task` rejects a root target from a child.
- [x] `followup_task` interrupts a busy child without losing the
      message.
- [x] `followup_task` completion notifies the parent on every turn.
- [x] `followup_task` rejects the legacy `items` field.
- [x] An interrupted turn does NOT notify the parent.

### List

- [x] Returns completed status and the last task message per agent.
- [x] Filters by relative path prefix.
- [x] Omits closed agents.

### Wait

- [x] Accepts a timeout-only argument.
- [x] Returns "no agents" without waiting when there are none.
- [x] Returns a summary for mailbox activity.
- [x] Returns immediately for mail already queued.
- [x] Wakes on any mailbox notification.
- [x] Does NOT return completed content (live signal only).

### Close

- [x] Accepts a task-name target.
- [x] Rejects a root target and a raw `agent_id`.

## How it works

- `docs/wiki/systems/multi-agent-v2.md` (stub — not yet written).
- Fork-turn boundaries are described in
  `codex-rs/core/src/thread_rollout_truncation.rs`.

## Implementation inventory

- `codex-rs/tools/src/agent_tool.rs` — tool spec constructors:
  `create_spawn_agent_tool_v2`, `create_send_message_tool`,
  `create_followup_task_tool`, `create_wait_agent_tool_v2`,
  `create_list_agents_tool`, `create_close_agent_tool_v2`.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/spawn.rs` — spawn handler, fork-mode validation, child-config build.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/send_message.rs` — direct mailbox messages.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/followup_task.rs` — interrupt-aware task delivery.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/wait.rs` — mailbox wait + summary.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/list_agents.rs` — agent enumeration + filter.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/close_agent.rs` — close-by-task-name.
- `codex-rs/core/src/tools/handlers/multi_agents_v2/message_tool.rs` — shared message-tool plumbing.
- `codex-rs/core/src/tools/handlers/multi_agents_common.rs` — shared helpers (`build_agent_spawn_config`, `build_agent_resume_config`, `build_wait_agent_statuses`, `parse_collab_input`, error mapping).
- `codex-rs/core/src/agent/control.rs` — `AgentControl::list_agents`, `spawn_forked_thread`, mailbox routing.
- `codex-rs/core/src/thread_rollout_truncation.rs` — fork-turn boundary detection (`truncate_rollout_to_last_n_fork_turns`).

## Tests asserting this spec

- `codex-rs/core/src/tools/handlers/multi_agents_tests.rs` — 30+ inline tests prefixed `multi_agent_v2_*` covering every bullet above plus `build_agent_spawn_config_*` and `build_agent_resume_config_*` helpers.
- `codex-rs/core/tests/suite/spawn_agent_description.rs` — tool-description rendering.
- `codex-rs/core/tests/suite/subagent_notifications.rs` — parent-side notification semantics.
- `codex-rs/core/tests/suite/hierarchical_agents.rs` — multi-level nesting.
- `codex-rs/core/tests/suite/agents_md.rs` — child AGENTS.md inheritance.
- `codex-rs/core/tests/suite/agent_jobs.rs` — agent_jobs (`spawn_agents_on_csv`) integration with v2 spawn.

## Known gaps (current cycle)

These are improvements promoted from PLAN.md; they are not yet shipped.

- [ ] Rename or alias `fork_turns` to a clearer input
      (`fork_history` / `history_scope`), keeping `fork_turns`
      accepted for back-compat. Update tool descriptions to make it
      explicit this controls copied parent context, not whether the
      child starts a turn.
- [ ] Add `status_filter` to `list_agents`
      (`running` / `completed` / `failed` / `non_final` / `all`) and
      back it with tests.
- [ ] Add a user-facing `/agents` (or `/list-agents`) slash command
      wired to the same registry data, showing path / nickname / role
      / status / last task message, with a non-final filter option.
- [ ] Extend `wait_agent` output to include changed mailbox senders,
      descendant status counts, and `all_descendants_final`. Keep the
      existing `message`/`timed_out` fields for back-compat.
- [ ] Add a `verify_agent` (or `spawn_agent(require_output=true)`)
      path that marks an agent failed if it completes without running
      a requested command or returning a structured PASS/FAIL.
- [ ] Improve spawned-agent instruction framing for `fork_turns:none`
      so the initial task message is visually distinct from repo
      bootstrapping/rule context. Add a regression test where the
      child must execute a command rather than merely acknowledge
      AGENTS.md.
- [ ] Add a parent-side warning when a child completes with an empty
      or generic acknowledgement final answer while its initial task
      asked for concrete evidence.
- [ ] Make rebase/commit helper paths set `GIT_EDITOR=true` (and
      `GIT_SEQUENCE_EDITOR=true` where appropriate) for
      non-interactive operations so automated `git rebase --continue`
      cannot open an editor.

## Out of scope

- Reviving the deprecated v1 multi-agent tool surface. v1 was
  removed in commit `583ed3144a`. Any compat work targets v2's
  protocol, not a re-introduction of v1 names.

## Open questions

- Should `list_agents` default to `non_final` for model-facing
  calls, or keep `all` as default and add a separate
  `list_running_agents` helper? Current bias: `all` for
  back-compat plus an explicit `status_filter`.
