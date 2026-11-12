# Interfaces: Quarantine

_Cycle 4 · Refine · 2026-11-12_ — additions to cycles [1](../claude-profile-sync/interfaces.md),
[2](../codex-cli-support/interfaces.md) and [3](../plugins-from-source/interfaces.md).

## Command line

```
colctl update [<name>…] [--check] [--rev <commit-ish>] [--dry-run]     # now holds instead of installing
colctl refresh [skills/<name> | agents/<name>.md …] [--dry-run]       # now holds instead of copying
colctl review [<name>] [--json]
colctl approve (<name>… | --all) [--dry-run]
colctl reject (<name>… | --all) [--dry-run]
```

`<name>` is a source name (`superpowers`), `skills/<name>` or `agents/<name>.md`.

| Command | 0 | 1 | 2 |
|---|---|---|---|
| `update` | Fetched; held or up to date | `--check` found a newer, unrejected commit; or a source failed | Usage (as in 1.2) |
| `refresh` | Held or up to date | Something refused | An item has no origin |
| `review` | Nothing held (list), or the named candidate shown | Something is held (list), or the named candidate can't be shown (EC4-12, EC4-13) | Usage, unknown name, nothing held for the name |
| `approve` | Approved and synced | Something refused, or sync reported drift | Usage, unknown name, nothing held for the name |
| `reject` | Rejected | — | Usage, unknown name, nothing held for the name |

`update`, `refresh`, `approve` and `reject` take the run lock (not `update --check`, not `--dry-run`). `review`
works offline and writes nothing.

## Text output

```
$ colctl update
superpowers: 8ca22db → 22f0b46 (12 new commits), held for review; colctl review superpowers
other: up to date

$ colctl review
sources/superpowers   8ca22db → 22f0b46   12 commits   3 flags
skills/twg-jira       ~/.config/twg/skills/twg-jira   2 files   1 flag

$ colctl review superpowers
superpowers  8ca22db → 22f0b46  (https://github.com/obra/superpowers)

commits (12)
  22f0b46 Release 7.0.2
  …

files (5)
  A  hooks/hooks.json
  M  .claude-plugin/plugin.json
  A  scripts/session-start.sh    (+x)
  M  skills/brainstorming/SKILL.md
  D  scripts/old.sh

flags (3)
  hook        hooks/hooks.json                 adds hooks
  manifest    .claude-plugin/plugin.json       plugin manifest changed
  executable  scripts/session-start.sh         new executable file

diff
<git diff output>
```

A candidate older than the pin prints `goes back n commits` in place of `commits (n)`.

`status` adds, in the source section:
```
  pending-review       sources/superpowers  8ca22db → 22f0b46  → colctl review superpowers
```
and list plugins become `unreviewed` items in each harness.

## JSON

`colctl review --json` (and with a name):

```json
{
  "version": 1,
  "candidates": [
    {
      "key": "sources/superpowers",
      "kind": "source",
      "name": "superpowers",
      "from": "8ca22dba9a94f28898bbce59f2537ff4d87c747d",
      "to": "22f0b46…",
      "direction": "forward",
      "repo": "https://github.com/obra/superpowers",
      "held_at": 1794500000,
      "commits": [{ "rev": "22f0b46…", "subject": "Release 7.0.2", "author": "Jesse Vincent", "date": "2026-11-10" }],
      "files": [{ "path": "hooks/hooks.json", "status": "added", "old_path": null, "executable": false, "symlink": null }],
      "flags": [{ "kind": "hook", "path": "hooks/hooks.json", "reason": "adds hooks" }],
      "error": null
    },
    {
      "key": "skills/twg-jira",
      "kind": "skill",
      "name": "twg-jira",
      "from": "sha256:…",
      "to": "sha256:…",
      "origin": "/Users/me/.config/twg/skills/twg-jira",
      "held_at": 1794500000,
      "commits": [],
      "files": [],
      "flags": [],
      "error": null
    }
  ]
}
```

