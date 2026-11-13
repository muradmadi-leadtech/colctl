# Interfaces: CLI output design

_Cycle 6 · Refine · 2026-11-13_ — how every command presents itself. Examples are shown without colour; colours are
listed per token. `--json` output and exit codes are **unchanged** and not covered here.

## Global flags

| Flag | Effect |
|---|---|
| `--color auto\|always\|never` | Colour policy (default `auto`) |
| `-q`, `--quiet` | Only errors and the verdict line; no progress, no hints |
| `-v`, `--verbose` | Also each tool call (`claude …`, `codex …`, `git …`) with its duration, under the step it belongs to |
| `-y`, `--yes` | Answer yes to every confirmation |
| `--no-pager` | Never page |

`-q` with `-v` is a usage error. Global flags work before or after the subcommand.

## Terminal capabilities (`Caps`)

| Capability | Rule |
|---|---|
| Colour | `--color always` → on; `never` → off; `auto` → on only if the stream is a terminal, `NO_COLOR` is unset or empty, and `TERM` isn't `dumb`. `CLICOLOR_FORCE` (non-empty, not `0`) turns it on in `auto` even when piped. Decided separately for stdout and stderr |
| Width | The terminal's columns; 80 when unknown (pipes, CI); clamped to 40–160. `COLUMNS` overrides when set to a number |
| Live progress | Only when stderr is a terminal, not `-q`, not `--json` |
| Prompts and pickers | Only when stdin **and** stdout are terminals, not `--json`, and `CI` is unset |
| Pager | Only when stdout is a terminal, not `--no-pager`, and the text is taller than the terminal. `$PAGER` (split on spaces), else `less -R -F -X`; if it can't start, print directly |

## Tokens

| Token | Colour | Symbol | Used for |
|---|---|---|---|
| `ok` | green | `✓` | done, in sync |
| `attention` | yellow | `!` | needs the person: drift, held versions, refusals |
| `failed` | red | `✗` | errors, failed steps |
| `info` | dim | `·` | informational states (`unsupported`, `unreviewed`, nothing to do) |
| `name` | bold | — | item, profile and source names |
| `command` | cyan | — | commands to run |
| `dim` | dim | — | secondary text: paths, counts, footers |
| `heading` | bold | — | section headings |

Symbols always appear, coloured or not, so meaning never depends on colour.

## Components

- **Verdict**: the first line of a result. `<symbol> <sentence>`, sentence case, no full stop. Optional facts after
  ` · `.
- **Section**: a blank line, a heading, then indented content (2 spaces).
- **Table**: columns sized to their content, separated by 2 spaces, the last flexible column cut with `…` to fit the
  width; never wraps a row. Width is measured in display columns (CJK and emoji count as 2).
- **Item**: `<symbol> <columns…>  → <fix>`; the fix is in the `command` token.
- **Hint**: `→ <command>` on its own line, dim arrow.
- **Warning**: `! warning: <text>` on stderr.
- **Error**: on stderr:
  ```
  ✗ error: <what went wrong>
    <cause, when there is one>
    hint: <what to do>
  ```
  Every error has a hint. Usage errors without a specific one get `hint: colctl <command> --help`. Argument errors
  from the parser (unknown command, missing value) use the same layout; their "did you mean" becomes the hint.
- **Footer**: dim, last line, for "what next" or counts.

## Commands

### status

Everything in sync:
```
✓ All 5 profiles in sync · 3 sources · nothing held for review

  claude        ~/.claude                        45 ok
  kilo-claude   ~/Library/…/profiles/claude      45 ok
  codex         ~/.codex                          3 ok · 1 unsupported
  kilo-codex    ~/Library/…/profiles/codex        3 ok · 1 unsupported
  agents        ~/.agents                        14 ok

→ colctl status --all lists every item
```

Something needs attention:
```
! 3 things need attention

  claude        ~/.claude                        44 ok · 1 missing
  kilo-claude   ~/Library/…/profiles/claude      45 ok
  …

Needs attention
  ! claude       skill    foo                    missing             → colctl sync
  ! ~/.colctl    file     skills/foo/SKILL.md    uncommitted         → colctl sync
  ! held         source   superpowers            8ca22db → 22f0b46   → colctl review superpowers
```

