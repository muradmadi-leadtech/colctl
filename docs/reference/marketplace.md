# colctl marketplace

```
colctl marketplace list [--json]
colctl marketplace refresh [--dry-run]
```

## list

Every marketplace any Claude or Codex profile knows, offline:

```
anthropic-plugin-directory  claude  built in                                3663 plugins  claude, kilo-claude
claude-plugins-official     claude  github:anthropics/claude-plugins-official  317 plugins  claude, kilo-claude
superpowers-dev             claude  ~/.colctl/plugins/superpowers                 1 plugin   claude, kilo-claude   managed
superpowers-dev             codex   ~/.colctl/plugins/superpowers                 1 plugin   codex, kilo-codex     managed
```

Columns: name, tool, where it comes from, how many plugins it lists, the profiles that know it, and `managed` when
colctl manages it (declared in `config.toml`, or a marketplace colctl added for a [source plugin](config.md#source-plugins)).

`--json`: `{"version": 1, "marketplaces": [{"name", "tool", "source", "plugins", "profiles", "managed"}], "warnings": []}`.

Exit codes: **0**; **1** when no profile knows any marketplace.

## refresh

Asks each tool to update its own catalogs, which is what `search` reads:

- each Claude profile: `claude plugin marketplace update` (every marketplace it knows);
- each Codex profile: `codex plugin marketplace upgrade` (every Git marketplace it knows).

Calls use colctl's usual environment (each profile's own `CLAUDE_CONFIG_DIR` / `CODEX_HOME`, credentials variables
removed) and the `COLCTL_TOOL_TIMEOUT_SECS` timeout. This is the one discovery command that uses the network. The
plugin directory is refreshed by Claude itself.

`--dry-run` prints each call without running it. A tool that isn't on `PATH` is reported as skipped for its profiles.

Exit codes: **0** when every call succeeded; **1** when one failed or a tool was missing.
