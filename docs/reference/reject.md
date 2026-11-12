# colctl reject

```
colctl reject (<name>… | --all) [--dry-run]
```

Drops held new versions without installing them, and remembers them so they aren't held again.

- The held version is deleted: the clone's `refs/colctl/candidate` ref, or the held copy in `.state/quarantine/`.
- Its version (the commit, or the copy's content hash) is added to `rejected` in [`state.json`](state.md).
- Later, [`update`](update.md) prints `<name>: <short> was rejected; nothing held` when upstream is still at that
  commit, and [`refresh`](refresh.md) prints `<key>: this version was rejected; nothing held` when the origin still
  has that content. A newer version is held as usual.
- `status` stops listing the item's origin as `origin-changed` while the origin holds the rejected content.

Removing the item (`colctl rm source|skill|agent`) also forgets its held version and its rejections.

`--dry-run` prints `would reject <key>: …` and changes nothing.

Exit codes: **0**; **2** no names and no `--all`, both, an unknown name, or nothing held for a name; **3** another
colctl holds the lock.
