# Unified Exec Shell Tool

The fork publicly registers only `exec_command` and `write_stdin`, both backed by `UnifiedExecHandler`. The deprecated `Feature::LegacyShellCompat` flag is gone, and the old aliases (`shell`, `container.exec`, `local_shell`, `shell_command`) are not exposed by the registry. Legacy handler source still exists as internal compatibility plumbing and must not be reintroduced as public tools. Source lives primarily in `codex-rs/core/src/tools/` (spec registration in `spec_plan.rs`, handler in `handlers/shell_spec.rs`, runtime in `runtimes/shell.rs` and its submodules). See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/unified-exec-shell-tool.md.

## What it must do

### Tool registration
- [ ] The public tool registry exposes `exec_command` as the canonical shell tool.
- [ ] The public tool registry exposes `write_stdin` as the canonical stdin-injection tool.
- [ ] The tool registry does NOT expose `shell`, `container.exec`, `local_shell`, or `shell_command` as public tools.
- [ ] `Feature::LegacyShellCompat` does not exist and cannot be re-enabled.
- [ ] `ToolHandlerKind::Shell` and `ToolHandlerKind::ShellCommand` variants do not exist as public-facing kinds.

### Parallel eligibility
- [x] The parallel-eligibility list is narrowed to exactly `unified_exec | exec_command | write_stdin` — no legacy aliases included.

### Internal plumbing preservation
- [ ] Legacy handler source may remain as internal plumbing but must not be reachable through public tool dispatch.

## How it works

- `docs/wiki/systems/unified-exec-shell-tool.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/core/src/tools/spec_plan.rs` — registers only `UnifiedExec`-backed tools publicly; omits legacy aliases.
- `codex-rs/core/src/tools/handlers/shell_spec.rs` — `UnifiedExecHandler` implementation.
- `codex-rs/core/src/tools/runtimes/shell.rs` — shell runtime core.
- `codex-rs/core/src/tools/runtimes/shell/unix_escalation.rs` — Unix privilege escalation support.
- `codex-rs/core/src/tools/runtimes/shell/zsh_fork_backend.rs` — zsh-based shell backend.
- `codex-rs/core/src/tools/spec_plan_tests.rs` — tests asserting correct tool registration and parallel eligibility.
- `codex-rs/core/src/tools/parallel.rs` — parallel-eligibility list narrowed to three names.

## Tests asserting this spec

- `codex-rs/core/src/tools/spec_plan_tests.rs` — `test_parallel_support_flags` (asserts parallel-eligibility list).

## Rebase risk

HIGH. Upstream still ships the legacy shell aliases. After a rebase: keep `Feature::LegacyShellCompat` gone, keep `ToolHandlerKind::{Shell,ShellCommand}` variants gone, keep `spec_plan.rs` from publicly registering the old aliases, and trim the parallel-eligibility list back to the three current names.

## Out of scope

- Re-exposing legacy shell aliases as public tools.
