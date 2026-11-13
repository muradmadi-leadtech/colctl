# colctl review

```
colctl review [<name>] [--json]
```

Shows what a held new version would change, before anything installs it. `<name>` is a source name
(`superpowers`), `skills/<name>` or `agents/<name>.md`. `review` works offline, writes nothing and doesn't take the
lock.

Versions are held by [`update`](update.md) (source plugins) and [`refresh`](refresh.md) (taken-over skills and
agents). Decide them with [`approve`](approve.md) or [`reject`](reject.md).

## Without a name

A verdict, then one row per held version:

```
! 2 versions held for review

  sources/superpowers  8ca22db → 22f0b46              12 commits  3 files flagged
  skills/twg-jira      ~/.config/twg/skills/twg-jira  2 files     1 file flagged

→ colctl review <name> shows one in full
```

A version older than the pin shows `goes back n commits`. A held version that can't be read shows `✗ error: …`.
In a terminal, `colctl review -i` picks one from a list.

## With a name

```
! superpowers  8ca22db → 22f0b46 · 12 commits · 3 files flagged
  https://github.com/obra/superpowers

Risk flags
  hooks/hooks.json            hook: adds hooks
  scripts/session-start.sh    executable · script: new executable file; new script
  .claude-plugin/plugin.json  manifest: plugin manifest changed

Commits (12)
  22f0b46  Release 7.0.2
  …

Files (5)
  A  hooks/hooks.json
  M  .claude-plugin/plugin.json
  A  scripts/session-start.sh  (+x)

Diff
<the unified diff>

→ colctl approve superpowers   or   colctl reject superpowers
```

**Risk flags come first**, one line per file with its kinds joined by ` · `, files ordered by their most serious kind
(hook, mcp, lsp, executable, script, binary, symlink, allowed-tools, inline-shell, manifest). They're never cut: a
reason that doesn't fit goes on the next line. In a terminal the diff is coloured (+ green, - red) and the whole view
goes through the pager when it's taller than the screen.

- **commits**: from the pin to the held commit. For a held version older than the pin, the heading is
  `goes back n commits (removed)` and lists what it takes out.
- **files**: `A` added, `M` modified, `D` deleted, `R` renamed, `T` mode or type changed. `(+x)` marks an executable,
  `-> target` a symlink.
- **flags**: see below.
- **diff**: for a source plugin, `git diff` between the two commits, over the **whole repo** (files outside the
  plugin's folder, like a marketplace manifest or a shared script, can matter too). For a skill or agent, the source
  copy against the held copy.
- A **warning** line appears when the source's pin was changed by hand after the version was held; `approve` will
  refuse it.

File names, commit subjects and authors come from someone else's repo, so control characters in them are printed
escaped (`\x1b`, `\n`) and can't drive your terminal. The diff keeps tabs and newlines and escapes other control
characters.

## Risk flags

A flag points at a file that lets code run or widens what an agent may do. It doesn't judge the code: a version with
no flags isn't "safe", and one with flags isn't "bad". Read the flagged files first.

| Flag | Matches |
|---|---|
| `hook` | any `hooks.json`; a `hooks` key added or changed in a `plugin.json` |
| `mcp` | `.mcp.json`; an `mcpServers` key added or changed in a `plugin.json` |
| `lsp` | `.lsp.json`; an `lspServers` key added or changed in a `plugin.json` |
| `executable` | a new executable file, a file that becomes executable, or a changed executable |
| `script` | a new or changed `.sh .bash .zsh .fish .py .js .mjs .cjs .ts .rb .pl .ps1` file, or one starting with `#!` |
| `binary` | a new or changed file that isn't text (UTF-8) |
| `symlink` | a new symlink or a changed target; "points outside" when it leaves the repo or skill |
| `allowed-tools` | `allowed-tools` added or changed in Markdown frontmatter (or `tools` in an agent); names Bash when granted |
| `inline-shell` | a new Markdown line with `` !`command` ``, which Claude runs when the file loads |
| `manifest` | any change under `.claude-plugin/`, `.codex-plugin/` or `.agents/plugins/` |

Deleted files are never flagged.

## Refusals

| Case | Shown as |
|---|---|
| The held copy of a skill or agent was edited after it was held | "the held copy of <key> changed since it was held; reject it and refresh again" |
| The held commit is gone from the clone (re-cloned, ref removed) | "the held commit for <name> is missing; run colctl update <name>" |

## `--json`

```json
{
  "version": 1,
  "candidates": [
    {
      "key": "sources/superpowers",
      "kind": "source",
      "name": "superpowers",
      "from": "8ca22db…",
      "to": "22f0b46…",
      "direction": "forward",
      "repo": "https://github.com/obra/superpowers",
      "held_at": 1794500000,
      "commits": [{ "rev": "…", "subject": "Release 7.0.2", "author": "…", "date": "2026-11-10" }],
      "files": [{ "path": "hooks/hooks.json", "status": "added", "old_path": null, "executable": false, "symlink": null }],
      "flags": [{ "kind": "hook", "path": "hooks/hooks.json", "reason": "adds hooks" }],
      "error": null,
      "warning": null
    }
  ]
}
```

Skills and agents have `kind` `skill` or `agent`, `from`/`to` as content hashes, `origin` instead of `repo` and
`direction`, and an empty `commits`. With a name, the candidate also has `"diff"`. Strings are the raw values (not
escaped).

## Exit codes

| Code | When |
|---|---|
| 0 | Nothing held (no name), or the named version was shown |
| 1 | Something is held (no name), or the named version can't be read (see Refusals) |
| 2 | Unknown name, or nothing held for it; no `git` |
