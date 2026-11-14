# Design spec: TUI

_Cycle 8 · Refine · 2026-11-14_

Written and agreed before any TUI code (SC-9). Screens follow it, and the snapshot tests hold them to it.

## Principles

1. **The answer first.** Each tab opens on its verdict line, the same sentence the CLI would print.
2. **Act where you look.** A problem on screen shows its fix, and one key runs it.
3. **Calm by default.** One accent colour, no borders around everything, empty space over decoration. Colour marks
   state and is never ornament.
4. **Motion explains change.** Something moves only to show where it came from or what just changed. Nothing is
   longer than 250 ms, and nothing ever blocks a key.
5. **No surprises.** Anything that writes asks first, and says what will happen and to which profiles. Esc always
   backs out.
6. **The CLI's words.** The same verdicts, verbs, state symbols and hints as the CLI, so both feel like one tool.

## Tokens

Colours use the terminal's own 16-colour ANSI palette, so they follow the user's light or dark theme. There are no
fixed RGB values and no background fills except the selection.

| Token | Use | Colour | Without colour (`NO_COLOR`) |
|---|---|---|---|
| `ok` | ✓, in sync, done | green | symbol + word |
| `attention` | !, held, outdated, needs you | yellow | symbol + word, **bold** |
| `failed` | ✗, errors | red | symbol + word, **bold** |
| `info` | ·, notes, secondary facts | dim | symbol + word |
| `accent` | active tab, focused input, runnable commands, key letters in the key bar | cyan | underline |
| `name` | plugin, skill, profile names | bold | bold |
| `muted` | paths, counts, help text, separators | dim | as is |
| `added` / `removed` | diff lines | green / red | `+` / `-` prefix (always present) |
| `selection` | the focused row | reversed | reversed |

The tokens extend `ui::style::Token`. `accent` is the CLI's `Command`. `tui::theme` maps each token to a ratatui
`Style` and is the only place colours are named.

**Symbols:** `✓ ! ✗ ·` for states (as in the CLI). `›` marks the focused row when colour is off. `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` is
the spinner, at 80 ms a frame. `─` draws rules, and overlays get rounded corners (`╭╮╰╯`).

## Layout

```
row 0     tab bar              Overview  Profiles  Items  Review !2  Discover        colctl 2.4.0
row 1     rule                 ───────────────────────────────────────────────────────────────────
rows 2…   body                 (one tab, full width, 1-column side gutter)
row h-2   status line          job progress, or the last result for 4 s, or empty
row h-1   key bar              s sync · ⏎ open · / filter · ? keys · q quit
```

- **Minimum 60×20.** Smaller than that, the whole screen says `Make the window at least 60×20 (now 52×18)` and
  nothing else.
- **Widths.** Tables cut with `…` using the CLI's `cut` and `cut_alike` rules, and columns always line up. At 100
  columns or more, tables show their optional columns: path, version, developer.
- **Overlays** (detail, confirm, help, error) are centred, 80% of the width up to 100 columns, and at most the body's
  height. A dim rule shows behind them. Each overlay's title sits in its top border and its keys in its bottom border.
- **Tab bar.** The active tab is in `accent` and underlined. Counts appear only when they matter: `Review !2` when items
  are held, `Overview !` when something needs you. The version sits on the right in `muted`.

## Components

| Component | Look and behaviour |
|---|---|
| **Verdict** | The CLI's verdict line: a symbol, a sentence and facts in `muted`. `✓ All 5 profiles in sync · 41 skills · 12 plugins` |
| **List / table** | One row per thing. The focused row is reversed. A scroll position (`12/41`) appears in the bottom-right of the body when the list doesn't fit |
| **Section heading** | Bold text and a blank line above it. No boxes |
| **Fix row** | A problem, then `→ colctl …` in `accent`. When it's focused, ⏎ runs it (after a confirm when it writes) |
| **Key bar** | Up to 6 keys for the current context, most useful first, as `key action`. The key is in `accent`. `?` and `q` always come last. It shrinks from the left at narrow widths |
| **Help overlay (`?`)** | Every key for the current tab, then the global keys, generated from the same keymap table |
| **Confirm** | Title = the action (`Approve superpowers`). Body: what will happen, which profiles, how big. Keys: `y` yes, `n`/Esc no. No is the default, and ⏎ alone does nothing |
| **Progress panel** | Replaces the status line during a write: one row per profile with a spinner, the current step and `n/total`. On finish, each row ends in ✓ or ✗ and the panel collapses to the verdict |
| **Toast** | A result in the status line, kept for 4 s: `✓ Synced 5 profiles`. Errors stay until a key is pressed |
| **Error** | An overlay in the CLI's layout: `error: …`, an indented cause, then `hint: …` |
| **Empty state** | One centred line and a key: `Nothing held for review. u checks for updates` |
| **Filter / search input** | `/` opens a one-line input above the list with a `›` prompt in `accent`. Results update on every key, and Esc clears and closes it |
| **Diff view** | Unified diff, `+` and `-` lines in `added` and `removed`, file headers in `name`. Scrolls by line and page. `]` and `[` jump between files |

