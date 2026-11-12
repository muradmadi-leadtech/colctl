# Containers (C4 level 2): Quarantine

```mermaid
flowchart LR
    subgraph machine["Developer's machine"]
        bin["<b>colctl</b> binary"]
        src[("Source store ~/.colctl<br/>config.toml pins, skills/, agents/")]
        clones[("Plugin clones ~/.colctl/plugins/&lt;name&gt;<br/>+ ref refs/colctl/candidate")]
        state[("State store .state<br/>state.json: candidates, rejections<br/>quarantine/: skill and agent snapshots")]
        origins[("Origin folders<br/>e.g. ~/.config/twg/skills")]
        harnesses[("Claude and Codex profiles")]
        clis["claude / codex CLIs"]
        git["git CLI"]
    end
    remote[("Plugin source repos")]

    bin -->|"update: fetch, mark candidate ref"| git
    git --> clones
    git -.->|"update only"| remote
    bin -->|"refresh: snapshot"| origins
    bin -->|"hold, reject: record"| state
    bin -->|"review: git log/diff, diff --no-index"| git
    bin -->|"approve: move pin, replace source copy, commit"| src
    bin -->|"approve → sync"| clis --> harnesses
```

| Container | Change in cycle 4 |
|---|---|
| Source store | Unchanged in shape. Pins and source copies now move only on `approve` |
| Plugin clones | Each clone with a held update keeps a ref `refs/colctl/candidate`, so the commit survives `git gc` and upstream force-pushes until it's approved or rejected |
| State store | `state.json` gains `quarantine` (held candidates) and `rejected` (rejected versions). `.state/quarantine/<kind>/<name>` holds exact snapshots of held skills and agents. `.state/` is already git-ignored, so nothing held is ever committed |
| git CLI | Also used for `log` and `diff` between the pin and the candidate, and `diff --no-index` between a source copy and its snapshot. No new dependency |
| claude / codex CLIs | Called only by `approve` (through sync), never by `update` or `refresh` |
