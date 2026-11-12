# colctl approve

```
colctl approve (<name>… | --all) [--dry-run]
```

Installs held new versions exactly as they were reviewed, then syncs every harness. `<name>` is a source name,
`skills/<name>` or `agents/<name>.md`; `--all` approves everything held.

- **A source plugin**: the pin in `config.toml` moves to the held commit, committed as
  `colctl: approve source <name> at <short>`. Sync then checks the clone out at it and updates every Claude and Codex
  profile.
- **A skill or agent**: the held copy replaces the source copy, its recorded origin hash becomes the held copy's,
  and it's committed as `colctl: approve skill <name>` (or `agent`). Sync then carries it to every profile,
  including regenerated Codex agent TOML.

Each approval is its own commit; sync runs once at the end. What gets installed is the held version, even if the
upstream repo or the origin folder has moved on since. Run `update` or `refresh` again to hold the newer one.

## Refusals

Nothing changes for a refused item; the others still go ahead. Exit code 1.

| Case | Message |
|---|---|
| The held copy was edited since it was held | "the held copy of <key> changed since it was held; reject it and refresh again" |
| The held commit is gone from the clone | "the held commit for <name> is missing; run colctl update <name>" |
| The pin in `config.toml` was changed by hand after holding | "<name>'s pin changed since it was held; run colctl update <name> again" |
| The skill or agent's source copy was edited since it was held | "changed in both places; …" |
| The clone has local changes or isn't usable | "the clone at … has local changes" or the reason |

If sync fails in a profile afterwards, the approval stands: the pin is committed and the version is no longer held.
`colctl sync` retries.

`--dry-run` prints `would approve <key>: <from> → <to>` for each and changes nothing.

Exit codes: **0** approved and in sync; **1** something refused or sync reported a problem; **2** no names and no
`--all`, both, an unknown name, or nothing held for a name; **3** another colctl holds the lock.
