# System context (C4 level 1): Codex CLI support

```mermaid
flowchart TB
    dev(["Developer"])
    colctl["<b>colctl</b>"]
    src[("~/.colctl<br/>AGENTS.md, skills/, agents/*.md, config.toml")]
    claudeh[("Claude profiles<br/>(cycle 1, unchanged)")]
    codexh[("Codex profiles = CODEX_HOME<br/>~/.codex, kilo profiles/codex<br/>AGENTS.md link, agents/*.toml, config.toml")]
    shared[("~/.agents<br/>skills/ shared by every Codex profile<br/>(+ skills other tools manage)")]
    codexcli["Codex CLI<br/>codex plugin …"]
    claudecli["Claude Code CLI"]
    kilo["kilo-proxy-headless<br/>merges its keys into profiles/codex/config.toml"]
    other["Other skill tools<br/>(e.g. the twg-* installer, .skill-lock.json)"]

    dev -->|runs| colctl
    dev -->|edits| src
    colctl -->|reads, commits| src
    colctl --> claudeh
    colctl -->|"AGENTS.md link, generated agents/*.toml,<br/>reads config.toml"| codexh
    colctl -->|"links skills/&lt;name&gt;"| shared
    colctl -->|"marketplace add/remove, plugin add/remove<br/>(CODEX_HOME per profile)"| codexcli
    colctl --> claudecli
    codexcli -->|"writes [marketplaces], [plugins], plugins/cache"| codexh
    kilo -->|owns model/provider keys| codexh
    other -->|own entries| shared
```

**Boundaries**
- In a codex harness colctl writes only `AGENTS.md` (a symlink) and the `agents/*.toml` files it generates. Its
  `config.toml` is changed only by the Codex CLI and kilo.
- In `~/.agents` colctl writes only symlinks under `skills/`. Everything else there belongs to other tools.
- colctl never creates a harness folder.
