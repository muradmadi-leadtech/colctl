# colctl convert

```
colctl convert <source> [--dry-run] [--json]
colctl convert --undo <source> [--dry-run] [--json]
```

Packages a [source plugin](../explanation/plugins-from-source.md) whose repo only packages for Claude so Codex can use
it too. The package is built from the pinned clone (which is never written to) and **held for review**: nothing
installs until you approve it.

```
$ colctl convert qdrant-skills
! Held qdrant-skills's Codex package for review · built from 1a2b3c4

  Carries   10 skills

→ colctl review qdrant-skills
```

Then `colctl review qdrant-skills` shows the package (its skills, every hook with its event, matcher and command,
risk flags, what Codex never runs and what isn't carried over), `colctl approve qdrant-skills` installs it in every
Codex profile, and `colctl reject qdrant-skills` drops it.

Approving adds `convert = ["codex"]` to the source in `config.toml` (committed). From then on:

- `colctl sync` keeps the package in `~/.colctl/plugins/.converted/<source>` as approved, rebuilding it if it's missing
  or edited, and Codex installs it from colctl's `colctl-<source>` marketplace;
- `colctl update` holds a new commit together with its new package: `review` shows what changes in both;
- if the repo starts packaging for Codex itself, sync uses that and drops the conversion, and says so.

## What carries over

| Part of the plugin | In the Codex package |
|---|---|
| Skills (`skills/`, or the manifest's `skills` path) | As they are |
| Hooks (`hooks/hooks.json`, the manifest's `hooks` path, or inline) | Every `command` hook on an event Codex runs, unchanged. Hooks on events Codex doesn't have (`Notification`), non-command hooks (`prompt`), and tool names Codex never fires in a matcher (`Read`, `Grep`, `Glob`, `WebFetch`, `WebSearch`, …) are left out and listed under **Codex never runs** |
| Every other file of the plugin folder | Copied, so the scripts hooks run are where they expect (`${CLAUDE_PLUGIN_ROOT}` works: Codex sets it) |
| Commands, agents, MCP servers, LSP servers, output styles | Not carried over; listed by name |
| Links leaving the plugin folder | Not followed; listed |

Codex asks you to trust a plugin's hooks before they run (`/hooks` in Codex), and again when a hook's script
changes. colctl reminds you when you approve a package with hooks.

The same commit always gives the same package, byte for byte; its digest is part of what's installed, so a colctl
that converts differently reinstalls once.

## Options

| Option | Effect |
|---|---|
| `--undo` | Stop converting: drop a held package, or remove `convert` (committed), the package and its marketplace entry, then sync, which uninstalls it from Codex profiles |
| `--dry-run` | Say what would be held or removed; change nothing |
| `--json` | One JSON document (below) |

## JSON

```json
{"source": "dbx", "held": true, "dry_run": false, "rev": "<40 hex>", "digest": "<64 hex>",
 "skills": ["edge", "scaling"],
 "hooks": [{"event": "SessionStart", "matcher": "startup", "type": "command", "command": "…"}],
 "never_runs": ["Notification hook (Codex has no such event)"],
 "not_carried": [{"kind": "command", "name": "setup"}, {"kind": "MCP server", "name": "github"}]}
```

`--undo --json`: `{"source": "…", "undone": true, "dry_run": false}`. When there's nothing to do (already converted,
already packaged for Codex) the document has `"held": false` and a `"note"`.

## Exit codes

**0** held, undone, or nothing to do; **1** refused (no Codex profile, nothing Codex can use, a hooks file that doesn't
parse, a path leaving the plugin); **2** no such source; **3** another colctl holds the lock.
