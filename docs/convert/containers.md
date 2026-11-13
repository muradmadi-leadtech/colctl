# Containers: convert (C4 level 2)

colctl stays one binary. Conversion adds one folder to the source and one config field; no new process or service.

```mermaid
flowchart TB
    subgraph colctl[colctl binary]
        cli[CLI: convert, review, approve, reject, update, sync, status]
        repackage[Repackager: clone → Codex package, pure]
        packaging[Packaging: which folder each tool installs from]
        quarantine[Quarantine: held candidates]
    end
    config[(config.toml: [[source]] convert = ["codex"])]
    clone[(plugins/&lt;name&gt;: the pinned clone, read only)]
    converted[(plugins/.converted/&lt;name&gt;: generated package)]
    held[(.state/quarantine/conversions/&lt;name&gt;: a held package)]
    manifests[(plugins/.manifests/&lt;name&gt;: colctl-&lt;name&gt; marketplace)]
    codex[Codex CLI]

    cli --> repackage --> clone
    repackage --> converted & held
    cli --> quarantine --> held
    cli --> config
    packaging --> converted
    packaging --> manifests
    cli -- "plugin marketplace add / enable" --> codex
    codex --> manifests --> converted
```

| Container | What changes |
|---|---|
| `config.toml` | `[[source]]` gains `convert = ["codex"]`, written only when the owner approves a conversion; removed by `convert --undo` or when the repo starts packaging for Codex itself. The pin (`rev`) stays the single record of what was reviewed: a conversion is a pure function of the pinned commit, so approving a commit approves its conversion |
| `plugins/.converted/<name>` | New. The generated package for the pinned commit. Rebuilt by sync when missing or stale; gitignored like the clones |
| `.state/quarantine/conversions/<name>` | New candidate kind: the package generated for review before the first approval |
| `plugins/.manifests/<name>` | Unchanged mechanism (cycle 6, T0): the Codex entry points at the converted package instead of a folder of the clone |
| Codex CLI | Unchanged calls. Codex asks the owner to trust the hooks before they run |
