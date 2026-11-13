# Interfaces: convert

## Command line

```
colctl convert <source> [--dry-run] [--json]
colctl convert --undo <source> [--dry-run] [--json]
```

Exit codes as everywhere: 0 done or nothing to do, 1 failed or refused, 2 usage, 3 locked.

### Text

```
$ colctl convert qdrant-skills
! Held qdrant-skills's Codex package for review · built from 1a2b3c4

  Carries   10 skills
  Hooks     none
  Not carried over   nothing

→ colctl review qdrant-skills
```

With hooks (the hooks fixture):

```
! Held dbx's Codex package for review · built from 1a2b3c4

  Carries   2 skills · 3 hooks (UserPromptSubmit, SessionStart, PostToolUse on Bash)
  Codex never runs   Notification hook · WebFetch in PostToolUse's matcher
  Not carried over   command setup · agent helper · MCP server github

→ colctl review dbx
```

After approve, when there are hooks: `Codex asks you to trust its hooks before they run: open Codex and use /hooks.`

### JSON (`convert --json`)

```json
{"source": "qdrant-skills", "held": true, "rev": "<40 hex>", "digest": "<sha256 hex>",
 "skills": ["qdrant-clients-sdk", "…"],
 "hooks": [{"event": "SessionStart", "matcher": "startup|clear", "type": "command", "command": "…"}],
 "never_runs": ["Notification hook (Codex has no such event)"],
 "not_carried": [{"kind": "command", "name": "setup"}, {"kind": "agent", "name": "helper"},
                 {"kind": "mcp", "name": "github"}],
 "dry_run": false}
```

`--undo --json`: `{"source": "…", "undone": true, "dry_run": false}`.

## Config

```toml
[[source]]
name = "qdrant-skills"
repo = "https://github.com/qdrant/skills"
rev = "…"
plugin = "qdrant"
convert = ["codex"]      # new, optional; only "codex"; written on approve
```

Validation: an unknown tool in `convert` is an invalid config (exit 2, as other config errors).

## State

Candidate key `conversions/<name>`:

```json
{"kind": "conversion", "rev": "<40 hex>", "digest": "<sha256 hex>", "held_at": 1763050000}
```

The package for review is kept in `.state/quarantine/conversions/<name>/`. The harness state records an installed
converted plugin's rev as `<rev>+<first 12 of digest>`, so a different digest reinstalls (EC7-20).

## The package

`~/.colctl/plugins/.converted/<name>/`, built in `.converted/.tmp-<name>` and renamed (EC7-19):

- every file of the plugin folder except `.git/`, `.claude-plugin/`, links leaving the folder (EC7-09), and the hooks
  file, which is rewritten;
- `.codex-plugin/plugin.json`: `name`, `version`, `description`, `author`, `homepage`, `repository`, `license`,
  `keywords` copied when present; `"skills": "./skills/"` when there are skills (or the manifest's own skills path);
  `"interface": {"displayName": <name>}`. Keys sorted, 2-space indent, trailing newline;
- `hooks/hooks.json` when any hook is kept;
- files written with mode 0644, or 0755 when executable in the clone; directories 0755.

Digest: SHA-256 over the sorted list of `(path, mode, sha256(content))`.

The `colctl-<name>` marketplace's Codex entry points at `./plugin-codex`, a link to the package (cycle 6 mechanism).

## Hook mapping

Events Codex runs: PreToolUse, PermissionRequest, PostToolUse, PreCompact, PostCompact, UserPromptSubmit,
SubagentStart, SubagentStop, Stop, SessionStart, SessionEnd, PreModelRequest, PostModelRequest, Interrupt. Others are
left out and reported.

Only `"type": "command"` hooks are kept; others are reported.

Matchers on tool events are split on `|` when every part is a plain name (letters, digits, `_`, `*`). Parts Codex never
fires (`Read`, `Grep`, `Glob`, `LS`, `WebFetch`, `WebSearch`, `NotebookEdit`, `NotebookRead`, `TodoWrite`,
`MultiEdit`, `Task`, `ExitPlanMode`) are removed and reported; a group with no part left is left out. Other matchers
(regexes, `mcp__…`, `Bash`, `Edit`, `Write`, `Agent`, `*`, empty) are kept as written. Commands are never changed.

Source: [Codex hooks](https://learn.chatgpt.com/docs/hooks.md) (events, matchers, `CLAUDE_PLUGIN_ROOT`, trust review).

## Status

| Situation | Codex item state | Fix / note |
|---|---|---|
| Not converted, Claude packaging exists | `unsupported` | `colctl convert <name>` |
| Conversion held | `unsupported`, plus a held row in Needs attention | `colctl review <name>` |
| Converted and installed | `ok`, `"converted": true` | — |
| Package missing, edited or from another digest | `outdated` | `colctl sync` |
