# System context (C4 level 1): Plugins from source

```mermaid
flowchart TB
    dev(["Developer"])
    colctl["<b>colctl</b>"]
    src[("~/.colctl<br/>config.toml: source plugins with pinned commits<br/>plugins/&lt;name&gt;/ clones (git-ignored)<br/>plugins/.manifests/ (colctl-written manifests)")]
    upstream[("Plugin source repos<br/>e.g. github.com/obra/superpowers")]
    aggregator[("Aggregators<br/>Anthropic plugin directory, other marketplaces")]
    claudeh[("Claude profiles")]
    codexh[("Codex profiles")]
    claudecli["claude CLI"]
    codexcli["codex CLI"]
    git["git CLI"]

    dev -->|"adopt, update, sync"| colctl
    colctl -->|"clone, fetch, checkout (pinned)"| git
    git --> upstream
    colctl -->|reads, commits pins| src
    colctl -->|"reads where a plugin came from"| claudeh
    colctl -->|"reads where a plugin came from"| codexh
    colctl -->|"marketplace add &lt;clone&gt;, install, update, uninstall aggregator copy"| claudecli
    colctl -->|"marketplace add &lt;clone&gt;, plugin add, remove aggregator copy"| codexcli
    claudecli -->|installs from| src
    codexcli -->|installs from| src
    aggregator -.->|"no longer used for adopted plugins"| claudeh
```

**Boundaries**
- colctl writes plugin code only into `~/.colctl/plugins`, and only by `git clone`/`fetch`/`checkout` of the
  declared repo at the declared commit. It never edits a clone.
- Each harness's own plugin cache is still written by its CLI.
- Aggregator marketplaces stay registered. Only the plugins colctl replaces are uninstalled.
