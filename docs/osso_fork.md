# Osso fork: feature index

This document is the **index** of every behavioral change the Osso fork carries
on top of `origin/main` (upstream `openai/codex`). Each feature now has its own
spec under `docs/specs/`; this page maps features to their specs and holds the
cross-cutting rebase checklist.

Each spec follows a hybrid format: a `What it must do` contract (bullets marked
`[x]` only when a named test asserts them, `[ ]` otherwise), an implementation
inventory, the tests asserting the spec, and a per-feature **Rebase risk**
section. Use the spec's rebase-risk section as the conformance checklist when
re-porting that feature onto a new upstream.

Baseline: diff range `origin/main..HEAD`. Regenerate the commit list with
`git log --oneline origin/main..HEAD` when refreshing the specs. Every commit in
that range should map to exactly one feature spec below; a new commit needs a
new spec.

---

## Feature → spec map

| # | Feature | Spec | Rebase risk |
|---|---------|------|-------------|
| 1 | Claude-compatible prompt context hooks | [`prompt-context-hooks.md`](specs/prompt-context-hooks.md) | Medium |
| 2 | `apply_patch` → Claude `Write` payload translation | [`apply-patch-claude-translation.md`](specs/apply-patch-claude-translation.md) | Low–Medium |
| 3 | Session-end transcript parser | [`session-end-transcript-parser.md`](specs/session-end-transcript-parser.md) | Low |
| 4 | Stop / session_start hook plumbing | [`stop-session-start-hook-plumbing.md`](specs/stop-session-start-hook-plumbing.md) | Medium |
| 5 | User rules loader (`$CODEX_HOME/rules/*.md`) | [`user-rules-loader.md`](specs/user-rules-loader.md) | Low |
| 6 | `/run-plan` slash command | [`run-plan-command.md`](specs/run-plan-command.md) | Medium |
| 7 | Multi-agent v2 only — v1 removed | [`multi-agent-v2.md`](specs/multi-agent-v2.md) | HIGH |
| 8 | Unified exec is the only public shell tool | [`unified-exec-shell-tool.md`](specs/unified-exec-shell-tool.md) | HIGH |
| 9 | Subagent / inter-agent communication polish | [`subagent-communication.md`](specs/subagent-communication.md) | Medium |
| 10 | TUI customization | [`tui-customization.md`](specs/tui-customization.md) | Medium |
| 11 | App-server & MCP robustness fixes | [`app-server-robustness.md`](specs/app-server-robustness.md) | Medium |
| 12 | Skip `PWD` in exported shell env | [`skip-pwd-shell-env.md`](specs/skip-pwd-shell-env.md) | Low |
| 13 | Worktree startup option (`-w/--worktree`) | [`worktree-startup-option.md`](specs/worktree-startup-option.md) | Medium |
| 14 | Local deploy script + Osso branding | [`deploy-and-branding.md`](specs/deploy-and-branding.md) | Medium |
| 15 | Model prompt + generated-file hygiene | [`model-prompt-hygiene.md`](specs/model-prompt-hygiene.md) | Low–Medium |
| 16 | Permission prompt approval tool | [`permission-prompt-tool.md`](specs/permission-prompt-tool.md) | HIGH |
| 17 | Hostrun adapter and JS host automation tool | [`hostrun.md`](specs/hostrun.md) | HIGH |
| 18 | PreToolUse command rewrites | [`pre-tool-use-rewrites.md`](specs/pre-tool-use-rewrites.md) | Medium |
| 19 | Resume picker SQLite-first listing | [`resume-picker-sqlite.md`](specs/resume-picker-sqlite.md) | Medium |
| 20 | Aggressive upstream-feature removals | [`upstream-removals.md`](specs/upstream-removals.md) | HIGHEST |

Related contract that is not a fork-specific feature but interacts with the
above: [`approval-system.md`](specs/approval-system.md) (approval policy,
reviewers, and presets — see specs §16 and §18).

---

## Rebase checklist

Before declaring a rebase clean, walk this list:

1. `git log --oneline origin/main..HEAD` — every commit here should map to a
   spec above. New commits need new specs (add them under `docs/specs/` and a
   row in the table above).
2. Resolve all files currently flagged `UU`; use
   `git status --short --untracked-files=all` as the source of truth.
3. Accept or reject any pending `*.snap.new` files after reading the rendered
   diff directly.
4. Walk each feature spec's **Rebase risk** section and re-run the per-spec
   test suites listed under **Tests asserting this spec**, plus:
   - `cargo fmt --check`
   - `cargo clippy --workspace --all-targets`
   - `cargo test --workspace`
5. Regenerate protocol schemas if anything under
   `codex-rs/app-server-protocol/schema/` diverged.
6. Smoke-test the fork's own features end-to-end: run a session with a
   `SessionStart` hook, trigger `/run-plan`, spawn a subagent, run one
   stateful `hostrun_eval` call followed by a second call that reads `ctx`,
   exercise the permission prompt approval tool, open `codex resume` on a
   large local session store to confirm picker rows render from SQLite, and
   confirm `deploy.sh` still installs cleanly.
