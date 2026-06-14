# Aggressive upstream-feature removals

The fork has subtracted ~170K lines of upstream code that the OSS Linux-only fork doesn't ship. Unlike conventional feature flags, these deletions are not expressed as configuration — the code simply does not exist in the fork tree. Each rebase reintroduces all of it because upstream continues to develop these subsystems. Every rebase therefore requires mechanical re-deletion of every item below. See `docs/osso_fork.md` for the full fork-divergence index.

## What it must do

### Build infrastructure removed

- [ ] Bazel build system stays removed: `MODULE.bazel`, `MODULE.bazel.lock`, `.bazelrc`, `.bazelversion`, root `BUILD.bazel`, `defs.bzl`, `rbe.bzl`, `workspace_root_test_launcher.{bat,sh}.tpl`, all 99 nested `BUILD.bazel` files, all 27 patches in `patches/`, `bazel.yml` / `rusty-v8-release.yml` / `v8-canary.yml` / `Dockerfile.bazel` workflows, `setup-bazel-ci` / `setup-rusty-v8-musl` / `prepare-bazel-ci` actions, `third_party/v8/`. (commit `16386babdc`)
- [ ] SDK packages stay removed: `sdk/python`, `sdk/python-runtime`, `sdk/typescript`, `sdk.yml` workflow, codex-sdk package path in `codex-cli/scripts/build_npm_package.py`. (commit `16386babdc`)
- [ ] `argument-comment-lint` Dylint tool and its 4 workflows stay removed. (commit `c89a4c527b`)
- [ ] `shell-tool-mcp/` stub stays removed. (commit `16386babdc`)

### Workspace crates removed

- [ ] `responses-api-proxy` crate stays removed. (commit `1972aa32dc`)
- [ ] `cloud-tasks`, `cloud-tasks-client`, `cloud-tasks-mock-client` crates stay removed. (commit `3ea96713f2`)
- [ ] `realtime-webrtc` crate and voice/audio TUI stay removed. (commit `314426231e`)
- [ ] `windows-sandbox-rs` crate and `core/src/windows_sandbox*.rs` stay removed. (commits `fb67fde605`, `b6883dda50`)
- [ ] `lmstudio` crate stays removed. (commit `46627d46a0`)
- [ ] `chatgpt` crate stays removed. (commit `b0959759a0`)
- [ ] TUI ChatGPT connector listing UI stays removed. (commit `3f2215deff`)
- [ ] `mcp-server` (Codex-as-MCP-server, inverse of codex-mcp) crate stays removed. (commit `c17284cbdc`)
- [ ] Feedback upload integration, TUI feedback view, and feedback/upload v2 RPC stay removed. A no-op `codex-feedback` compatibility crate remains in the workspace. (commits `4d5cce1b35`, `59084856f0`)
- [ ] `otel` crate stays removed; stub types remain in `core/src/telemetry.rs`. (commit `5d0c63fd45`)
- [ ] `cloud-requirements` crate stays removed. (commit `f17766b20e`)
- [ ] `aws-auth` crate and Amazon Bedrock provider stay removed. (commit `e866b86470`)
- [ ] `analytics` crate stays gutted in-place to no-ops. (commit `f2e7ab2a46`)
- [ ] `rollout-trace` stays inlined into `codex-core`; standalone crate removed. (commits `af03000a8a`, `95e180c2ac`)
- [ ] `exec-server` remote/WebSocket backend stays removed. (commit `e212445f28`)

### Feature flags removed

- [ ] `RemoteModels` feature flag stays removed (Stage::Removed). (commit `cf84f8b07a`)
- [ ] `WebSearchRequest`, `WebSearchCached`, `SearchTool` feature flags stay removed (Stage::Removed). (commit `f5249a4570`)
- [ ] `UseLegacyLandlock` feature flag and the legacy Landlock code path stay removed. (commit `6a713150b7`)
- [ ] `LegacyMultiAgentV1` feature flag stays removed. (commit `583ed3144a`)
- [ ] `LegacyShellCompat` feature flag stays removed. (commit `b1c1c6e350`)
- [ ] `Sqlite`, `WindowsSandbox`, and `WindowsSandboxElevated` feature flags stay removed, along with their corresponding legacy config/schema keys. (commit `251ea266ff`)

### Workspace plumbing changes

- [ ] `default-members = ["cli"]` remains set in workspace `Cargo.toml` so `cargo build` only touches the shipped binary.
- [ ] `mold` linker, `target-cpu=native`, and `-Z share-generics=y` remain configured in `codex-rs/.cargo/config.toml`. `RUSTC_BOOTSTRAP=1` must remain set in the build environment.
- [ ] Dev profile keeps `debug = "line-tables-only"` and `split-debuginfo = "unpacked"`.
- [ ] `docs/gpt-5.4-underperformance-fix.md` remains present; it records the local model-instruction benchmark note added with the feature-flag cleanup commit.

(These are removal invariants; mark `[x]` only if a real test asserts the removal stays removed. Most stay `[ ]`.)

## How it works

- `docs/wiki/systems/upstream-removals.md` — stub, not yet written.
- `docs/osso_fork.md` — fork-divergence index entry.

## Re-deletion procedure (rebase risk: HIGHEST)

1. After the rebase merges, run `git log --oneline origin/main..HEAD` and look for the deletion commits listed above.
2. If a deletion commit was lost in the merge, cherry-pick it back.
3. If upstream introduced a NEW peripheral feature on top of a removed one (e.g. cloud-tasks v2), assess whether it falls under the same "fork doesn't ship" criterion and remove it too — then add a new deletion commit and update this section.
4. For deps: re-run `cargo machete` after every rebase to catch newly introduced unused crate-level deps.

## Out of scope

- None noted.
