# System context (C4 level 1): Discovery

```mermaid
flowchart TB
    dev(["Developer"])
    tui(["colctl TUI (cycle 6)"])
    colctl["<b>colctl</b>"]
    claudeh[("Claude profiles<br/>known_marketplaces.json, marketplace manifests,<br/>plugin-directory-cache-v2.json, installed_plugins.json")]
    codexh[("Codex profiles<br/>config.toml marketplaces")]
    codexcli["codex CLI<br/>plugin list --available --json"]
    claudecli["claude CLI"]
    src[("~/.colctl<br/>config.toml sources and lists")]
    upstream[("Plugin repos")]

    dev -->|"search, show, developers, marketplace list, connectors"| colctl
    tui -->|"same commands, --json"| colctl
    colctl -->|reads| claudeh
    colctl -->|reads| codexh
    colctl -->|"lists catalogs (no network)"| codexcli
    colctl -->|reads| src
    dev -->|"add source &lt;id&gt;"| colctl
    colctl -.->|"clone, only on add source (cycle 3)"| upstream
    colctl -.->|"marketplace refresh: update catalogs"| claudecli
    colctl -.->|"marketplace refresh: upgrade catalogs"| codexcli
```

**Boundaries**
- Discovery reads catalogs that Claude and Codex already keep. It never writes them, except through the tools' own
  `marketplace update`/`upgrade` in `marketplace refresh`.
- Nothing is installed by discovery itself. `add source <id>` is the existing cycle 3 path with the repo looked up.
- No network outside `marketplace refresh` and `add source`.