## Keymap

One table (`tui::keys`) drives handling, the key bar and the help overlay.

| Key | Where | Does |
|---|---|---|
| Tab / Shift-Tab, 1–5 | everywhere | next / previous tab, jump to tab |
| ↑ ↓ / k j | lists | move |
| PgUp PgDn, g G | lists, diff | page, top, bottom |
| ⏎ | lists | open detail, or run the focused fix |
| Esc | everywhere | close the overlay, clear the filter, cancel a job that can be cancelled, else nothing |
| / | Profiles, Items, Discover | filter (Discover: focus search) |
| r | everywhere | reload from disk |
| s | Overview, Profiles | sync (all, or the focused profile in Profiles) |
| u | Review | check for updates |
| a | Review / Discover | approve the focused item / add the focused plugin |
| x | Review | reject the focused item |
| A | Review | approve all held |
| ] [ | diff | next / previous file |
| ? | everywhere | keys |
| q, Ctrl-C | everywhere | quit (Ctrl-C also during a job: asks first if the job writes) |

**Mouse:** a click on a tab switches to it. A click on a row focuses it, and a double click opens it. The wheel
scrolls lists, diffs and overlays. A click on a key in the key bar runs it. Clicks outside an overlay close it, except
on a Confirm.

## Motion

| Moment | Effect (tachyonfx) | Duration |
|---|---|---|
| Tab switch | The body slides 4 columns in the direction of travel and fades in | 180 ms, ease-out |
| Overlay opens / closes | Fade and a slight scale from the centre / fade out | 150 ms / 100 ms |
| A profile or item finishes | Its row's symbol pops (brightens, then settles) | 250 ms |
| List rows change after a reload | New rows fade in; changed states flash once | 200 ms |
| Progress | Spinner | 80 ms a frame |

Rules: effects never delay input, and a key during an effect finishes it at once. The app redraws only while
something is moving or running, so it is idle at 0% CPU. With `COLCTL_REDUCED_MOTION=1`, effects are off and the
spinner becomes a static `…` beside `n/total`. Tests run with motion off.

## Screens (80×24)

**Overview**
```
 Overview !  Profiles  Items  Review !2  Discover                   colctl 2.4.0
────────────────────────────────────────────────────────────────────────────────
 ! 2 updates held for review · 5 profiles in sync

 Profiles
   ✓ claude        ~/.claude                      41 skills · 12 plugins · 1 agent
   ✓ kilo-claude   ~/L…/profiles/claude           41 skills · 12 plugins · 1 agent
   ✓ codex         ~/.codex                       1 agent · 3 plugins
   ✓ kilo-codex    ~/L…/profiles/codex            1 agent · 3 plugins
   ✓ agents        ~/.agents                      41 skills

 Needs you
 › ! superpowers   v4.1.0 → v4.2.0 held           → colctl review superpowers
   ! qdrant-skills d2127f6 conversion held        → colctl review qdrant-skills




 s sync · ⏎ run fix · r reload · ? keys · q quit
```

**Review** (detail open)
```
 Overview !  Profiles  Items  Review !2  Discover                   colctl 2.4.0
────────────────────────────────────────────────────────────────────────────────
 ! 2 held fo╭ superpowers · v4.1.0 → v4.2.0 ─────────────────────────────╮
            │ 3 commits · 7 files · 1 flag                               │
 › superpowe│                                                            │
   qdrant-sk│ ! New hook: PreToolUse on Bash runs hooks/guard.sh         │
            │                                                            │
            │ hooks/hooks.json                                           │
            │ +    "PreToolUse": [                                       │
            │ +      { "matcher": "Bash", "hooks": [ …                   │
            │ skills/brainstorming/SKILL.md                              │
            │ -  Ask one question at a time                              │
            │ +  Ask one question at a time, with options                │
            │                                                     12/140 │
            ╰ a approve · x reject · ] next file · esc close ────────────╯

 a approve · x reject · u check updates · ⏎ open · ? keys · q quit
```

**Discover**
```
 Overview  Profiles  Items  Review  Discover                        colctl 2.4.0
────────────────────────────────────────────────────────────────────────────────
 › sprpw▏
 3 of 214 plugins · 6 catalogs

 › superpowers            obra              Skills for planning, TDD and review ✓
   superpowers-lab        obra              Experimental skills
   spr-pw-tools           someone           Sprint and power tools



 ⏎ details · a add · esc clear · ? keys · q quit
```

Profiles, Items and the confirm, progress, help and too-small screens are drawn the same way. Their exact snapshots
are the T6–T9 tests.
