# Skip PWD from Shell Environment

When exporting the shell environment to child processes, `PWD` is stripped from the exported variable set. A stale `PWD` leaks working-directory state from the parent codex process into spawned shells and confuses path-sensitive tools that trust `PWD` over the kernel's notion of the current directory. Source lives in `codex-rs/protocol/src/shell_environment.rs`, with regression tests in `codex-rs/core/src/exec_env_tests.rs`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/skip-pwd-shell-env.md.

## What it must do

### Environment export filtering
- [x] `PWD` is excluded from the environment variable set exported to child shell processes.
- [x] Default excludes behaviour strips `PWD` even when the caller does not explicitly list it.
- [ ] The exclusion applies regardless of case on case-insensitive platforms.

### Correctness
- [ ] Other path-related variables (`PATH`, `HOME`, etc.) are not inadvertently stripped by the same logic.
- [ ] The child process inherits the correct working directory through the OS-level mechanism rather than through `PWD`.

## How it works

- `docs/wiki/systems/skip-pwd-shell-env.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `codex-rs/protocol/src/shell_environment.rs` — environment construction logic; strips `PWD` before export.
- `codex-rs/core/src/exec_env_tests.rs` — regression tests covering the exclusion behaviour.

## Tests asserting this spec

- `codex-rs/core/src/exec_env_tests.rs` — `test_excludes_pwd_state` (direct regression for PWD exclusion).
- `codex-rs/core/src/exec_env_tests.rs` — `test_core_inherit_with_default_excludes_enabled` (default-excludes path covers PWD).

## Rebase risk

Low — 16-line change plus tests.

## Out of scope

- None noted.
