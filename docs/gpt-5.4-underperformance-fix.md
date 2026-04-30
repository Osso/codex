# Codex CLI — gpt-5.4 Underperformance Fix

## Problem

`gpt-5.4` run via codex-cli consistently underperforms the same model run via
other harnesses on multi-requirement coding tasks:

| Harness | Model | Score |
|---|---|---|
| codex-cli | gpt-5.4 | 2–3/10 (n=3, identical failure mode) |
| codex-cli | gpt-5.3-codex | 7/10 |
| openclaw | gpt-5.4 | 7/10 |
| openrouter raw SDK | gpt-5.4 | 9/10 |

Failure mode: model commits the first patch (main helper class), declares
"done", never re-reads the task to address secondary requirements (e.g.
admin-bypass logic). Same model in raw-SDK harness handles both parts.

## Root Cause

`codex-rs/models-manager/models.json` ships per-model `base_instructions`.
The `gpt-5.4` entry includes a strict directive:

> **"Always use apply_patch for manual code edits. Do not use cat or any
> other commands when creating or editing files."**

`gpt-5.3-codex`'s entry softens this to:

> "Try to use apply_patch for single file edits, but it is fine to explore
> other options to make the edit if it does not work well."

The `Always` + `Do not use cat` framing pushes `gpt-5.4` into a single-pass
"read→patch→commit" loop. It resolves the first visible requirement in the
patch and stops. `gpt-5.3-codex` retains permission to iterate (re-read,
spot-check with `cat`/`rg`, patch again), so it addresses all requirements.

Instruction length also diverges: gpt-5.4 ships ~14,134 chars of base
instructions vs ~12,375 for gpt-5.3-codex — the added content is stricter
tool-use rules, not more task guidance.

## Fix

Relax the `apply_patch` directive in `gpt-5.4`'s `base_instructions` to
match the exploratory tone used for `gpt-5.3-codex`. Keep `apply_patch` as
the preferred edit tool but re-enable free exploration.

### Minimal change

In `codex-rs/models-manager/models.json`, find the entry with
`"slug": "gpt-5.4"` and in its `base_instructions` string replace:

```
Always use apply_patch for manual code edits. Do not use cat or any other
commands when creating or editing files.
```

with:

```
Prefer apply_patch for code edits. You may use cat, rg, or other shell
tools to re-read files and verify your changes between patches.
```

### Verification

1. Re-run the llm-bench task:
   `/syncthing/Sync/Projects/claude/llm-bench` — run-id 38 (`codex-cli/gpt54/full/inline`)
2. Expected: score climbs from 3/10 toward the 7–9/10 range other harnesses
   get on the same model. Failure mode (missing admin-bypass) should
   disappear because the model is no longer forced into single-pass patching.
3. Re-run run-id 19 (`codex-cli/gpt5.3-codex/empty/inline`) as a regression
   guard — should stay ≥7/10.

### Secondary cleanup (optional)

Audit `base_instructions` across all non-codex-family GPT-5.x models
(`gpt-5`, `gpt-5.1`, `gpt-5.2`, `gpt-5.4`) for similar strict directives.
The directive is appropriate for codex-trained variants; general models
need exploratory room.

## Evidence Log

- llm-bench runs in `/syncthing/Sync/Projects/claude/llm-bench/results/`
  - run-38 (codex/gpt5.4/full): 3/10
  - run-21 (codex/gpt5.4/empty): 3/10
  - run-26 (codex/gpt5.4/skills): 2/10
  - run-24 (openclaw/gpt5.4): 7/10
  - run-39 (openrouter/gpt5.4): 9/10
  - run-19 (codex/gpt5.3-codex): 7/10
- Instruction lengths cross-checked with:
  `nu -c 'open models.json | get models | where slug =~ "gpt-5" | each {|m| {slug: $m.slug, len: ($m.base_instructions | str length)}}'`
