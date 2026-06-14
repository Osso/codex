# Model Prompt Hygiene

This spec covers two small but merge-sensitive fork-local changes that keep model metadata and generated artifacts aligned with Osso workflow expectations. First, the `apply_patch` instruction for GPT-5.4 in `codex-rs/models-manager/models.json` is shortened and relaxed so the model is encouraged to use the patch tool without over-constraining every edit path. Second, `.gitignore` is extended to ignore `*.snap.new` files so transient `insta` snapshot output does not contaminate fork commits. A related benchmark note is preserved in `docs/gpt-5.4-underperformance-fix.md`. See docs/osso_fork.md for the fork-divergence index; how it works belongs in docs/wiki/systems/model-prompt-hygiene.md.

## What it must do

### GPT-5.4 apply_patch wording
- [ ] The GPT-5.4 model entry in `codex-rs/models-manager/models.json` contains a shortened, relaxed `apply_patch` instruction that encourages use of the patch tool.
- [ ] The instruction does not over-constrain edit paths (no exhaustive list of required conditions).

### Snapshot ignore
- [x] `.gitignore` includes a rule ignoring `*.snap.new` files.

## How it works

- `docs/wiki/systems/model-prompt-hygiene.md` (stub — not yet written).
- `docs/osso_fork.md` — fork-divergence index entry.
- `docs/gpt-5.4-underperformance-fix.md` — local model-instruction benchmark note; link here when the wiki stub is written.

## Implementation inventory

- `codex-rs/models-manager/models.json` — contains the GPT-5.4 model entry with the relaxed `apply_patch` instruction.
- `.gitignore` — extended with `*.snap.new` to suppress transient insta output.
- `docs/gpt-5.4-underperformance-fix.md` — records the benchmark rationale for the instruction change.

## Tests asserting this spec

- No automated tests. Re-run tests that consume model metadata when touching `models.json`; review pending snapshots directly before accepting them.

## Rebase risk

Low to medium. Upstream frequently regenerates `codex-rs/models-manager/models.json`, so inspect the GPT-5.4 model entry by hand after each merge instead of assuming a JSON merge preserved the intended prompt text.

## Out of scope

- None noted.
