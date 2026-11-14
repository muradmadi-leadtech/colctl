# Acceptance criteria: TUI

Tests are named after these ids (`ac8_01_…`). "The owner fixture" is the cycle 6 fixture: 5 profiles (claude,
kilo-claude, codex, kilo-codex, agents), ~100 items, a fake `claude` and `codex` on `PATH`. "Held" adds one held update
(superpowers) and one held conversion (qdrant-skills). TUI tests drive `tui::app::update` with key and mouse messages,
render to ratatui's `TestBackend` with motion off, and compare with insta snapshots at 80×24 and 120×40.

## Core API: the CLI first (T1–T4)

**AC8-01 Status from a snapshot**
```gherkin
Given the owner fixture, with and without held items
When core::status::snapshot runs
Then it returns profiles, items with their states and hints, source problems, held items, the verdict and the exit code
And colctl status, status --all and status --json print byte for byte what 2.3 printed, with the same exit codes
```

**AC8-02 Runs report through an observer and never print**
```gherkin
Given a recording Observer and stdout captured
When core sync, update, approve and reject run on the owner fixture with held items
Then the observer gets step(profile, action, n, total) and done(profile, summary) for each profile that changes,
  in order, and notes and warnings that 2.3 printed directly
And nothing is written to stdout or stderr
And each returns a typed report with the per-profile results and the snapshot after the run
```

**AC8-03 The CLI renders the reports unchanged**
```gherkin
When sync, update, review, approve, reject, show and add run from the CLI after T2 and T3
Then their text, --json, progress (in a pty) and exit codes match 2.3's snapshots, and no snapshot is updated
```

**AC8-04 Review and discovery data**
```gherkin
When core::quarantine::held, review(name), core::discover::catalog, show(id) and add(id) run
Then they return the data review --json, search --json and show --json are built from, without printing
And add returns the same report colctl add prints
```

**AC8-05 Fuzzy search, shared**
```gherkin
Given a catalog with superpowers, superpowers-lab, spr-pw-tools and qdrant
When I run colctl search sprpw
Then superpowers and superpowers-lab are found (letters in order, not necessarily together), and qdrant isn't
And for a query 2.3 already matched (e.g. "superpowers"), every 2.3 result is still found, with exact name matches first
And with several words each word must match, as in 2.3; --tool, --marketplace, --developer, --installed and --limit behave as before
And the TUI's Discover ranks with the same function and gets the same order
```

## Launch (T5)

**AC8-06 Bare colctl opens the TUI only in a terminal**
```gherkin
Given stdin and stdout are terminals, TERM is not "dumb" and CI is unset
When I run colctl with no arguments
Then the TUI opens on the Overview tab
And in every other case (stdout piped, stdin piped, TERM=dumb, CI set) it prints the 2.3 welcome help and exits 0
And colctl help and colctl --help always print help
```

**AC8-07 The terminal is always restored**
```gherkin
When the TUI quits by q, by Ctrl-C, by an error, or by a panic in a view
Then raw mode, the alternate screen and mouse capture are off and the cursor is shown
And q and Ctrl-C exit 0; a panic prints "colctl hit a bug" with the panic message on the normal screen and exits 101
```

**AC8-08 First frame from local state**
```gherkin
Given a git and claude that fail if called, and no network
When the TUI opens
Then the first frame shows the Overview verdict and profiles, built from ~/.colctl and the profiles on disk only
And the Discover catalog loads in the background; until it does, Discover shows "Loading catalogs…"
```

## The shell (T5)

**AC8-09 Tabs, keys and help**
```gherkin
When I press Tab, Shift-Tab, or 1–5, or click a tab
Then the matching tab shows, with the tab bar marking it
And the key bar shows the keys for the current tab and focus, generated from the keymap
And ? shows every key for the current tab and the global keys; Esc closes it
```

**AC8-10 Mouse**
```gherkin
When I click a row, double-click a row, scroll the wheel over a list, or click a key in the key bar
Then the row is focused, opened, the list scrolls, or the key's action runs, exactly as the keys would
```

**AC8-11 Too small**
```gherkin
Given a terminal of 59×24 or 80×19
Then the screen says "Make the window at least 60×20 (now 59×24)" and nothing else
And resizing to 60×20 or more redraws the current tab with its state kept
```

**AC8-12 Colour and motion switches**
```gherkin
Given NO_COLOR is set, or --color never
Then no frame contains a colour, states keep their symbols and words, and the selection is reversed
Given COLCTL_REDUCED_MOTION=1
Then no effect runs and spinners are a static "…" beside n/total
```

## Overview and Profiles (T6)

**AC8-13 The answer first**
```gherkin
Given the owner fixture with held items
Then Overview's first body line is the verdict colctl status gives, followed by one line per profile with what it has
  (as status's profile lines) and a "Needs you" list with each fix as in status
And with nothing to do, Overview shows the verdict and the profiles, and no "Needs you" section
```

**AC8-14 Run a fix in place**
```gherkin
When I focus a Needs you row and press ⏎
Then a fix that only reads (review) opens the matching view (the Review tab with that item open)
And a fix that writes (sync, update) asks to confirm, then runs as if from the key that does it
```

