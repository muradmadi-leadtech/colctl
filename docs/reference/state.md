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

Links aren't recorded: a link in a profile that points into the source belongs to colctl by definition.

If the file doesn't parse, every command stops with exit 2 and says so. Moving it aside is safe: generated
`CLAUDE.md` files with the expected content are recognised again, edited ones show up as foreign, and plugins colctl
installed are no longer uninstalled automatically.

## lock

Holds the pid of the colctl run that changes things. A lock whose pid isn't running is reclaimed with a notice.
