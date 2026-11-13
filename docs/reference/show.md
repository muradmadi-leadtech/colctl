# colctl show

```
colctl show <id-or-name> [--json]
```

Shows one plugin from the [catalogs](../explanation/catalogs.md) in full. `<id-or-name>` is any of its ids
(`superpowers@anthropic-plugin-directory`), or its name when only one plugin has it.

```
$ colctl show superpowers
Superpowers
  ids          superpowers@claude-plugins-official, superpowers@anthropic-plugin-directory, superpowers@superpowers-dev
  catalogs     claude-plugins-official (claude), anthropic-plugin-directory (claude), colctl (claude)
  developer    obra
  author       Jesse Vincent
  description  Core skills library for Claude Code: TDD, debugging, collaboration patterns…
  repo         https://github.com/obra/superpowers
  version      7.0.2 (commit 22f0b46)
  installed    claude, kilo-claude
  add          already source plugin superpowers
```

Rows without a value are left out. `subpath` appears for plugins in a folder of a larger repo. `add` says what to run:
`colctl add source <id>`, `colctl add plugin <id>` (no repo), or that it's already a source plugin.

`--json` prints `{"version": 1, "result": {…}}` with the fields of a [`search`](search.md#--json) result.

Exit codes: **0**; **2** when no plugin has that id or name ("no plugin <x>"), or several share the name
("<x> is ambiguous: …" with their ids).
