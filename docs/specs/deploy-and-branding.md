# Deploy and Branding

The fork ships a `deploy.sh` at the repo root that builds and installs the `codex` binary locally with an `-osso` version suffix, independent of upstream release machinery. It separates `CODEX_CLI_VERSION` (plain semver, used for update checks) from `CODEX_CLI_DISPLAY_VERSION` (e.g. `0.131.0-alpha.8-osso`, shown in the UI), ensuring update-check logic remains compatible with upstream while local builds are visually distinguishable. Branding constants live in `codex-rs/tui/src/version.rs`; the update-check consumer is `codex-rs/tui/src/update_prompt.rs`; the display point is `codex-rs/cli/src/main.rs`. Release profile tuning in `codex-rs/Cargo.toml` trades smaller binaries for faster local iteration. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/deploy-and-branding.md.

## What it must do

### Build and install
- [ ] `deploy.sh` runs `cargo build -p codex-cli --bin codex --release --locked` from the workspace root.
- [ ] `deploy.sh` installs the resulting binary into `$CODEX_INSTALL_ROOT/bin`.

### Branding split
- [x] `CODEX_CLI_VERSION` is plain semver (no suffix) and is what update checks use.
- [x] `CODEX_CLI_DISPLAY_VERSION` equals `CODEX_CLI_VERSION` with the `-osso` suffix appended.
- [ ] The UI (version display) uses `CODEX_CLI_DISPLAY_VERSION`.
- [ ] Update-check logic uses `CODEX_CLI_VERSION`, not the display version.

### Release profile
- [ ] LTO is disabled in the `release` profile in `codex-rs/Cargo.toml` to speed up local builds.
- [ ] `codegen-units` in the `release` profile is raised to 4.

## How it works

- `docs/wiki/systems/deploy-and-branding.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.

## Implementation inventory

- `deploy.sh` — builds and installs the local fork binary.
- `codex-rs/tui/src/version.rs` — defines `CODEX_CLI_VERSION` and `CODEX_CLI_DISPLAY_VERSION`.
- `codex-rs/cli/src/main.rs` — display point for version string.
- `codex-rs/tui/src/update_prompt.rs` — update-check consumer; must use `CODEX_CLI_VERSION`.
- `codex-rs/Cargo.toml` — release profile LTO and codegen-units tuning.
- `announcement_tip.toml` — may reference version; review on each upstream bump.

## Tests asserting this spec

- `codex-rs/tui/src/version.rs` — `cli_version_is_plain_semver_for_update_checks`: asserts `CODEX_CLI_VERSION` parses as plain semver.
- `codex-rs/tui/src/version.rs` — `display_version_keeps_local_branding`: asserts `CODEX_CLI_DISPLAY_VERSION` equals `{CODEX_CLI_VERSION}-osso`.

## Rebase risk

Medium. Every upstream version bump will conflict with the branding split; walk through `version.rs` manually each time.

## Out of scope

- None noted.
