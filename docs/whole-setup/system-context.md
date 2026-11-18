# System context: whole setup

_Cycle 10 · Focus · C4 level 1_

```mermaid
flowchart LR
  owner([Owner]) -->|add mcp, rm mcp, adopt, status, sync, TUI| colctl[colctl CLI + TUI]
  scripts([Scripts / CI]) -->|--json| colctl
  colctl -->|config: mcp entries, commands/, hooks/| home[(~/.colctl)]
  colctl -->|resolves secret references at sync| secrets[(environment, macOS keychain)]
  colctl -->|claude mcp add/remove, claude plugin install| claude[Claude Code CLI]
  colctl -->|codex mcp add/remove, codex plugin install| codex[Codex CLI]
  colctl -.->|reads, never writes| configs[(~/.claude.json, config.toml)]
  claude --> cprofiles[(~/.claude, kilo claude)]
  codex --> xprofiles[(~/.codex, kilo codex)]
```

What changes this cycle:

- **New in `~/.colctl`:** MCP server entries in `config.toml` (shared values plus per-profile overrides; secrets as
  references only), a `commands/` folder and a hooks file. colctl packages the last two as its own plugin.
- **New external system: secret stores.** colctl reads an environment variable or a keychain item at sync, passes
  the value to the tool's CLI and never stores it.
- **New reads:** each profile's MCP servers, from `.claude.json` and `config.toml`. Read-only: `claude mcp list` would
  start every server.
- **Same writers as before:** everything a profile gets still goes through `claude` and `codex` themselves.
- **Not involved:** `~/.agents` (no MCP servers, commands or hooks).
