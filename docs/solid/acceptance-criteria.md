# Acceptance criteria: solid

Tests are named after these ids (`ac9_01_…`). "The owner fixture" is cycle 6's: 5 profiles, ~100 items, fake `claude`
and `codex` on `PATH`. "Held" adds a held source update (superpowers), a held conversion (qdrant-skills) and a held
taken-over skill. TUI tests drive `tui::app::update` and render to `TestBackend` with motion off, as in cycle 8.

## Measure (T1)

**AC9-01 Where the time goes is written down**
```gherkin
Given a copy of the real profiles with databricks and qdrant-skills updates held
When colctl status -v and colctl review run
Then the cycle file records the time spent in git log, the file list, risk flags and the Codex package, per held
  version, and names the step the cache must remove
```

## Saved checks (T2)

**AC9-02 A held version is checked once**
```gherkin
Given the owner fixture and a counting inspector
When colctl update holds superpowers, then status, review, review superpowers --json and status --json run
Then superpowers is inspected exactly once, during update
And every later command reads the saved check
```

**AC9-03 Saved and fresh checks give the same output**
```gherkin
Given the owner fixture, held
When status, status --json, review and review --json run once with the cache and once with .state/cache removed
Then the output is byte for byte the same, and so are the exit codes
```

**AC9-04 Under a second with big updates held**
```gherkin
Given a source held 60 commits ahead with 300 changed files
When colctl status and colctl review run after update
Then each finishes in under 1 s in the release build (a timing test marked #[ignore], run in the smoke test)
And under 200 ms of that is spent on held versions in a debug test with the fake tools
```

**AC9-05 A changed hold is checked again**
```gherkin
Given a saved check for superpowers held at commit B
When update holds superpowers at commit C
Then the next status inspects C once and saves it, and the entry for B is gone
```

**AC9-06 Approve and reject remove the saved check**
```gherkin
Given saved checks for superpowers and the held skill
When approve superpowers and reject <skill> run
Then neither has a saved check afterwards, and status shows neither as held
```

**AC9-07 A changed pin or a changed held copy is never served from the cache**
```gherkin
Given a saved check for superpowers
When config.toml's rev for superpowers is edited by hand
Then status shows the pin warning it shows today, without reading the saved check
And given a saved check for a held skill whose snapshot is then edited
Then status shows "the held copy … changed since it was held", as today
```

**AC9-08 A new colctl or new risk rules recompute**
```gherkin
Given saved checks written by an older colctl version or older risk rules
When status runs
Then each held version is inspected again and the new check replaces the old
```

**AC9-09 A diff is never saved**
```gherkin
When review superpowers --diff runs (or ⏎ then d in the TUI)
Then the diff is computed from git each time
And no file under .state/cache contains diff text
```

**AC9-10 The cache is never committed**
```gherkin
When any write commits ~/.colctl
Then no path under .state/cache is in the commit
```

## TUI opens on the last state (T3)

**AC9-11 The first frame is the last known state**
```gherkin
Given a saved status from an earlier run
When colctl opens the TUI and the fresh snapshot takes 2 s
Then the first frame shows the saved profiles, items and verdict, with "as of <age> ·" before the spinner and
  "colctl status" on the status line
And when the fresh snapshot arrives it replaces the saved one and the age goes away
```

**AC9-12 No saved status falls back to today's behaviour**
```gherkin
Given the TUI saves each fresh status to .state/cache/status.json (colctl status itself writes nothing)
And no such file, or one from another colctl version, or one that doesn't parse
When the TUI opens
Then it shows the loading state it shows in 2.5
```

**AC9-13 Writes never act on the saved status**
```gherkin
Given the TUI showing a saved status that's out of date
When the user presses s (sync) before the refresh finishes
Then the sync runs against the state on disk, as the CLI does, and the overlay shows the real plan
```

## Core calls for the rest (T4)

**AC9-14 Each new job runs the same code as its command**
```gherkin
Given the owner fixture
When Job::Rm, Adopt, Convert, ConvertUndo, Refresh and MarketplaceRefresh each run through core::run
And the matching command (rm, adopt, convert, convert --undo, refresh, marketplace refresh) runs from the CLI on a copy
Then ~/.colctl, the profiles and the fake tools' call logs end the same
And the observer gets step and done for each profile changed, and nothing is printed
```

**AC9-15 Reads return what --json prints**
```gherkin
When core::adopt::candidates, core::marketplaces, core::connectors and core::developers run
Then each returns the data the command prints, and the command's --json is that data serialised, unchanged from 2.5
```