**AC8-15 Sync with live progress**
```gherkin
Given profiles that need changes
When I press s on Overview and confirm
Then the progress panel shows a row per changing profile with its current step and n/total while the fake tools run,
  and the UI keeps redrawing and answering keys
And it ends with each row ✓ or ✗, the verdict as a toast, and Overview reloaded
And ~/.colctl, its state and the profiles are as colctl sync leaves them on the same fixture (AC8-30)
And s on a focused profile in Profiles syncs that profile only (colctl sync --harness <name>)
```

**AC8-16 Profiles**
```gherkin
Then Profiles lists each profile with kind, path and counts by kind; ⏎ opens one, listing its items with states and
  hints like colctl status --harness <name> --all
And / filters the list by name as I type
```

## Items (T7)

**AC8-17 Everything by kind**
```gherkin
Then Items lists instructions, skills, agents, plugins and source plugins, grouped by kind, each with its state in every
  profile that has it (one symbol per profile, profile initials in the header)
And items that need attention sort first within their kind
And / filters by name; ⏎ opens an item with where it comes from, each profile's state and path, and its fix
```

## Review (T8)

**AC8-18 The queue**
```gherkin
Given held items
Then the Review tab shows "Review !2" and lists each held item with its change (v4.1.0 → v4.2.0, or the conversion),
  size and flag count, as colctl review lists them
And with nothing held it shows "Nothing held for review. u checks for updates"
```

**AC8-19 A held item in full**
```gherkin
When I press ⏎ on a held item
Then the overlay shows what colctl review <name> shows: commits, files, risk flags first, then the diff
And the diff scrolls by line and page, ] and [ jump between files, and flags are listed above the diff
```

**AC8-20 Approve and reject**
```gherkin
When I press a (or x) on a held item, from the list or the overlay, and confirm
Then it runs approve (or reject) with live progress, and ends in the state colctl approve (reject) <name> leaves (AC8-30)
And A approves every held item after one confirm that lists them
And n or Esc at the confirm changes nothing
```

**AC8-21 Check for updates**
```gherkin
When I press u and confirm
Then update runs with progress per source; new versions appear in the queue as they're held; it ends with update's verdict
And Esc while it fetches stops before the next source and keeps what was already held
```

## Discover (T9)

**AC8-22 Search as you type**
```gherkin
When the catalog has loaded and I type in the search line
Then the results update on every key, ranked by core::discover::rank (AC8-05), showing name, developer, description and
  ✓ when installed somewhere, and "<shown> of <total> plugins · <n> catalogs"
And an empty query lists every plugin in the order colctl search with no terms uses
```

**AC8-23 Show and add**
```gherkin
When I press ⏎ on a result
Then an overlay shows what colctl show <id> shows
When I press a on a result and confirm
Then it adds the plugin as its add command would (plugin or source), with progress, then syncs, and the result shows ✓
And for a plugin colctl can't add, a shows the reason (show's "add" field) as a toast and does nothing
```

## Motion (T10)

**AC8-24 Effects as specified**
```gherkin
With motion on, switching tabs, opening and closing overlays, finishing a profile and reloading lists start the effects
  in the design spec with their durations
And a key pressed during an effect is handled at once and finishes the effect
And when nothing moves or runs, the app makes no draw calls (idle)
```

## Safety and parity (T5–T11)

**AC8-25 Writes ask first**
```gherkin
Then every action that writes (sync, update, approve, reject, add) shows Confirm, naming the action, the items and the
  profiles it touches; nothing writes before y
```

**AC8-26 One job at a time**
```gherkin
Given a write is running
When I press a key that starts another write
Then a toast says "Wait for <job> to finish" and nothing starts; reading keys (tabs, scroll, open, ?) still work
```

**AC8-27 Errors**
```gherkin
When a core call fails (a tool fails, the lock is held, a file can't be read)
Then an error overlay shows the error, cause and hint the CLI would print, and the screen reloads after Esc
```

**AC8-28 Quit during a write**
```gherkin
When I press q or Ctrl-C while a write runs
Then it asks "A sync is running. Quit when it finishes?"; y waits for the job, then quits; n carries on
And the job is never killed half way
```

**AC8-29 Reload**
```gherkin
When ~/.colctl or a profile changes outside the TUI and I press r
Then every tab shows the new state, keeping the focused tab and, where it still exists, the focused row
And every finished write reloads by itself
```

**AC8-30 Parity**
```gherkin
For each of sync, sync --harness, update, approve, approve all, reject and add
When the action runs in the TUI on one copy of a fixture, and the CLI command on another
Then both copies' ~/.colctl (files, config, state, git log messages) and profiles are identical
```

**AC8-31 Docs**
```gherkin
Then docs/tutorials has "A first look around colctl's TUI", docs/reference has a TUI page (launch rule, every key, env
  vars, mouse), the search reference describes fuzzy matching, and README mentions the TUI
And every AC8 and EC8 id has a test whose name contains it
```

## Constraints (not criteria)

- Built on ratatui, crossterm, tachyonfx and nucleo-matcher. No async runtime, and no new process model.
- The TUI holds `~/.colctl`'s lock only while a write runs, the same lock as the CLI.
- No TUI-only logic: anything the TUI shows or does goes through `src/core`. Review checks this with the import
  rule: `src/tui` may import `core`, `ui::style` and `ui::render` and nothing from `commands`, `plan` or `apply`.
- macOS and Linux terminals (Terminal.app, iTerm2, Ghostty, kitty, WezTerm, Alacritty, tmux). Windows is out.
- Coverage ≥ 90% lines, as in `just check`.
