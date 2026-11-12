# Interfaces: Claude profile sync

_Cycle 1 · Refine · 2026-11-12_

## Command line

```
colctl init   [--dry-run]
colctl status [--harness <name>] [--json]
colctl sync   [--harness <name>] [--dry-run]
colctl add skill  <path|git-url> [--subdir <path>] [--name <name>] [--dry-run]
colctl add agent  <file.md> [--dry-run]
colctl add plugin <name@marketplace> [--marketplace <source>] [--dry-run]
colctl rm  skill|agent|plugin <name> [--dry-run]
colctl adopt [<harness> [<item>]] [--dry-run]
colctl --version | version
```

`<item>` for `adopt` is `skills/<name>`, `agents/<name>.md`, `CLAUDE.md`, or `plugin <name@marketplace>`.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success; for `status`, everything is in sync |
| 1 | Out of sync, or one or more actions failed or were refused (details printed) |
| 2 | Usage, config, state or environment error (bad flag, invalid config, git missing…); nothing changed |
| 3 | Another colctl holds the lock |

### Output

Human output goes to stdout; warnings and errors to stderr. Every failed action prints one line with the item, the
harness and the reason, plus a fix hint. `status --json` prints exactly one JSON document to stdout:

```json
{
  "version": 1,
  "source": { "path": "/Users/me/.colctl", "uncommitted": ["skills/foo/SKILL.md"] },
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

Harness `state`: `ok` | `harness-missing` | `invalid`. Item `kind`: `instructions` | `skill` | `agent` | `marketplace`
| `plugin`. Item `state`: `ok` | `missing` | `broken` | `foreign` | `edited` | `invalid` | `plugin-missing` |
`marketplace-missing`.

## `config.toml`

```toml
version = 1                      # required, must be 1

[[harness]]                      # zero or more
name = "kilo-claude"             # required, unique, [a-z0-9-]+
kind = "claude"                  # required; only "claude" in cycle 1
path = "~/Library/Application Support/kilo-proxy-headless/profiles/claude"  # required; absolute or ~-prefixed
exclude = ["skills/foo"]         # optional; items colctl must stay out of

[marketplaces]                   # optional; name = source
sadr = "github:muradmadi/sadr"   # "github:owner/repo" | "https://…" | absolute path

[plugins]
enabled = ["sadr@sadr", "superpowers@anthropic-plugin-directory"]  # name@marketplace
```

Rules: unknown keys are errors. A plugin's marketplace must be in `[marketplaces]` unless it is
`anthropic-plugin-directory`. colctl edits this file only with `toml_edit`, so formatting and comments survive.

## Source layout

```
<source>/
  config.toml
  AGENTS.md                 # optional
  skills/<name>/SKILL.md    # + any other files
  agents/<name>.md
  .gitignore                # contains ".state/"
  .state/state.json
  .state/lock
```

Item names match `^[A-Za-z0-9][A-Za-z0-9._-]*$`.

## `.state/state.json`

```json
{
  "version": 1,
  "harnesses": {
    "kilo-claude": {
      "generated": { "CLAUDE.md": "sha256:<hex>" },
      "marketplaces": ["sadr"],
      "plugins": ["sadr@sadr"]
    }
  }
}
```

Links are not recorded: a symlink whose target is inside `<source>` is owned by definition.

`.state/lock` contains the holder's pid. Written with create-exclusive; removed on exit.

## Generated `CLAUDE.md`

Exactly these two lines, with `<source>` written as `~/.colctl` when the source is the default and as an absolute
path otherwise:

```
<!-- Managed by colctl. Edit <source>/AGENTS.md instead; this file is regenerated. -->
@<source>/AGENTS.md
```

## Links

`<harness>/skills/<name>` → `<source>/skills/<name>` and `<harness>/agents/<name>.md` → `<source>/agents/<name>.md`,
absolute targets. Created as `<parent>/.colctl-tmp-<pid>-<name>` then renamed into place.

## Calling `claude`

| Action | argv |
|---|---|
| Add marketplace | `claude plugin marketplace add <arg>` where `<arg>` is `owner/repo` for `github:owner/repo`, otherwise the source as written |
| Install plugin | `claude plugin install <name@marketplace>` |
| Uninstall plugin | `claude plugin uninstall <name@marketplace>` |
| Remove marketplace | `claude plugin marketplace remove <name>` |

Environment for every call: inherited, **minus** `CLAUDE_CONFIG_DIR`, `CLAUDE_SECURESTORAGE_CONFIG_DIR`,
`ANTHROPIC_*`; **plus** `CLAUDE_CONFIG_DIR=<harness path>` unless the harness's canonical path is the canonical
`~/.claude`. stdin is `/dev/null`. Timeout 300 s. A non-zero exit, timeout or spawn failure is an action failure
(AC-30).

Reading plugin state: `<harness>/plugins/installed_plugins.json` (`.plugins` keys = installed ids) and
`<harness>/plugins/known_marketplaces.json` (keys = marketplace names, `.source` = origin).

## Calling `git`

`git init`, `git add -A`, `git -c user.name=colctl -c user.email=colctl@localhost commit -m <msg>` (the `-c` pair only
when no identity is configured), `git status --porcelain`. All run with `-C <source>`.

Commit messages: `colctl: init`, `colctl: sync`, `colctl: add <kind> <name>`, `colctl: rm <kind> <name>`,
`colctl: adopt <kind> <name>`.