**AC9-16 adopt can say what it would take before it takes it**
```gherkin
Given a profile with two skills and a plugin that ~/.colctl doesn't have
When core::adopt::candidates runs for that profile
Then it lists the three, each with where it lives and whether it can be moved, and changes nothing
```

## Screens (T5)

**AC9-17 Remove from Items**
```gherkin
Given Items with a skill selected
When the user presses x
Then a confirm overlay shows what rm would remove and from which profiles
And y runs Job::Rm, shows progress, and ends in the result overlay; n or Esc changes nothing
```

**AC9-18 Convert and undo from Items**
```gherkin
Given Items with a Claude-only source selected
When the user presses c
Then the overlay shows what the Codex package would contain and asks to convert
And with a converted source selected, c offers to undo the conversion instead
And on a source with nothing Codex can use, c explains why, as the CLI does, and offers nothing to run
```

**AC9-19 Refresh from Items**
```gherkin
Given Items with a taken-over skill selected
When the user presses f
Then Job::Refresh runs for it and a change is held, and the Review tab's count goes up
And on an item that isn't taken over, f says so in the status line
```

**AC9-20 Adopt from Profiles**
```gherkin
Given Profiles with a profile selected that has items ~/.colctl doesn't
When the user presses a
Then a list of those items opens, each with a checkbox, all checked
And space toggles one, ⏎ asks to confirm with the plan, and y runs Job::Adopt for the checked ones
And with nothing to adopt, a says "Nothing in <profile> that colctl doesn't already have"
```

**AC9-21 Discover's other views**
```gherkin
Given the Discover tab
When the user presses Ctrl-V (letters search in Discover, so the key needs Ctrl)
Then the view cycles Plugins → Developers → Marketplaces → Connectors → Plugins, and the header says which
And type-to-search filters whichever view is showing
And in Marketplaces, ⏎ asks to refresh them and runs Job::MarketplaceRefresh
And in Developers, ⏎ on a developer shows their plugins in Plugins
```

**AC9-22 Every command is reachable**
```gherkin
For each command in colctl help except init, completions, version and help
Then a test finds the TUI key or palette entry that runs it, and the core call it makes
```

## Palette (T6)

**AC9-23 The palette runs any action**
```gherkin
Given any tab, with no overlay open
When the user presses : or Ctrl-K
Then an overlay lists every action from KEYMAP with its tab and key, the ones valid now first
And typing filters it with the same fuzzy matcher as Discover
And ⏎ runs the selected action as if its key had been pressed, switching tab first if needed
And Esc closes it with nothing run
```

**AC9-24 The palette and the Overview menu**
```gherkin
Then the Overview menu stays as it is in 2.5, and its last line reads ": all actions"
```

## Follow-ups (T7)

**AC9-25 allowed-tools flags are grouped**
```gherkin
Given a held plugin whose 12 skills each declare allowed-tools
When review runs
Then one flag line says "12 skills declare allowed-tools" with their names in -v and in --json's per-file list
And the recommendation is unchanged
```

**AC9-26 -v prints paths from home**
```gherkin
When any command runs with -v
Then every path under the home folder in tool-call lines is printed as ~/…
```

**AC9-27 Fake tools never fail with ETXTBSY**
```gherkin
When the test suite's fake tools are installed
Then each is written to a temporary name and renamed into place, and the suite passes 50 runs in a row in CI's
  stress job
```

~~AC9-28 connectors shows no empty fields~~ — _dropped in Generate._ A connector row already leaves out an empty
description; leaving fields out of `--json` would change its shape, which S7 rules out.

**AC9-29 sync's wording**
```gherkin
When sync finishes with a profile it couldn't fix
Then the verdict names what's wrong and the command that fixes it, instead of "needs attention"
```

~~AC9-30 status names adopt for a plugin colctl didn't install~~ — _dropped in Generate._ The case behind it (qdrant
installed as `qdrant@qdrant`) was fixed by 2.5.0's re-pointing and reinstall (#114); the real profiles show it in
sync, and no state is left where `status` suggests the wrong fix.

**AC9-31 marketplace with no subcommand**
```gherkin
When colctl marketplace runs with no subcommand
Then it prints its help and exits 2, instead of "✗ error:" with no message
```

## Docs and release (T8)

**AC9-32 The docs cover what changed**
```gherkin
Then docs/reference/tui.md lists every new key and the palette, docs/reference/review.md explains the saved checks,
  and docs/explanation has a page on why checks are saved and when they're redone
```
