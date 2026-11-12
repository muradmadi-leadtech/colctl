# Containers (C4 level 2): Codex CLI support

Still one Rust binary. The new outside pieces are the Codex CLI and two new kinds of folder.

```mermaid
flowchart LR
    subgraph machine["Developer's machine"]
        bin["<b>colctl</b> binary"]
        src[("Source store ~/.colctl")]
        state[("State store ~/.colctl/.state")]
        claudeh[("Claude profile folders")]
        codexh[("Codex profile folders (CODEX_HOME)")]
        agentsdir[("~/.agents (shared skills)")]
        claude["claude CLI (optional)"]
        codex["codex CLI (optional)"]
        git["git CLI (required)"]
    end

    bin --> src
    bin --> state
    bin -->|links, CLAUDE.md, reads plugins/*.json| claudeh
    bin -->|AGENTS.md link, agents/*.toml, reads config.toml| codexh
    bin -->|skill links| agentsdir
    bin -->|spawn, CLAUDE_CONFIG_DIR| claude
    bin -->|spawn, CODEX_HOME| codex
    bin --> git
```

| Container | Change in cycle 2 |
|---|---|
| colctl binary | Two new harness kinds, a Codex plugin list, agent conversion |
| Source store | Unchanged layout. `config.toml` gains an optional `[codex]` table |
| State store | Unchanged shape. `generated` now also holds `agents/<name>.toml` hashes for codex harnesses |
| Codex profile folders | New target. colctl writes `AGENTS.md` (symlink) and `agents/*.toml`; reads `config.toml` |
| `~/.agents` | New target. colctl writes symlinks under `skills/` only |
| codex CLI | New, optional. Like `claude`: if it's missing, file work still happens and plugin actions are skipped |
