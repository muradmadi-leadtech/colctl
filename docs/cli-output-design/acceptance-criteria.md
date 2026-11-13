# Acceptance criteria: CLI output design

_Cycle 6 · Refine · 2026-11-13_

IDs are `AC6-NN`; each needs a test whose name contains `ac6_NN`. Layouts are in [interfaces.md](interfaces.md),
edge cases in [edge-cases.md](edge-cases.md). Unless a criterion says otherwise, tests render with colour off at 80
columns, as a pipe would.

## Foundation (T1)

**AC6-01 JSON and exit codes don't change**
```gherkin
Given fixture machines for status, review, search, show, developers and marketplace list
When each runs with --json
Then the output is byte-identical (temp paths normalised) to golden files recorded from colctl 2.1
And every exit code in the earlier suites is unchanged
```

**AC6-02 One output layer**
```gherkin
Given the source tree
Then no file outside src/ui (and main.rs) calls println!, print!, eprintln! or eprint!
```

**AC6-03 Colour policy**
```gherkin
Then colour is on for a terminal by default, off when piped, off with NO_COLOR set (non-empty), off with
  TERM=dumb, off with --color never, on with --color always, and on when piped with CLICOLOR_FORCE=1
And stdout and stderr are decided separately
```

**AC6-04 Symbols carry meaning without colour**
```gherkin
Given colour off
Then every state still shows its symbol and word (✓ ok, ! attention, ✗ failed, · info)
```

**AC6-05 Width**
```gherkin
Then the width is the terminal's, else COLUMNS, else 80, clamped to 40–160
And tables never wrap a row: the last flexible column is cut with "…", measured in display columns
```

**AC6-06 Errors**
```gherkin
When any colctl error or argument error happens
Then stderr shows "✗ error: <what>", an indented cause when there is one, and "hint: <what to do>"
And the exit code is the same as in 2.1
```

**AC6-07 Global flags**
```gherkin
Then --color, -q, -v, -y and --no-pager are accepted before or after the subcommand
And -q with -v is a usage error
```

## status (T2)

**AC6-08 All in sync**
```gherkin
Given a machine with 5 profiles and ~100 items, all in sync
When I run colctl status
Then it prints a ✓ verdict with profiles, sources and held count, one line per profile with counts, and the --all hint:
  at most 8 non-blank lines
And the exit code is 0
```

**AC6-09 Problems only, each with its fix**
```gherkin
Given missing, broken, foreign and edited items, uncommitted files, a held version and an origin change
When I run colctl status
Then the verdict is "! <n> things need attention" and a "Needs attention" section lists exactly those, each with its fix
And informational states only appear in the per-profile counts
```

**AC6-10 --all**
```gherkin
When I run colctl status --all
Then every item of every profile is listed under its profile heading, with state, kind, name and fix
```

**AC6-11 Unusable profiles**
```gherkin
Given a registered profile whose folder is missing
Then its line is "✗ <name>  <path>  <reason>" with its fix, and it counts in the verdict
```

## Running commands (T3)

**AC6-12 Outcomes, not calls**
```gherkin
When sync makes changes in several profiles
Then the verdict is "✓ Synced <n> profiles · <m> changes" and each profile line lists outcomes in past tense, grouped
  by verb and counted
And no tool command line appears without -v
```

**AC6-13 Informational outcomes don't contradict**
```gherkin
Given a source that isn't packaged for Codex
When I run colctl add source on it
Then the Codex profiles show "· not packaged for Codex", the verdict is ✓, and the words "needs attention" don't appear
```

**AC6-14 Failures**
```gherkin
Given a tool call that fails in one profile
Then that profile shows "✗ … failed: <first line of the error>", the verdict is "✗ Synced <n-1> of <n> profiles · 1 failed",
  a hint suggests colctl sync and -v, and the exit code is as in 2.1
```

**AC6-15 Live progress in a terminal only**
```gherkin
Given stderr is a terminal
When sync runs tool calls
Then each profile has a live line with a spinner and the current step, replaced by its result when done
And when stderr isn't a terminal, or with -q, no progress is drawn and the output contains no control characters
```

**AC6-16 -v and -q**
```gherkin
Then -v adds every tool call with ✓/✗ and its duration under its profile
And -q prints only the verdict line (and errors)
```

**AC6-17 Commands lead with their own result**
```gherkin
Then add, rm, adopt, approve, reject and init start with a verdict naming what they did (e.g. "✓ Added source qdrant-skills ·
  github.com/qdrant/skills at d2127f6") before the profile block
And --dry-run shows "· Would …" and the profile block in future tense
```

## Interaction (T4)

**AC6-18 Confirmations**
```gherkin
Given a terminal
When I run colctl rm source superpowers, or approve --all, or reject --all
Then colctl asks "[y/N]" (showing what's affected); No exits 1 with "· Nothing changed"; --yes skips the question
And without a terminal nothing is asked and the command proceeds as in 2.1
```

**AC6-19 Pickers**
```gherkin
Given a terminal and held versions
When I run colctl approve or colctl reject with no names
Then a multi-select of held versions opens, and the chosen ones are approved or rejected
And colctl show with no id, or review -i, opens a single-select
And without a terminal the missing name is the same usage error as in 2.1
```

**AC6-20 Pager**
```gherkin
Given a terminal and output taller than it
When I run colctl review <name> or a long colctl search
Then the text goes through $PAGER (else less -R -F -X), unless --no-pager
And when the pager can't start, the text is printed directly
```

## The rest (T5)

**AC6-21 update and refresh**
```gherkin
Then update, update --check and refresh print a verdict, one row per item with its state, and the review hint
```

**AC6-22 review**
```gherkin
Then review's list has a verdict and a table; review <name> shows Risk flags first, grouped per file and ordered by
  severity, then Commits, Files and Diff, then the approve/reject hint
And with colour on, diff + lines are green and - lines red
```

**AC6-23 Discovery output**
```gherkin
Then search, developers and connectors print aligned tables and a dim footer with counts and the next step
And installed plugins show "✓ installed"; connectors show their marketplace when they have no description and
  "openai" as developer for openai-curated-remote
And show prints a heading and an aligned key–value block; marketplace list prints a table with a header row
```

**AC6-24 Every error has a hint**
```gherkin
Given each error colctl can raise
Then its rendering ends with a hint line
```

## Help (T6)

**AC6-25 Grouped help**
```gherkin
When I run colctl --help
Then commands are grouped (Get started, Manage, Updates and review, Discover, Other) with one-liners that match what each
  command does in 2.2
```

**AC6-26 Examples**
```gherkin
Then every command's --help ends with an "Examples:" section
```

**AC6-27 Completions**
```gherkin
When I run colctl completions bash|zsh|fish
Then it prints a completion script for that shell that names every subcommand; an unknown shell is a usage error
```

## Proof and docs (T7)

**AC6-28 Snapshots**
```gherkin
Then every command's text output has snapshot tests at 60, 80 and 120 columns with colour off, and at 80 with colour on
```

**AC6-29 Docs**
```gherkin
Then docs/reference/output.md describes the tokens, components, flags, interaction rules and that human text is unstable;
  docs/explanation/how-colctl-talks.md explains the design; reference examples match the new output; links resolve
```

**AC6-30 Traceability and earlier suites**
```gherkin
Then every AC6/EC6 ID has a test named after it, and earlier suites keep their test counts with none ignored (text
  assertions updated and listed in the cycle review)
```
