# Output

How colctl presents results, runs, errors and questions. Everything here is for people reading a terminal. Scripts
should use `--json` and [exit codes](exit-codes.md): **human-readable text may change in any release**, while JSON and
exit codes follow semver.

## Global flags

| Flag | Effect |
|---|---|
| `--color auto\|always\|never` | Colour policy (default `auto`) |
| `-q`, `--quiet` | Only errors and the verdict line |
| `-v`, `--verbose` | Also each tool call (`claude …`, `codex …`) with how long it took |
| `-y`, `--yes` | Answer yes to every confirmation |
| `--no-pager` | Never page long output |

They work before or after the command. `-q` with `-v` is a usage error.

## Help

`colctl` on its own, `colctl help`, `colctl -h` and `colctl --help` show the same page: what colctl does, **Start here**
(`init`, `status`, `sync`), every command grouped by task, the global options and where to learn more. Nothing is read
or created, so it works before `colctl init`. `colctl help <command>` is `colctl <command> --help`: its options,
wrapped to the terminal, and examples. In a terminal headings are bold and commands cyan.

## What the terminal decides

| | Rule |
|---|---|
| Colour | On for a terminal; off when piped, with `NO_COLOR` (non-empty), with `TERM=dumb`, or `--color never`. `CLICOLOR_FORCE=1` (or `--color always`) turns it on when piped. Decided separately for stdout and stderr |
| Width | The terminal's columns, else `COLUMNS`, else 80; never below 40 or above 160 |
| Live progress | Only when stderr is a terminal, and not with `-q`, `--json` or `CI` set |
| Questions and pickers | Only when stdin **and** stdout are terminals, and not with `--json` or `CI` set |
| Pager | Only when stdout is a terminal, the text is taller than it, and not `--no-pager`. `$PAGER`, else `less -R -F -X`; if it can't start, the text is printed |

## Symbols

| Symbol | Colour | Means |
|---|---|---|
| `✓` | green | done, in sync |
| `!` | yellow | needs you: drift, held versions, refusals |
| `✗` | red | failed |
| `·` | dim | informational: not packaged for a tool, nothing to change |

The symbol and a word are always there, so nothing depends on colour.

## How results are laid out

- **The first line is the answer**: `✓ All 5 profiles in sync · 3 sources · nothing held for review`,
  `! 2 things need attention`, `✗ Synced 4 of 5 profiles · 1 failed`.
- **Then what matters**: one line per profile, and under **Needs attention** only the things that need you, each with
  the command that fixes it (`→ colctl sync`). `status --all` lists everything.
- **Outcomes, not calls**: runs say `installed superpowers · linked skills a, b`; `-v` shows the calls.
- **Nothing important is cut.** Tables cut descriptions first and ids last; fixes, risk flags and outcomes that don't
  fit move to the next line instead of being cut. Paths are cut in the middle (`~/Library/…/profiles/claude`).
- **A footer** (dim) ends lists: counts and the next step.

## Errors

```
✗ error: no source named "nope"
  hint: colctl review --help
```

Every error says what went wrong, why when there's more to say, and a hint. The exit code is as in
[exit codes](exit-codes.md). Argument errors look the same; a typo gets `hint: did you mean status`.

## Questions

| Command | Asks (in a terminal) |
|---|---|
| `rm` | `Remove <kind> <name> from <n> profiles? [y/N]` |
| `approve --all`, `reject --all` | lists what's held, then `Approve <n> held versions? [y/N]` |
| `approve`, `reject` with no names | a multi-select of held versions |
| `show` with no id, `review -i` | a fuzzy picker |

`--yes` answers yes. Answering no (or end of input) prints `· Nothing changed` and exits 1; Ctrl-C exits 130. Without a
terminal nothing is asked: `rm` and `--all` go ahead and a missing name is a usage error, as before 2.2.

## Shell completions

```sh
colctl completions fish > ~/.config/fish/completions/colctl.fish
colctl completions zsh  > ~/.zfunc/_colctl
colctl completions bash > ~/.local/share/bash-completion/completions/colctl
```
