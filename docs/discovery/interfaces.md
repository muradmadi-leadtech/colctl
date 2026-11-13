# Interfaces: Discovery

_Cycle 5 · Refine · 2026-11-13_ — additions to cycles [1](../claude-profile-sync/interfaces.md) to
[4](../quarantine/interfaces.md).

## Command line

```
colctl search [<term>…] [--tool claude|codex] [--marketplace <m>] [--developer <d>] [--installed] [--limit <n>] [--json]
colctl show <id-or-name> [--json]
colctl developers [<developer>] [--json]
colctl marketplace list [--json]
colctl marketplace refresh [--dry-run]
colctl connectors [<term>…] [--limit <n>] [--json]
colctl add source <repo-or-id> [--rev <commit-ish>] [--name <name>] [--plugin <plugin>] [--dry-run]
```

| Command | 0 | 1 | 2 |
|---|---|---|---|
| `search`, `developers <d>`, `connectors` | Results | No results, no catalogs, or (connectors) a profile failed | Usage; connectors without codex or Codex profiles |
| `show` | Shown | — | Unknown or ambiguous |
| `developers`, `marketplace list` | Listed | No catalogs | Usage |
| `marketplace refresh` | All calls succeeded | A call failed or a tool was missing | Usage |

None of these take the lock, except `add source` (as in cycle 3). Only `marketplace refresh`, `add source` and
`connectors` run `claude` or `codex`; only `marketplace refresh` and `add source` use the network.

## Text output

```
$ colctl search api security
42crunch-api-security-testing@claude-plugins-official   42crunch-ai   Automate API security directly in Claude Code wi…
security-guidance@anthropic-plugin-directory            anthropics    Security reminders while editing  [installed: claude]
2 of 2 results

$ colctl show superpowers
superpowers
  ids          superpowers@claude-plugins-official, superpowers@anthropic-plugin-directory, superpowers@superpowers-dev
  catalogs     claude-plugins-official (claude), anthropic-plugin-directory (claude), colctl
  developer    obra
  author       Jesse Vincent
  description  Core skills library for Claude Code: TDD, debugging, collaboration patterns…
  keywords     skills, tdd, debugging
  repo         https://github.com/obra/superpowers
  version      7.0.2 (commit 22f0b46)
  installed    claude, kilo-claude, codex, kilo-codex
  add          already source plugin superpowers

$ colctl developers
obra            3 plugins   claude-plugins-official, anthropic-plugin-directory, colctl
anthropics     58 plugins   …

$ colctl marketplace list
claude-plugins-official      claude   github:anthropics/claude-plugins-official   317 plugins   claude, kilo-claude
anthropic-plugin-directory   claude   built in                                   3663 plugins   claude, kilo-claude
superpowers-dev              claude   ~/.colctl/plugins/superpowers                1 plugin    claude, kilo-claude   managed
superpowers-dev              codex    ~/.colctl/plugins/superpowers                1 plugin    codex, kilo-codex     managed
```

(`developers` is sorted by plugin count, then name; the lines above are illustrative.)

## JSON

```json
{
  "version": 1,
  "matched": 2,
  "results": [
    {
      "name": "superpowers",
      "display_name": "Superpowers",
      "ids": ["superpowers@claude-plugins-official", "superpowers@anthropic-plugin-directory"],
      "catalogs": [{ "name": "claude-plugins-official", "tool": "claude" }, { "name": "anthropic-plugin-directory", "tool": "claude" }],
      "kind": "plugin",
      "developer": "obra",
      "author": "Jesse Vincent",
      "description": "…",
      "keywords": ["skills"],
      "category": null,
      "homepage": null,
      "repo": "https://github.com/obra/superpowers",
      "subpath": null,
      "version": "7.0.2",
      "commit": "22f0b46…",
      "installed": ["claude", "codex"],
      "source_plugin": "superpowers",
      "add": "already source plugin superpowers"
    }
  ],
  "warnings": ["…"]
}
```

- `kind`: `plugin` or `connector` (connectors only from `colctl connectors`).
- `add`: the command to run, or why there's none.
- `show --json` prints one result object (with `"version": 1` around it as `{"version": 1, "result": {…}}`).
- `developers --json`: `{"version": 1, "developers": [{"name", "plugins", "catalogs"}]}`; with a name, the `search`
  shape.
- `marketplace list --json`: `{"version": 1, "marketplaces": [{"name", "tool", "source", "plugins", "profiles",
  "managed"}]}`.

## Catalog sources

| Catalog | File | Entry |
|---|---|---|
| Claude marketplace `m` | `known_marketplaces.json` → `installLocation` → `.claude-plugin/marketplace.json` | `<name>@m` |
| Plugin directory | `plugins/plugin-directory-cache-v2.json`, `schemaVersion` 2, listings with surface `claude_code` | `<entryNames[id]>@anthropic-plugin-directory` |
| Codex marketplace `m` | `config.toml` `[marketplaces.m]` with `source_type = "local"` → `source` → `.agents/plugins/marketplace.json` | `<name>@m` |
| colctl sources | `config.toml` `[[source]]` and the clone's packaging | the packaged ids |
| Codex connectors | `codex plugin list --available --json` → `available[]` with `source.source = "remote"` | `pluginId` |

Plugin source → repo and subpath (shared with `origin`):

| `source` | Repo | Subpath | Commit |
|---|---|---|---|
| `"./path"` (string) | the marketplace's own repo | `path` | — |
| `{"source": "github", "repo": "o/r"}` | `https://github.com/o/r` | — | `sha` if present |
| `{"source": "url" \| "git", "url": U}` | `U` (relative → marketplace repo + path) | — | `sha` if present |
| `{"source": "git-subdir", "url": U, "path": P}` | `U` | `P` | `sha` |
| `{"source": "command", …}` | none | — | — |
| directory listing `repository` | `url` | `path` | `reviewed_commit` |

A marketplace's own repo: `github` → `https://github.com/<repo>`; `git`/`url` → the URL; `directory`/local → the
folder (a local path; its developer comes from the author).

**Normalised repo** (for merging and developers): lowercase scheme and host, `git@host:o/r` → `https://host/o/r`, drop
`.git` and trailing `/`; `host/o/r#p` → repo `https://host/o/r`, subpath `p`. Owner and repo name are lowercased for
comparison only.

## Ranking

Each term must hit at least one field. Score per term: exact name 100, name contains 50, keyword or tag 30,
description 10, developer or author 5. A result's score is the sum; ties by name, then first id.

## Constraints

- No new crate dependencies.
- Discovery never writes outside what `add source` already writes (cycle 3).
- The JSON shapes are a contract for the cycle 6 TUI.
