# Interfaces: Codex CLI support

_Cycle 2 · Refine · 2026-11-12_ — additions to [cycle 1's interfaces](../claude-profile-sync/interfaces.md).

## Command line

```
colctl refresh [<item>…] [--dry-run]            # items: skills/<name>, agents/<name>.md; default: all with an origin
colctl add plugin <name@marketplace> [--codex] [--marketplace <source>] [--dry-run]
colctl rm  plugin <name@marketplace> [--codex] [--dry-run]
colctl adopt <harness> AGENTS.md | agents/<name>.toml | skills/<name> | plugin <id>
```

Without `--codex`, `add`/`rm plugin` work on the Claude list exactly as in 1.0. Exit codes are unchanged.

## `config.toml`

```toml
[[harness]]
name = "kilo-codex"
kind = "codex"          # "claude" | "codex" | "agents"
path = "~/Library/Application Support/kilo-proxy-headless/profiles/codex"
exclude = ["AGENTS.md", "agents/reviewer.toml", "plugins/x@m"]

[[harness]]
name = "agents"
kind = "agents"
path = "~/.agents"
exclude = ["skills/twg-jira"]

[codex.marketplaces]    # optional; same source formats as [marketplaces]
superpowers-dev = "github:obra/superpowers"

[codex.plugins]
enabled = ["superpowers@superpowers-dev"]   # marketplace must be in [codex.marketplaces] or be "openai-curated"
```

| Kind | Items colctl manages | Valid `exclude` entries |
|---|---|---|
| `claude` | `CLAUDE.md`, `skills/*`, `agents/*.md`, `[plugins]` | as in 1.0 |
| `codex` | `AGENTS.md`, `agents/*.toml`, `[codex.plugins]` | `AGENTS.md`, `agents/<name>.toml`, `plugins/<id>` |
| `agents` | `skills/*` | `skills/<name>` |

`init` knows: `codex` → `~/.codex`, `kilo-codex` → `~/Library/Application Support/kilo-proxy-headless/profiles/codex`,
`agents` → `~/.agents` (each registered only if it's an existing folder).

## Generated agent TOML

`<codex harness>/agents/<stem>.toml`, from `<source>/agents/<stem>.md`:

```toml
# Managed by colctl. Edit <source>/agents/<stem>.md instead; this file is regenerated.
name = "<frontmatter name, or <stem> if absent>"
description = "<frontmatter description>"
developer_instructions = """
<body after the frontmatter, trimmed of leading and trailing blank lines>
"""
```

- `<source>` is written as in `CLAUDE.md` (`~/.colctl` or the absolute `COLCTL_HOME`).
- Values are TOML strings encoded by `toml_edit`; the encoding may vary (e.g. literal vs basic strings), but parsing
  the file always gives back the exact values.
- Every other frontmatter key (`model`, `tools`, `color`, …) is ignored.
- Can't convert (item `invalid`): no frontmatter block, frontmatter isn't valid YAML, `description` missing or empty,
  or an empty body.

## Codex profile state (read, never written)

From `<CODEX_HOME>/config.toml`: keys of `[plugins]` = installed plugin ids (whatever their `enabled`), keys of
`[marketplaces]` = known marketplaces, and for adopt, `[marketplaces.<m>]`'s `source_type`/`source`:

| `source_type` | Config source written by adopt |
|---|---|
| `git` | `source` as is (a URL) |
| `local` | `source` (an absolute path) |
| anything else | adopt refuses |

A missing `config.toml` means nothing is installed; an unparsable one makes the harness's plugins `invalid`.

## Calling `codex`

| Action | argv |
|---|---|
| Add marketplace | `codex plugin marketplace add <arg>` (`owner/repo` for `github:owner/repo`, else the source) |
| Install | `codex plugin add <name@marketplace>` |
| Uninstall | `codex plugin remove <name@marketplace>` |
| Remove marketplace | `codex plugin marketplace remove <name>` |

Environment: inherited, minus `CODEX_HOME` and every `OPENAI_*`, plus `CODEX_HOME=<harness path>` unless the
harness's canonical path is the canonical `~/.codex`. stdin `/dev/null`, own process group, timeout from
`COLCTL_TOOL_TIMEOUT_SECS` (default 300; `COLCTL_CLAUDE_TIMEOUT_SECS` is still honoured for both tools). After a
marketplace add, colctl re-reads `config.toml` to check the name, as for Claude.

## `status --json`

Unchanged shape. Harness `kind` gains `"codex"` and `"agents"`. Codex harness items: `instructions` `AGENTS.md`,
`agent <stem>` (state from the generated TOML), `marketplace`, `plugin`. Agents harness items: `skill`.

## Commit messages

`colctl: add codex plugin <id>`, `colctl: rm codex plugin <id>`, `colctl: adopt codex plugin <id>`; others as in 1.0
(`colctl: adopt instructions <harness>` for a Codex `AGENTS.md`, `colctl: adopt skill <name>` for `~/.agents`).

## Taken-over items

New item states (all kinds): **`linked-elsewhere`** (a skill or agent symlink whose target is outside the source;
was reported as `foreign` in 1.0) and **`origin-changed`** (an adopted item whose origin now differs from what was
adopted while the source copy is unedited). `status --json` items in these states add `"target"` / `"origin"`.

`.state/state.json` gains a top-level map (machine-specific, so it lives in the git-ignored state):

```json
"origins": {
  "skills/twg-jira": { "path": "/Users/me/.config/twg/skills/twg-jira", "hash": "sha256:…" }
}
```

`hash` is the tree hash adopted or last refreshed. `refresh` compares the origin and the source copy against it:
origin changed and source unchanged → copy (staged, verified, renamed into place); both changed → refuse; origin
missing → refuse. Commit message: `colctl: refresh skill <name>` / `colctl: refresh agent <name>`.

## Changes made during Generate

Found while implementing and recorded here so intent and code agree:

1. `origin-changed` is reported for the source copy (`source.origin_changed` in `status --json`, a line under the
   source in human output) rather than as an item state in every harness, because it's about the one source copy.
2. `status --json` items carry `"target"` (the link target of a `linked-elsewhere` item, otherwise `null`).
3. When a tool's link is pointed back at an existing source copy whose origin changed, `adopt` prints
   `relinked <item> in <harness> to the source copy; origin has changes; run colctl refresh <item>`.
4. `refresh` prints `<item> is up to date with <origin>` when there is nothing to pull, and runs `sync` afterwards.
5. Cycle 1 unit tests that built internal types (`Action`, `Item`) were updated mechanically for the T1 refactor; one
   (`ac_02`) used `"codex"` as its example unknown kind and now uses `"cursor"`; `ec_14`'s expectations changed as
   AC2-21 allows. `tests/cli.rs` is otherwise unchanged.
