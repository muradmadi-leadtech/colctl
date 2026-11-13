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
hash when adopted or last approved. `status` and `refresh` compare the origin and the source copy against it;
`approve` updates it. The key is absent in state files written by colctl 1.0.

If the file doesn't parse, every command stops with exit 2 and says so. Moving it aside is safe: generated
`CLAUDE.md` files with the expected content are recognised again, edited ones show up as foreign, and plugins colctl
installed are no longer uninstalled automatically.

## lock

Holds the pid of the colctl run that changes things. A lock whose pid isn't running is reclaimed with a notice.

## sources (per harness)

```json
"harnesses": { "codex": { "sources": {
  "superpowers": { "id": "superpowers@superpowers-dev", "marketplace": "superpowers-dev", "rev": "8ca22db…" }
} } }
```

What colctl installed for each source plugin in that harness, and at which commit. A failed update leaves the old
commit here, so the next sync retries. When a source is removed, colctl uninstalls exactly what's recorded.

## quarantine

```json
"quarantine": {
  "sources/superpowers": { "kind": "source", "from": "8ca22db…", "to": "22f0b46…", "held_at": 1794500000 },
  "skills/twg-jira": { "kind": "skill", "origin": "/Users/me/.config/twg/skills/twg-jira",
                       "base": "sha256:…", "hash": "sha256:…", "held_at": 1794500000 }
}
```

New versions held for review (2.0). The key is `sources/<name>`, `skills/<name>` or `agents/<name>.md`, or
`conversions/<name>` for a source's Codex package held by [convert](convert.md) (2.3):
`{"kind": "conversion", "rev": "<commit>", "digest": "<sha256>", "held_at": …}`, with the package in
`.state/quarantine/conversions/<name>/`. A converted source installed in a Codex profile is recorded at
`<rev>+<first 12 of digest>`.

- A source plugin: `from` is the pin when it was held, `to` the held commit. The clone keeps that commit under
  `refs/colctl/candidate` until it's approved or rejected, so `git gc` and upstream force-pushes can't lose it.
- A skill (`kind: skill`) or agent (`kind: agent`): `origin` is where it was copied from, `base` the source copy's hash
  when it was held, `hash` the held copy's hash. The copy itself is in `.state/quarantine/<key>`, so it's never
  committed.
- `held_at`: Unix seconds.

## rejected

```json
"rejected": {
  "sources/superpowers": ["22f0b46…"],
  "skills/twg-jira": ["sha256:…"]
}
```

Versions you [rejected](reject.md): commits for source plugins, content hashes for skills and agents. `update` and
`refresh` don't hold them again. Removing the item forgets them.

Both keys are absent until something is held or rejected, so 1.x state files load unchanged. colctl 1.x can't read a
state file that has them; that's expected after a major upgrade.
