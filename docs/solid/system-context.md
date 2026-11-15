# System context: solid

_Cycle 9 · Focus · C4 level 1_

```mermaid
flowchart LR
  owner([Owner]) -->|colctl, colctl status, review…| colctl[colctl CLI + TUI]
  scripts([Scripts / CI]) -->|--json| colctl
  colctl -->|reads, writes state + review cache| home[(~/.colctl)]
  colctl -->|git log, diff| vault[(plugin clones in ~/.colctl)]
  colctl -->|claude plugin …, codex …| tools[Claude Code, Codex CLIs]
  tools --> profiles[(5 profiles: ~/.claude, ~/.codex, ~/.agents, kilo ×2)]
```

What changes this cycle:

- **New inside `~/.colctl`:** a review cache, one entry per held version, keyed by what was held (commit or content
  hash). Deleting it costs one slow run, nothing else.
- **No new external systems.** The commands now on TUI screens (adopt, convert, rm, refresh, marketplace, connectors,
  developers) call the same tools they already call from the CLI.