- `kind`: `source`, `skill` or `agent`.
- `direction`: `forward` or `back` (EC4-08), sources only.
- `status`: `added`, `modified`, `deleted`, `renamed`, `mode`.
- `error`: the refusal from EC4-12/13 when the candidate can't be read, else null.
- With a name, the one candidate also has `"diff": "<unified diff>"`.

`colctl status --json` gains a top-level `"quarantine"` array with the same objects minus `commits`, `files`,
`diff` (it carries `"flags": <count>` instead), and `"state": "pending-review"`. Item state `"unreviewed"` joins
the item states.

## State (`.state/state.json`)

```json
{
  "version": 1,
  "harnesses": { … },
  "origins": { … },
  "quarantine": {
    "sources/superpowers": { "kind": "source", "from": "<40 hex>", "to": "<40 hex>", "held_at": 1794500000 },
    "skills/twg-jira": { "kind": "skill", "origin": "/abs/path", "base": "sha256:…", "hash": "sha256:…", "held_at": 1794500000 }
  },
  "rejected": {
    "sources/superpowers": ["<40 hex>"],
    "skills/twg-jira": ["sha256:…"]
  }
}
```

- `from`: the pin when held. `to`: the held commit.
- `base`: the source copy's hash when held. `hash`: the snapshot's hash.
- `held_at`: Unix seconds.
- Both maps are omitted when empty. The `version` stays 1: the new fields are optional, so 1.x files load.
  colctl 1.x refuses a 2.0 state file that holds anything, which is expected for a major release.

## Files

```
~/.colctl/.state/quarantine/skills/<name>/     exact copy of the origin at hold time (symlinks kept as symlinks)
~/.colctl/.state/quarantine/agents/<name>.md
~/.colctl/plugins/<name>/  ref refs/colctl/candidate → the held commit
```

## Git calls

| Purpose | Call |
|---|---|
| Hold | `git update-ref refs/colctl/candidate <rev>` in the clone |
| Check it's intact | `git rev-parse --verify -q refs/colctl/candidate^{commit}` equals the state's `to` |
| Commits | `git log --format=%H%x00%s%x00%an%x00%as <from>..<to>` (or `<to>..<from>` going back) |
| Files | `git diff --raw -z -M <from> <to>` (status, modes, paths) |
| Diff | `git diff -M <from> <to>` |
| File content for risk checks | `git show <to>:<path>` (and `<from>:<path>` for changed keys) |
| Skill files and diff | `git diff --no-index --raw -z` and `git diff --no-index` between the source copy and the snapshot (exit 1 means "different", not an error) |
| Release | `git update-ref -d refs/colctl/candidate` |

## Risk rules

Applied to added, modified, renamed and mode-changed files; never to deleted ones. A file can get several flags.

| Flag | Matches | Reason |
|---|---|---|
| `hook` | any `hooks.json`; a `plugin.json` whose `hooks` key is added or changed | adds hooks / changes hooks |
| `mcp` | `.mcp.json`; a `plugin.json` whose `mcpServers` key is added or changed | adds or changes MCP servers |
| `lsp` | `.lsp.json`; a `plugin.json` whose `lspServers` key is added or changed | adds or changes LSP servers |
| `executable` | the executable bit is set on a new file, or newly set, or the content of an executable file changed | new / newly / changed executable file |
| `script` | extension `.sh .bash .zsh .fish .py .js .mjs .cjs .ts .rb .pl .ps1`, or content starting with `#!` | new or changed script |
| `binary` | content that isn't valid UTF-8 | new or changed binary file |
| `symlink` | a new symlink or a changed target | links to <target> (+ "points outside" when it leaves the repo or skill) |
| `allowed-tools` | `.md` frontmatter `allowed-tools` added or changed; `tools` in an agent's frontmatter | grants <tools> (names Bash when present) |
| `inline-shell` | an added Markdown line containing `` !` `` | runs a shell command when loaded |
| `manifest` | any file under `.claude-plugin/`, `.codex-plugin/` or `.agents/plugins/` | plugin manifest changed |

## Constraints

- No new crate dependencies; diffs come from `git`.
- `sync` stays offline and never reads candidates.
- First-time `add source` and `adopt` are unchanged.
- The `--json` shapes are what the cycle 6 TUI will read; changing them later is a breaking change.
