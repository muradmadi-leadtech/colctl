# colctl update

```
colctl update [<name>…] [--check] [--rev <commit-ish>] [--dry-run]
```

Moves [source plugins](config.md#source-plugins)' pins. It's the only command that looks for newer upstream code.

- **`--check`** fetches each source and prints `<name>: <old> → <new> (<n> new commits)` or `<name>: up to date`.
  It changes nothing. Exit code 1 when anything is newer, 0 when not.
- **Without `--check`**, each named source (all by default) is pinned to its default branch's head. Each change is
  committed as `colctl: update source <name> to <short>`, then sync updates every harness: Claude runs
  `claude plugin marketplace update` then `claude plugin update`, and Codex runs `codex plugin add`.
- **`--rev <commit-ish>`** pins a tag, branch or commit, resolved after fetching. It needs exactly one source name.

A source whose upstream can't be reached reports the error and keeps its pin (exit 1). `--rev` without exactly one
name, an unknown source, or a rev that doesn't resolve is exit 2.
