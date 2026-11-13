# colctl search

```
colctl search [<term>…] [--tool claude|codex] [--marketplace <m>] [--developer <d>] [--installed] [--limit <n>] [--json]
```

Searches every plugin catalog your harnesses already keep (see [Where the catalogs come from](../explanation/catalogs.md)):
Claude's marketplaces and Anthropic's plugin directory, Codex's marketplaces, and colctl's own sources. It works
offline, writes nothing, takes no lock and runs neither `claude` nor `codex`.

```
$ colctl search api security
42crunch-api-security-testing@claude-plugins-official  42crunch-ai  Automate API security directly in Claude Code wi…
security-guidance@anthropic-plugin-directory           anthropics   Security reminders while editing  [installed: claude]
2 of 2 results
```

Each line: the plugin's id, its developer, its description cut to the line, and where it's installed.

## Matching

- Every term must appear (case-insensitive) in the name, display name, keywords or tags, description, developer or
  author.
- Results rank by where the terms hit: exact name (100), name (50), keyword (30), description (10), developer or
  author (5), summed over the terms; ties by name.
- With no terms, everything is listed by name.

## Merging

A plugin listed in several catalogs is one result when its repo, folder and name are the same, however the repo URL is
written (`git@github.com:o/r`, `https://github.com/O/r.git`). The result lists every id and catalog. Its first id is a
marketplace's when there is one, so `add` names something you can find.

## Developer

The owner of the plugin's repo (`obra` for `github.com/obra/superpowers`, lowercased), or for a repo that's a local
folder or no repo at all, the manifest's author name, or `unknown`. See [`colctl developers`](developers.md).

## Filters

| Flag | Keeps |
|---|---|
| `--tool claude\|codex` | plugins listed for that tool |
| `--marketplace <m>` | plugins in that marketplace (`anthropic-plugin-directory` for the directory, `colctl` for sources) |
| `--developer <d>` | that developer's plugins (case-insensitive) |
| `--installed` | plugins installed in at least one profile |
| `--limit <n>` | at most `n` results (default 20; `0` for all). The footer says how many matched |

Codex's app connectors aren't searched here; see [`colctl connectors`](connectors.md).

## `--json`

```json
{
  "version": 1,
  "matched": 1,
  "results": [
    {
      "name": "superpowers", "display_name": "Superpowers",
      "ids": ["superpowers@claude-plugins-official", "superpowers@anthropic-plugin-directory"],
      "catalogs": [{ "name": "claude-plugins-official", "tool": "claude" }],
      "kind": "plugin", "developer": "obra", "author": "Jesse Vincent",
      "description": "…", "keywords": ["skills"], "category": null, "homepage": null,
      "repo": "https://github.com/obra/superpowers", "subpath": null,
      "version": "7.0.2", "commit": "22f0b46…",
      "installed": ["claude", "codex"], "source_plugin": "superpowers",
      "add": "already source plugin superpowers"
    }
  ],
  "warnings": []
}
```

`add` is the command to run (`colctl add source <id>`, `colctl add plugin <id>` for plugins without a repo) or why
there's none. Strings are raw in JSON; in text, control characters are escaped.

## Warnings and exit codes

A catalog that can't be read (a manifest that isn't JSON, a marketplace folder that's gone, a plugin-directory cache
with an unknown schema) is skipped with `warning: …` on stderr; the rest is searched.

Exit codes: **0** with results; **1** when nothing matches or no catalogs exist ("no catalogs found; start Claude or
Codex once…"); **2** usage errors.
