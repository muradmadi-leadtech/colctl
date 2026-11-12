# colctl refresh

```
colctl refresh [skills/<name> | agents/<name>.md …] [--dry-run]
```

Looks at the folders [taken-over](adopt.md#skills-a-tool-linked-in-from-its-own-folder) skills and agents came from,
and **holds** what changed there for review. With no items, it covers every item with a recorded origin. Nothing it
holds reaches the source copy until you [`approve`](approve.md) it.

For each item it compares three things: the origin now, the source copy now, and the hash recorded when the item was
adopted or last approved.

| Origin | Source copy | What refresh does |
|---|---|---|
| unchanged | anything | Nothing ("up to date") |
| changed | unchanged | Copies the origin into `.state/quarantine/<key>` (staged, checked, renamed into place) and records it as held: "held <key> for review; colctl review <key>" |
| changed | changed | Refuses: "changed in both places". Compare the two, keep one, then adopt again |
| gone | — | Refuses: "origin is gone". The source copy stays as it is |

- The source copy, its recorded hash and the source's history don't change, and sync doesn't run.
- An origin that changes again replaces the held copy ("replaces the copy held before"); unchanged since, it prints
  "<key> already held for review".
- Content you [rejected](reject.md) isn't held again: "<key>: this version was rejected; nothing held".
- The held copy keeps symlinks as symlinks; it never follows them.

Exit code: **0**, or **1** if anything was refused, or **2** if an item has no recorded origin, or **3** if another
colctl holds the lock. `--dry-run` lists what would be held.

Before 2.0, `refresh` copied the origin over the source copy, committed and synced in one step.
