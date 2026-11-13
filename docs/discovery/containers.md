# Containers (C4 level 2): Discovery

```mermaid
flowchart LR
    subgraph machine["Developer's machine"]
        bin["<b>colctl</b> binary"]
        claudeh[("Claude profiles<br/>known_marketplaces.json · marketplaces/&lt;m&gt;/.claude-plugin/marketplace.json<br/>plugin-directory-cache-v2.json · installed_plugins.json")]
        codexh[("Codex profiles<br/>config.toml [marketplaces] · local marketplace manifests")]
        src[("Source store ~/.colctl<br/>config.toml, clones")]
        codex["codex CLI"]
        claude["claude CLI"]
    end

    bin -->|"search, show, developers, marketplace list: read files"| claudeh & codexh & src
    bin -->|"connectors only: plugin list --available --json (per profile, in parallel)"| codex
    bin -.->|"marketplace refresh: marketplace update"| claude
    bin -.->|"marketplace refresh: marketplace upgrade"| codex
```

| Container | Change in cycle 5 |
|---|---|
| Claude profiles | Read only: their marketplace list, every marketplace manifest, the plugin-directory cache and installed plugins |
| Codex profiles | Read only: `config.toml` marketplaces and the manifests of local or git marketplaces |
| codex CLI | `plugin list --available --json` for `connectors` (remote entries have no files to read); `plugin marketplace upgrade` for `refresh` |
| claude CLI | `plugin marketplace update` for `refresh` |
| Source store | Read for "already a source"; `add source <id>` writes it as in cycle 3 |

No new files. Nothing is cached by colctl: the tools' own caches are the catalog, so discovery is never staler than
the tools themselves.
