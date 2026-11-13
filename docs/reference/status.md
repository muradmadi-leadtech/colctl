# colctl status

```
colctl status [--harness <name>] [--all] [--json]
```

Reports every item in every registered harness, the source's uncommitted changes and invalid source items. It never
writes anything and doesn't take the lock; `git` is optional (without it, uncommitted changes aren't listed).

```
$ colctl status
✓ All 5 profiles in sync · 3 sources · nothing held for review

  ✓  claude       ~/.claude                    14 skills · 3 plugins · CLAUDE.md
  ✓  kilo-claude  ~/Library/…/profiles/claude  14 skills · 3 plugins · CLAUDE.md
  ✓  codex        ~/.codex                     2 plugins · AGENTS.md · skills via
                                               ~/.agents · 1 not packaged
  ✓  kilo-codex   ~/Library/…/profiles/codex   2 plugins · AGENTS.md · skills via
                                               ~/.agents · 1 not packaged
  ✓  agents       ~/.agents                    14 skills

→ colctl status --all lists every item
```

The first line is the verdict; then one line per profile with what it has: skills, agents, plugins (list and source
plugins together) and its instructions file, then any problems (`1 missing`) and what's informational (`1 not packaged`:
a source plugin whose repo has no packaging for that tool; `1 unreviewed`). Codex reads skills from `~/.agents`, not
its own folder, so a Codex profile says `skills via ~/.agents` instead of counting them; that's why Claude and Codex
counts differ. An empty profile says `nothing yet`.

Paths are shown in full when the terminal is wide enough. When they aren't, every path in the column is shortened the
same way (`~/Library/…/profiles/claude` and `~/Library/…/profiles/codex`), and what a profile has wraps under itself
rather than being cut. When something needs you, the verdict
says how many things (`! 3 things need attention`) and a **Needs attention** section lists exactly those, each with
its fix:

```
Needs attention
  !  claude  skill   alpha        missing            → colctl sync
  !  held    source  superpowers  8ca22db → 22f0b46  → colctl review superpowers
```

`--all` adds every item of every profile. `-q` prints only the verdict. See [Output](output.md).

Exit code: **0** when everything is in sync and the source is committed, **1** otherwise, **2** on errors (no source,
invalid config, corrupt state, unknown `--harness`).

## Item states

| State | Meaning | Fix |
|---|---|---|
| `ok` | As it should be | — |
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
| `unreviewed` | A plugin from `[plugins]`/`[codex.plugins]`. Its updates come from its marketplace (and the tool's own auto-update), not through `colctl review`. Informational: doesn't make the exit code 1. When its source repo is known, the hint says so | — (or `colctl adopt <harness> plugin <id>` to make it a source plugin, whose updates are held for review) |

Harness states: `ok`, `harness-missing` (start that profile once so it creates its folder), `invalid` (the path is
a file or a symlink).

## Held for review

Versions held by `update` or `refresh` are listed under **Needs attention** as `held`, with `colctl review <name>` as
the fix. While anything is held the exit code is 1. Held versions don't affect whether the profiles are in sync, so
`sync` isn't held back by them.

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

`"quarantine"` lists held versions, each with `"state": "pending-review"`, the fields of
[`review --json`](review.md#--json) without `commits`, `files` and `diff`, and `"flags"` as a count:

```json
"quarantine": [
  { "key": "sources/superpowers", "kind": "source", "name": "superpowers", "state": "pending-review",
    "from": "8ca22db…", "to": "22f0b46…", "direction": "forward", "repo": "https://github.com/obra/superpowers",
    "held_at": 1794500000, "flags": 3, "error": null, "warning": null }
]
```

`"in_sync"` is about the profiles and the source only; a non-empty `"quarantine"` still makes the exit code 1.

`"sources"` lists each `[[source]]` with its clone's state: `ok`, `missing` (sync clones it), `moved` (sync checks out
the pin), `dirty` (local changes; never touched) or `invalid`. Each claude/codex harness also lists each source as an
item of kind `"source"`. `status` never contacts upstream.

Harness `kind`: `claude`, `codex` or `agents`. Item `kind`: `instructions` (named `CLAUDE.md` or `AGENTS.md`),
`skill`, `agent`, `marketplace`, `plugin`. `target` is set for `linked-elsewhere` items.

`source.origin_changed` lists adopted items whose origin has changed since they were adopted or last approved, while
the source copy hasn't, and the origin's content is neither held nor rejected; `colctl refresh <item>` holds it for
review. Any entry there makes the exit code 1. When a profile's plugin files can't be read,
there is one item `{ "kind": "plugin", "name": "*", "state": "invalid" }` with the error as its hint.
