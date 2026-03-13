# Config JSON Schema

We generate a JSON Schema for the default user config file (`~/.config/codex/config.toml`, or
`$CODEX_HOME/config.toml` when `CODEX_HOME` is set) from the `ConfigToml` type
and commit it at `codex-rs/core/config.schema.json` for editor integration.

When you change any fields included in `ConfigToml` (or nested config types),
regenerate the schema:

```
just write-config-schema
```
