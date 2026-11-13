# Edge cases: CLI output design

_Cycle 6 · Refine · 2026-11-13_

IDs are `EC6-NN`; each needs a test whose name contains `ec6_NN`.

## Terminal and environment

| ID | Case | Expected |
|---|---|---|
| EC6-01 | Width below 40 or above 160, or `COLUMNS` not a number | Clamped to 40–160; a bad `COLUMNS` is ignored |
| EC6-02 | `NO_COLOR` set but empty | Doesn't count (per no-color.org): colour stays `auto` |
| EC6-03 | `CLICOLOR_FORCE=0` | Doesn't force colour |
| EC6-04 | stdout piped, stderr a terminal | Results plain on stdout; progress may still draw on stderr; no prompts (stdout isn't a terminal) |
| EC6-05 | stdin not a terminal (e.g. `< /dev/null`), stdout a terminal | No prompts or pickers; behaves as 2.1 |
| EC6-06 | `CI` set | No prompts, pickers or progress even in a terminal |

## Layout

| ID | Case | Expected |
|---|---|---|
| EC6-07 | Names longer than the width allows | Cut with "…" in the flexible column; a profile path is middle-truncated (`~/Library/…/profiles/claude`) |
| EC6-08 | Wide characters (CJK, emoji) in names or descriptions | Measured as 2 columns; columns stay aligned |
| EC6-09 | Text from catalogs or repos with control characters | Escaped as in cycles 4–5, before measuring |
| EC6-10 | A profile with no items | `· 0 items`; not an error |
| EC6-11 | No registered profiles | `! No profiles registered` with hint `colctl init` |

## Interaction

| ID | Case | Expected |
|---|---|---|
| EC6-12 | `--yes` without a terminal | Accepted, nothing asked |
| EC6-13 | A picker with nothing to pick (nothing held) | No picker: `· Nothing held for review`, exit 0 |
| EC6-14 | Ctrl-C (or EOF) at a prompt or picker | Nothing changed; exit 130 for Ctrl-C, 1 for EOF; the terminal is left clean |
| EC6-15 | `$PAGER` set to something that fails, or `less` missing | Printed directly |
| EC6-16 | `--json` in a terminal | No colour, prompts, pickers, pager or progress; JSON only (AC6-01) |

## Flags and errors

| ID | Case | Expected |
|---|---|---|
| EC6-17 | `-q -v` together | Usage error, exit 2, with the error layout |
| EC6-18 | Unknown command, or a typo of one | Error layout; the parser's suggestion becomes `hint: did you mean …`; exit 2 |
| EC6-19 | An error while progress is drawn | Progress is cleared before the error prints; no half-drawn lines |
| EC6-20 | A broken pipe while printing (`… \| head`) | Still exits quietly with 141 (EC5-24) |

## Sources (T0)

| ID | Case | Expected |
|---|---|---|
| EC6-21 | A repo whose Claude manifest points inside the repo but whose Codex manifest points to GitHub (or the reverse) | Decided per tool: the wrapper only where the repo's own manifest points elsewhere |
| EC6-22 | The wrapper's id is already installed by hand from another marketplace | Treated like any foreign install of that id: left alone, reported |

## Revise

| ID | Case | Expected |
|---|---|---|
| EC6-23 | `colctl` with no command, before `colctl init` (no ~/.colctl) | The welcome help, exit 0; nothing is created |
