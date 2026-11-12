# System context (C4 level 1): Claude profile sync

```mermaid
flowchart TB
    dev(["Developer<br/>edits skills, AGENTS.md, config.toml;<br/>runs colctl"])

    colctl["<b>colctl</b><br/>CLI that keeps harnesses in step<br/>with ~/.colctl"]

    src[("~/.colctl<br/>source of truth: config.toml, AGENTS.md,<br/>skills/, agents/, .state/ — a local git repo")]
    native[("~/.claude<br/>native Claude Code profile")]
    kilo[("kilo-proxy profiles/claude<br/>(or any CLAUDE_CONFIG_DIR profile)")]

    claudecli["Claude Code CLI<br/>claude plugin …"]
    git["git CLI"]
    kiloproxy["kilo-proxy-headless<br/>creates and merges its profile's settings.json"]
    gh["GitHub Releases<br/>colctl binaries (installed via mise)"]

    dev -->|runs| colctl
    dev -->|edits| src
    colctl -->|reads, writes, commits| src
    colctl -->|"creates links, CLAUDE.md;<br/>reads plugin state"| native
    colctl -->|"creates links, CLAUDE.md;<br/>reads plugin state"| kilo
    colctl -->|"marketplace add, install, uninstall<br/>(per profile, CLAUDE_CONFIG_DIR)"| claudecli
    claudecli -->|writes plugins/, settings.json| native
    claudecli -->|writes plugins/, settings.json| kilo
    colctl -->|init, add, commit| git
    kiloproxy -->|owns env/model keys| kilo
    gh -.->|install| colctl
```

**Boundaries**
- colctl writes **only** inside `~/.colctl` and, inside each harness, symlinks under `skills/` and `agents/` plus a
  generated `CLAUDE.md`.
- Everything under a harness's `plugins/` and its `settings.json` is written by Claude Code's CLI (or kilo), never by
  colctl.
- colctl never creates a harness folder; harnesses are created by their own tools (Claude Code, kilo-proxy).