- The verdict counts things that need attention; informational states (`unsupported`, `unreviewed`) only appear in the
  per-profile counts.
- Profile lines: name, path (middle-truncated to fit), then counts per state in a fixed order: ok, then problem
  states, then informational ones.
- A profile that can't be used shows `✗ <name>  <path>  <reason>` with its fix.
- `--all` prints, after the summary, one section per profile listing every item as an item line, then the source
  section.
- `--harness <h>` limits both views to one profile.

### Running commands (`sync`, `add`, `rm`, `adopt`, `approve`, `init`, `refresh` when it syncs)

While running (terminal only, on stderr), one line per profile:
```
⠹ claude        installing superpowers                  2/4
✓ kilo-claude   3 changes
  codex         waiting
```

Then the result on stdout:
```
✓ Synced 5 profiles · 6 changes

  claude        installed superpowers · linked 2 skills
  kilo-claude   installed superpowers
  codex         added marketplace superpowers-dev · installed superpowers
  kilo-codex    · not packaged for Codex
  agents        · nothing to change
```

- Step words (past tense, grouped by verb, counted when more than one): `linked`, `removed`, `wrote`, `added
  marketplace`, `removed marketplace`, `installed`, `updated … to <short>`, `uninstalled`, `forgot`.
- Informational outcomes use `·` and never count as attention. The contradiction "needs attention … in sync" can't
  happen: a run with only informational notes is `✓`.
- A failed step: `✗ <profile>  <what> failed: <first line of the tool's error>`, then the verdict becomes
  `✗ Synced 4 of 5 profiles · 1 failed` with a hint (`colctl sync` retries; `-v` shows the call).
- `-v` lists, under each profile, every tool call with ✓/✗ and its duration in dim text.
- A command that does something first leads with that: `✓ Added source qdrant-skills · github.com/qdrant/skills at
  d2127f6`, then the profile block.
- `--dry-run`: verdict `· Would make 6 changes in 3 profiles`, then the same profile block in future tense
  (`would install superpowers`).

### update and update --check

```
! 1 update held for review

  superpowers   8ca22db → 22f0b46   4 new commits   held
  other         up to date

→ colctl review superpowers
```
`--check`: verdict `! 1 update available` (or `✓ Everything up to date`); states `available`, `held`, `rejected`,
`up to date`, `failed` (with the error).

### refresh

```
! 2 skills held for review

  skills/twg-jira       held       ~/.config/twg/skills/twg-jira
  skills/twg-artifacts  up to date
  skills/old            ✗ origin is gone   → adopt it again or rm skill old
```

### review

List: verdict `! 2 versions held for review`, then a table: key, change, size, flags, with `✗ <reason>` for
unreadable candidates; hint `colctl review <name>`.

One candidate (paged when taller than the terminal):
```
! superpowers  8ca22db → 22f0b46 · 12 commits · 4 files flagged
  https://github.com/obra/superpowers

Risk flags
  hooks/hooks.json            hook: adds hooks
  hooks/session-start         executable · script: changed executable file
  .claude-plugin/plugin.json  manifest: plugin manifest changed

Commits
  22f0b46  Release v7.0.2 …

Files
  A  hooks/hooks.json
  M  scripts/session-start.sh  (+x)

Diff
  <unified diff; + lines green, - lines red when colour is on>

→ colctl approve superpowers   or   colctl reject superpowers
```
Flags are grouped per file (one line per file, kinds joined with ` · `), files ordered by the most serious kind
(hook, mcp, lsp, executable, script, binary, symlink, allowed-tools, inline-shell, manifest). Risk flags come first.

### approve, reject

`✓ Approved superpowers · 8ca22db → 22f0b46` then the profile block. `✓ Rejected superpowers · 22f0b46 won't be
offered again`. Refusals: `! Refused skills/x · <reason>` with a hint; verdict counts them.

### search, developers, connectors

