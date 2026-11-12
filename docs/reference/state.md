# The .state folder

`~/.colctl/.state/` is machine-specific bookkeeping. It's git-ignored.

## state.json

```json
{
  "version": 1,
  "harnesses": {
    "kilo-claude": {
      "generated": { "CLAUDE.md": "sha256:…" },
      "marketplaces": ["sadr"],
      "plugins": ["sadr@sadr"]
    }
  }
}
```

- `generated`: files colctl wrote in that profile, with the hash of what it wrote. A `CLAUDE.md` with a different
  hash is reported as `edited` and left alone.
- `marketplaces`, `plugins`: what colctl itself added or installed there. Only these are ever removed by colctl.

For codex harnesses, `generated` also holds `agents/<name>.toml` with the hash of what colctl wrote.

Links aren't recorded: a link in a profile that points into the source belongs to colctl by definition.

## origins

```json
"origins": {
  "skills/twg-jira": { "path": "/Users/me/.config/twg/skills/twg-jira", "hash": "sha256:…" }
}
```

Where each [taken-over](adopt.md#skills-a-tool-linked-in-from-its-own-folder) skill or agent came from, and its tree
hash when adopted or last refreshed. `status` compares the origin and the source copy against it; `refresh` updates
it. The key is absent in state files written by colctl 1.0.

If the file doesn't parse, every command stops with exit 2 and says so. Moving it aside is safe: generated
`CLAUDE.md` files with the expected content are recognised again, edited ones show up as foreign, and plugins colctl
installed are no longer uninstalled automatically.

## lock

Holds the pid of the colctl run that changes things. A lock whose pid isn't running is reclaimed with a notice.
