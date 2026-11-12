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
| `ok` | As it should be. A list plugin whose source can be found also gets the hint `source available: …` | — (or `colctl adopt …` to move it to its source) |
| `missing` | colctl should have put it there and hasn't | `colctl sync` |
| `broken` | colctl's link or `CLAUDE.md` is wrong (points elsewhere, stale content) | `colctl sync` |
| `orphaned` | colctl's item whose source is gone | `colctl sync` removes it |
| `foreign` | A real item colctl doesn't own | `colctl adopt …` |
| `linked-elsewhere` | A symlink pointing outside the source, e.g. a skill another tool linked in | For skills and Claude agents: `colctl adopt …` takes it over. Otherwise remove the link yourself |
| `edited` | A file colctl generated (`CLAUDE.md`, a Codex agent TOML) that someone changed | Move the change into the source, delete the file, sync |
| `invalid` | Can't be read or used (e.g. a skill without `SKILL.md`, unparsable plugin files) | Shown in the hint |
| `plugin-missing` | A plugin in `config.toml` that isn't installed there | `colctl sync` |
| `marketplace-missing` | A marketplace in `config.toml` the profile doesn't know | `colctl sync` |
| `outdated` | A source plugin installed at another commit than its pin | `colctl sync` |
| `unsupported` | A source whose repo has no packaging for this harness's tool. Informational: doesn't make the exit code 1 | — |
| `from-aggregator` | A plugin installed from a marketplace while its own repo is known (shown in the hint) | `colctl adopt <harness> plugin <id>` |

Harness states: `ok`, `harness-missing` (start that profile once so it creates its folder), `invalid` (the path is
a file or a symlink).

## `--json`

```json
{
  "version": 1,
  "source": {
    "path": "/Users/me/.colctl",
    "uncommitted": ["skills/foo/SKILL.md"],
    "invalid": [{ "kind": "skill", "name": "bare", "reason": "no SKILL.md" }],
    "origin_changed": [{ "item": "skills/twg-jira", "origin": "/Users/me/.config/twg/skills/twg-jira" }]
  },
  "harnesses": [
    {
      "name": "kilo-claude",
      "kind": "claude",
      "path": "/Users/me/Library/Application Support/kilo-proxy-headless/profiles/claude",
      "state": "ok",
      "items": [
        { "kind": "skill", "name": "foo", "state": "ok", "hint": null, "target": null },
        { "kind": "plugin", "name": "sadr@sadr", "state": "plugin-missing", "hint": "colctl sync", "target": null }
      ]
    }
  ],
  "sources": [
    { "name": "superpowers", "repo": "https://github.com/obra/superpowers", "rev": "8ca22db…", "clone": "ok" }
  ],
  "in_sync": false
}
```

`"sources"` lists each `[[source]]` with its clone's state: `ok`, `missing` (sync clones it), `moved` (sync checks out
the pin), `dirty` (local changes; never touched) or `invalid`. Each claude/codex harness also lists each source as an
item of kind `"source"`. `status` never contacts upstream.

Harness `kind`: `claude`, `codex` or `agents`. Item `kind`: `instructions` (named `CLAUDE.md` or `AGENTS.md`),
`skill`, `agent`, `marketplace`, `plugin`. `target` is set for `linked-elsewhere` items.

`source.origin_changed` lists adopted items whose origin has changed since they were adopted or last refreshed, while
the source copy hasn't; `colctl refresh <item>` pulls them in. Any entry there makes the exit code 1. When a profile's plugin files can't be read,
there is one item `{ "kind": "plugin", "name": "*", "state": "invalid" }` with the error as its hint.
