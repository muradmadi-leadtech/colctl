# colctl refresh

```
colctl refresh [skills/<name> | agents/<name>.md …] [--dry-run]
```

Pulls [taken-over](adopt.md#skills-a-tool-linked-in-from-its-own-folder) skills and agents up to date with the folder
they came from. With no items, it covers every item with a recorded origin.

For each item it compares three things: the origin now, the source copy now, and the hash recorded when the item was
adopted or last refreshed.

| Origin | Source copy | What refresh does |
|---|---|---|
| unchanged | anything | Nothing ("up to date") |
| changed | unchanged | Copies the origin over the source copy (staged, checked, renamed into place), updates the recorded hash, commits `colctl: refresh skill <name>` |
| changed | changed | Refuses: "changed in both places". Compare the two, keep one, then adopt again |
| gone | — | Refuses: "origin is gone". The source copy stays as it is |

Then it runs [sync](sync.md), so generated files such as Codex agent TOML follow the new content.

Exit code: **0**, or **1** if anything was refused, or **2** if an item has no recorded origin.
`--dry-run` lists what would be refreshed.
