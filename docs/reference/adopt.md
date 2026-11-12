# colctl adopt

```
colctl adopt [--dry-run]
colctl adopt <harness> [--dry-run]
colctl adopt <harness> skills/<name> | agents/<name>.md | CLAUDE.md | plugin <name@marketplace> [--dry-run]
```

Moves foreign items into the source, one at a time, then runs [sync](sync.md). With no arguments it takes every
foreign item in every harness; with a harness, every foreign item there.

| Item | What happens |
|---|---|
| skill / agent | Copied into the source; the copy's hash is checked against the original; only then is the original replaced by a link. If the source already has an identical item it is just linked; if it differs, it is refused. |
| `CLAUDE.md` | Its text becomes `~/.colctl/AGENTS.md` (refused if a different `AGENTS.md` exists) and it is replaced by the generated import. |
| plugin | Added to `[plugins].enabled`; its marketplace's origin is read from the profile's `known_marketplaces.json` and added to `[marketplaces]` if missing. It is not recorded as installed by colctl, so colctl won't uninstall that copy later. |

Each adopted item is committed as `colctl: adopt <kind> <name>` (`colctl: adopt instructions <harness>` for
`CLAUDE.md`). Symlinks pointing outside the source are refused.

Output: one line per item, `adopted …`, `linked (already identical) …` or `refused …: <reason>`.
Exit code: **1** if anything was refused or the sync afterwards isn't clean, otherwise **0**; **2** for an unknown
harness or item.
