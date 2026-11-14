# The TUI

`colctl` with no command opens a full-screen view of everything colctl manages, when you run it in a terminal. It
shows and runs only what the commands do. Every action in it is a command you could type, and it ends in the same
state.

## When it opens

| Situation | What `colctl` does |
|---|---|
| stdin and stdout are terminals, `TERM` isn't `dumb`, `CI` isn't set | Opens the TUI |
| Anything else (piped, redirected, `TERM=dumb`, `CI` set) | Prints the welcome help, as `colctl help` does |
| `colctl help`, `colctl --help` | Always the help |

Global flags: `--color never` (or `NO_COLOR`) opens it without colour and `--color always` forces colour. `-q`, `-v`,
`-y` and `--no-pager` don't apply to it. Nothing in the TUI skips a confirmation.

Exit codes: **0** when you quit (the TUI isn't a check; use `colctl status` for that), **101** after a bug, which is
reported on the normal screen with how to report it. The terminal is always restored, whether you quit with `q` or
Ctrl-C, after an error, or after a bug.

## Tabs

| Tab | Shows | Actions |
|---|---|---|
| **Overview** | `colctl status`'s verdict; a menu (sync every profile, check for updates, review what's held, find a plugin, look through every item); one line per profile with what it has; and **Needs you**: each problem with its fix | ⏎ runs the focused menu entry or fix (a review opens it; sync and update ask first; anything else says which command to run); `s` sync, `u` check for updates |
| **Profiles** | Every profile with its path and what it has | ⏎ lists a profile's items, like `status --harness <name> --all`; `s` syncs only that profile; `/` filters |
| **Items** | Every instructions file, skill, agent, plugin and source plugin once, with its state in each profile (numbered columns; the legend is above) | ⏎ shows each profile's state, where the item lives there, and its fix; `/` filters |
| **Review** | What's held for review, as `colctl review` lists it; the count is on the tab | ⏎ shows a held version at a glance: what changed, the recommendation, anything suspicious, what runs code; `d` shows everything `colctl review <name>` prints, diff included; `a` approve, `x` reject, `A` approve all, `u` check for updates |
| **Discover** | Every plugin every catalog knows, ranked as `colctl search` ranks them | type to search (every key but `?` goes to the search; Ctrl-C quits); ⏎ shows what `colctl show <id>` prints, and `a` there adds it as its add command says; Esc clears the search |

The tab bar marks the current tab. `Overview !` means something needs you, and `Review !2` means two versions are
held.

## Keys

| Key | Where | Does |
|---|---|---|
| → ←, Tab, ⇧Tab, 1–5 | everywhere (digits and letters search in Discover) | next, previous, go to a tab |
| ↑ ↓, k j | lists, details | move |
| PgUp PgDn, g G, Home End | lists, details | page, top, bottom |
| ⏎ | lists | open, or run the focused fix |
| Esc | everywhere | close the overlay, clear the filter, stop an update after the source it's on |
| / | Profiles, Items | filter; ⏎ or ↓ leaves the line, Esc clears it |
| type | Discover | search; Esc clears it; ←→ still switch tabs and ctrl-c quits |
| r | everywhere | reload from disk |
| s | Overview, Profiles | sync |
| u | Review | check for updates |
| a | Review, Discover | approve, add |
| x | Review | reject |
| A | Review | approve every held version |
| d | a held version | the full review, diff included |
| ] [ | a full review | next, previous file in the diff |
| y n | a confirmation | yes, no (⏎ alone does nothing) |
| ? | everywhere | every key for where you are |
| q, Ctrl-C | everywhere | quit; while a write runs, it asks first and waits for it to finish |

The key bar at the bottom shows the most useful keys for where you are. `?` shows all of them.

**Mouse:** click a tab to open it, click a row to focus it, double-click to open it, and click a key in the key bar to
run it. The wheel and trackpad scroll the view, at most 3 rows a frame however fast you flick; the selection moves
only to stay on screen. A click outside an overlay closes it, except for a confirmation.

## Writing

Anything that changes something (sync, update, approve, reject, add) asks first, saying what will happen and to
which profiles. While it runs, the status line shows each profile (or source) with its current step and `n/total`,
then ✓ or ✗. It ends with the command's verdicts, worst first, and every tab reloads. While anything loads, the
status line names it the same way, as the command it is (`⠋ colctl status`, `⠋ colctl review databricks`).

When a run doesn't succeed, a **Result** overlay opens with the CLI's full report: which profiles failed, why, and
what to run.

- Only one write runs at a time. Another write waits ("Wait for colctl sync to finish"), and reading keeps working.
- A sync, approve or add runs to the end. An update stops after the source it's on when you press Esc.
- Errors show what the CLI would print: the error, why, and what to do.
- Another colctl holding the lock is an error like any other. Nothing changes.

## Screen size

At least 60 columns by 20 rows. Smaller than that, the screen asks for a bigger window and keeps your place.

## Environment

| Variable | Effect |
|---|---|
| `NO_COLOR` | No colour. Symbols (✓ ! ✗ ·) and words still say everything, and the focused row is reversed |
| `COLCTL_REDUCED_MOTION` | `1` (anything but empty or `0`) turns animations off, and spinners become `…` |
| `COLCTL_HOME`, `COLCTL_TOOL_TIMEOUT_SECS` | As for every command ([Commands](commands.md#environment)) |
| `CI`, `TERM=dumb` | Never open the TUI |

## Motion

Short animations show what changed. A new tab sweeps in from the side you moved to (180 ms). An overlay forms in
place (150 ms). A profile's row brightens when it finishes (250 ms). A list re-forms after a reload (200 ms). A key or
click ends them at once. When nothing moves, nothing is redrawn. Without colour, only character effects run.
