# colctl sync

```
colctl sync [--harness <name>] [--dry-run]
```

1. Removes leftovers of an interrupted run (`.colctl-tmp-*`) and commits pending edits in the source
   (`colctl: sync`).
2. For each harness (or only `--harness`), in this order:
   - removes leftover temporary entries in the profile;
   - writes, rewrites or removes the generated `CLAUDE.md`;
   - creates, repairs or removes links in `skills/` and `agents/` (creating those folders if needed);
   - `claude plugin marketplace add` for missing marketplaces, then `claude plugin install` for missing plugins;
   - `claude plugin uninstall` / `marketplace remove` for plugins and marketplaces colctl installed that are no
     longer in `config.toml`.

   For a **codex** harness: the `AGENTS.md` link, generated `agents/<name>.toml` files (written, rewritten when the
   source agent changes, removed when it goes), then `codex plugin marketplace add` / `plugin add` / `plugin remove`
   with `CODEX_HOME` set to that profile. For the **agents** harness: links in `skills/` only.
3. Saves the state, re-checks every harness and lists what still needs attention.

It never touches foreign or edited items, never creates a missing profile folder, and keeps going when one action
fails. A failed `claude` call is reported with the full command, its exit code and its stderr. If `claude` isn't on
`PATH`, file changes still happen and plugin actions are reported as `skipped: claude not found`.

Exit code: **0** when every action succeeded and everything is in sync afterwards, **1** otherwise, **2** on errors,
**3** if locked.

`--dry-run` prints `would …` for each planned action, or `nothing to do`.
