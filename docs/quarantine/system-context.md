# System context (C4 level 1): Quarantine

```mermaid
flowchart TB
    dev(["Developer"])
    colctl["<b>colctl</b>"]
    src[("~/.colctl<br/>config.toml: pins (moved only by approve)<br/>skills/, agents/: source copies<br/>plugins/&lt;name&gt;/: clones at the pin<br/>quarantine: held candidates (skill snapshots, plugin commits)<br/>.state: candidates, rejections, risk flags")]
    upstream[("Plugin source repos")]
    origins[("Origin folders of taken-over skills<br/>e.g. ~/.config/twg/skills")]
    marketplaces[("Marketplaces of list plugins")]
    claudeh[("Claude profiles")]
    codexh[("Codex profiles")]
    git["git CLI"]
    clis["claude / codex CLIs"]

    dev -->|"update, refresh: fetch and hold"| colctl
    dev -->|"review: read diff and risk flags"| colctl
    dev -->|"approve / reject"| colctl
    colctl -->|"fetch (no checkout) on update;<br/>checkout the pin only after approve"| git
    git --> upstream
    colctl -->|"snapshot on refresh"| origins
    colctl -->|"holds candidates, commits approved pins"| src
    colctl -->|"after approve only: install / update"| clis
    clis --> claudeh
    clis --> codexh
    marketplaces -.->|"versions colctl can't hold: flagged unreviewed"| claudeh
```

**Boundaries**
- `update` and `refresh` only write to `~/.colctl`'s quarantine area, its state and the clones' git objects.
  They never check out a new commit, change a source copy, commit a pin or call `claude`/`codex`.
- `approve` is the only path from a candidate into a harness, and it installs the held candidate, not whatever
  upstream or the origin holds by then.
- colctl doesn't judge code. Risk flags point the owner at files; the decision is the owner's.
- List plugins' versions stay under their marketplace and the harness's own auto-update. colctl only reports them.