```
42crunch-api-security-testing@claude-plugins-official  42crunch-ai  Automate API security directly in Claude Code wi…
security-guidance@anthropic-plugin-directory           anthropics   Security reminders while editing  ✓ installed

2 of 2 results · colctl show <id> for details
```
No verdict line for searches (the list is the answer); the footer carries counts and the next step. Installed
plugins get a dim `✓ installed` (the profile names are in `show`). Connectors without a description show their
marketplace in the description column, and `openai` as developer when the id ends in `@openai-curated-remote`.
Long `--limit 0` lists are paged.

### show, marketplace list

`show`: the name as a heading, then a key–value block (keys dim, aligned). `marketplace list`: a table with a header
row (`NAME  TOOL  SOURCE  PLUGINS  PROFILES`), `managed` shown as `✓ managed`.

### init, adopt, rm

`init`: `✓ Created ~/.colctl and registered 5 profiles`, then the profiles found, then the sync block.
`adopt`: verdict counts adopted, refused; one item line each. `rm`: `✓ Removed source qdrant-skills` then the
profile block.

## Interaction

| Command | Confirmation (terminal only; `--yes` skips) | Picker when no name is given (terminal only) |
|---|---|---|
| `rm <kind> <name>` | `Remove <kind> <name> from <n> profiles? [y/N]` | — |
| `approve --all`, `reject --all` | `Approve <n> held versions? [y/N]` (the list is shown first) | — |
| `approve`, `reject` | — | multi-select of held versions |
| `review`, `show` | — | select one (`review` without a name in a terminal still lists first; the picker opens with `review -i`) |

Without a terminal, nothing asks: `rm` and `--all` proceed as in 2.1, and a missing name is the same usage error as
2.1. Answering No exits 1 with `· Nothing changed`. Ctrl-C at a prompt exits 130 with nothing changed.

## Help

```
Keep every AI coding harness in sync with one source of truth in ~/.colctl

Usage: colctl [OPTIONS] <COMMAND>

Get started:
  init          Create ~/.colctl and register the profiles on this machine
  status        What's in sync, what isn't, and what to run
  sync          Make every profile match ~/.colctl
Manage:
  add           Add a skill, agent, plugin or source plugin, then sync
  rm            Remove one, everywhere
  adopt         Move what a profile already has into ~/.colctl
Updates and review:
  update        Fetch new versions of source plugins and hold them for review
  refresh       Hold changes from the folders taken-over skills came from
  review        See what a held version changes, with risk flags
  approve       Install held versions exactly as reviewed
  reject        Drop held versions for good
Discover:
  search        Search every plugin catalog your profiles know
  show          One plugin in full
  developers    Who publishes what
  marketplace   List or refresh marketplaces
  connectors    Codex's app connectors
Other:
  completions   Shell completions (bash, zsh, fish)
  version       Print the version

Options: --color, -q, -v, -y, --no-pager, -h, -V
Learn more: https://github.com/muradmadi-leadtech/colctl/tree/main/docs
```
Each command's `--help` ends with an `Examples:` section of 2–4 real invocations.

`colctl completions <bash|zsh|fish>` prints the script for that shell.

## Text stability

Human-readable text may change in any release; scripts should use `--json` and exit codes, which follow semver.

## Revise: status profile lines and help

Profile line: `<mark>  <name>  <path>  <contents>`, where contents lists kinds in a fixed order with counts:
`<n> skills · <n> agents · <n> plugins · CLAUDE.md|AGENTS.md` (kinds with none are left out), then for a Codex profile
when an agents harness is registered `skills via ~/.agents`, then problem states (`1 missing`), then informational
(`1 not packaged`, `1 unreviewed`). Plugins count list plugins and source plugins together; marketplaces aren't
counted. An empty profile shows `nothing yet`.

Path column: shown in full when `indent + name + path + contents` fits the width; otherwise every path is cut with the
same (head, tail) parts kept, chosen as the most that lets all of them fit.

Welcome help (`colctl`, `colctl --help`, `colctl help`, `-h`): one description column for the whole page; the first
group's names carry `colctl ` so they can be copied; the others are task groups (Change what you have, Updates and
review, Find plugins, Other); then Options, then `More:` and `Docs:` lines. Descriptions wrap under their column.
`colctl help <command>` re-runs as `colctl <command> --help`, styled (bold headings, cyan literals) and wrapped by clap.
