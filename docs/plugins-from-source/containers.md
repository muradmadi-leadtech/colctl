# Containers (C4 level 2): Plugins from source

```mermaid
flowchart LR
    subgraph machine["Developer's machine"]
        bin["<b>colctl</b> binary"]
        src[("Source store ~/.colctl<br/>config.toml with [[source]] pins")]
        clones[("Plugin clones ~/.colctl/plugins/&lt;name&gt;<br/>+ plugins/.manifests (git-ignored)")]
        state[("State store .state<br/>source plugins installed per harness, at which commit")]
        claudeh[("Claude profiles")]
        codexh[("Codex profiles")]
        claude["claude CLI"]
        codex["codex CLI"]
        git["git CLI"]
    end
    remote[("Plugin source repos")]

    bin --> src
    bin --> state
    bin -->|"clone / fetch / checkout pinned commit"| git
    git --> clones
    git -.->|"only in update and adopt"| remote
    bin -->|"reads origins of installed plugins"| claudeh & codexh
    bin -->|"install from local marketplace, update, uninstall replaced"| claude & codex
    claude -->|reads| clones
    codex -->|reads| clones
```

| Container | Change in cycle 3 |
|---|---|
| Source store | `config.toml` gains `[[source]]` entries (repo, pinned `rev`, optional `replaces`). `.gitignore` gains `plugins/` |
| Plugin clones | New. One git clone per source plugin, checked out at its pin. Colctl-written manifests for repos that lack them live in `plugins/.manifests/<name>/` |
| State store | `harnesses.<h>.sources`: source plugin → commit colctl installed there |
| git CLI | Now also talks to the network, but only from `adopt`, `update` and the first clone of a newly declared source |
