# colctl connectors

```
colctl connectors [<term>…] [--limit <n>] [--json]
```

Lists Codex's app connectors (Gmail, Slack, Linear…): the entries of Codex's remote catalog that have no repo, so
colctl can't install them from source. They're kept out of [`search`](search.md), which works offline.

It runs `codex plugin list --available --json` once per Codex profile, in parallel, and lists the entries whose source
is `remote`, once each, with the profiles they're installed in. Terms and `--limit` work as in `search`.

```
$ colctl connectors gmail
✓  gmail@openai-curated-remote  openai  openai-curated-remote

1 of 1 connectors · ✓ installed · installed through codex
```

Each result's `add` reads `installed by codex: codex plugin add <id>`: connectors are installed and signed into
through Codex itself. `--json` has the `search` shape with `"kind": "connector"`.

Exit codes: **0** with results; **1** when nothing matches or a profile's call failed (that profile is skipped with a
warning; the rest are listed); **2** when `codex` isn't on `PATH` or no Codex profile is registered.
