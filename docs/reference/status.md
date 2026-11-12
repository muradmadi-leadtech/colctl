# colctl status

```
colctl status [--harness <name>] [--json]
```

Reports every item in every registered harness, the source's uncommitted changes and invalid source items. It never
writes anything and doesn't take the lock; `git` is optional (without it, uncommitted changes aren't listed).

Exit code: **0** when everything is in sync and the source is committed, **1** otherwise, **2** on errors (no source,
invalid config, corrupt state, unknown `--harness`).

## Item states

| State | Meaning | Fix |
|---|---|---|
| `ok` | As it should be | — |
| `missing` | colctl should have put it there and hasn't | `colctl sync` |
| `broken` | colctl's link or `CLAUDE.md` is wrong (points elsewhere, stale content) | `colctl sync` |
| `orphaned` | colctl's item whose source is gone | `colctl sync` removes it |
| `foreign` | A real item colctl doesn't own, or a link to somewhere outside the source | `colctl adopt …`, or remove the link yourself |
| `edited` | A `CLAUDE.md` colctl generated that someone changed | Move the text into `AGENTS.md`, delete `CLAUDE.md`, sync |
| `invalid` | Can't be read or used (e.g. a skill without `SKILL.md`, unparsable plugin files) | Shown in the hint |
| `plugin-missing` | A plugin in `config.toml` that isn't installed there | `colctl sync` |
| `marketplace-missing` | A marketplace in `config.toml` the profile doesn't know | `colctl sync` |

Harness states: `ok`, `harness-missing` (start that profile once so it creates its folder), `invalid` (the path is
a file or a symlink).

## `--json`

```json
{
  "version": 1,
  "source": {
    "path": "/Users/me/.colctl",
    "uncommitted": ["skills/foo/SKILL.md"],
    "invalid": [{ "kind": "skill", "name": "bare", "reason": "no SKILL.md" }]
  },
  "harnesses": [
    {
      "name": "kilo-claude",
      "kind": "claude",
      "path": "/Users/me/Library/Application Support/kilo-proxy-headless/profiles/claude",
      "state": "ok",
      "items": [
        { "kind": "skill", "name": "foo", "state": "ok", "hint": null },
        { "kind": "plugin", "name": "sadr@sadr", "state": "plugin-missing", "hint": "colctl sync" }
      ]
    }
  ],
  "in_sync": false
}
```

Item `kind`: `instructions`, `skill`, `agent`, `marketplace`, `plugin`. When a profile's plugin files can't be read,
there is one item `{ "kind": "plugin", "name": "*", "state": "invalid" }` with the error as its hint.
