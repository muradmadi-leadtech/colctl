# colctl update

```
colctl update [<name>…] [--check] [--rev <commit-ish>] [--dry-run]
```

Looks for newer upstream code of [source plugins](config.md#source-plugins) and **holds** it for review. It's the only
command that looks upstream for new versions. Nothing it finds is installed until you [`approve`](approve.md) it.

- **Without `--check`**, each named source (all by default) is fetched. If upstream's default branch has a commit
  other than the pin, that commit is held: kept in the clone under `refs/colctl/candidate` and recorded in
  [`state.json`](state.md#quarantine). It prints
  `<name>: <old> → <new> (<n> new commits), held for review; colctl review <name>`.
  - The pin, `config.toml`, the source's history, the clone's checkout and every profile stay as they are.
    `update` never calls `claude` or `codex`.
  - A newer commit replaces one held before (`(replaces <short>)`); the same one prints `already held for review`.
  - A commit you [rejected](reject.md) isn't held again: `<name>: <short> was rejected; nothing held`.
- **`--check`** fetches and prints `<name>: <old> → <new> (<n> new commits)`, with `(held for review)` or
  `(rejected)` when that applies, or `<name>: up to date`. It holds nothing. Exit code 1 when any source has a newer
  commit that isn't rejected, 0 when not.
- **`--rev <commit-ish>`** holds a tag, branch or commit, resolved after fetching, newer or older than the pin
  (`goes back n commits`). It needs exactly one source name.

Then: [`colctl review`](review.md), and [`approve`](approve.md) or [`reject`](reject.md).

A source whose upstream can't be reached reports the error and holds nothing (exit 1); the others continue. `--rev`
without exactly one name, an unknown source, or a rev that doesn't resolve is exit 2. Without `--check` and
`--dry-run`, `update` takes the lock (exit 3 if another colctl holds it). With no sources declared it prints
`no sources declared`.

Before 2.0, `update` moved the pin and updated every profile in one step.
