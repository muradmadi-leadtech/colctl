# PRD: CLI output design

_Cycle 6 · Focus · 2026-11-13_

## Problem statement

colctl's commands work, but each one presents information its own way, and most present too much of the wrong thing:
- **No answer up front.** `status` on the owner's machine prints 90+ lines of `ok` and only then says `in sync`.
  Nothing up front says "all good", or "2 things need you".
- **Logs, not results.** `sync`, `add` and `approve` print every underlying `claude`/`codex` call, so reading the
  output means translating tool calls back into intent.
- **Contradictions and misalignment.** After `add source`, the output says "needs attention" about informational
  `unsupported` items, then ends with "in sync". Labels like `kilo-codex:unsupported` push columns out of line.
- **Silence while working.** A sync that calls the tools a dozen times shows nothing until it's done.
- **Uneven errors.** Some say what to run next, some don't; the formats differ between colctl and clap.
- **Out-of-date help.** `--help` still says `update` moves pins and `refresh` pulls updates (both changed in 2.0) and
  lists 17 commands in one ungrouped list.
- **Interaction where it would help, and none of it.** `colctl approve` with nothing named fails instead of
  offering what's held; `rm source` removes a plugin from every profile without asking; a long review diff
  scrolls past.

The best CLIs (cargo, uv, gh, jj, mise) treat presentation as a designed system. colctl should too: a person shouldn't
need a TUI to understand it.

## Target users

People running colctl in a terminal, daily. Also scripts and the future TUI, which rely on `--json` and exit codes
staying exactly as they are.

## The design system (what this cycle builds)

Twelve rules, implemented once in an output layer that every command uses:

1. **Lead with the answer.** The first line of a result says what happened or what state things are in
   ("All 5 profiles in sync", "2 updates held for review").
2. **Show what matters; detail on request.** Default views show problems and changes; `--all` (or `-v`) shows
   everything; `show`/`review` give one thing in full.
3. **Say what happened, not how.** Results speak in outcomes ("installed superpowers in 4 profiles"); the tool calls
   behind them appear with `-v`.
4. **Every problem carries its fix.** A state that needs action shows the command that resolves it.
5. **Live while working.** In a terminal, long work shows progress (per profile, per step, with a spinner) and ends in
   a summary. Piped output prints plain lines, no animation.
6. **Colour adds, never carries.** Each state has a symbol and a word as well as a colour (✓ ok, ! attention,
   ✗ failed, · info). `NO_COLOR`, `--color auto|always|never`, and no escape codes when not a terminal.
7. **Fits the terminal.** Tables measure the terminal and truncate with "…" instead of wrapping; columns always
   line up; prose wraps at the width.
8. **One vocabulary, one set of components.** Heading, key–value rows, table, item with a state badge, summary,
   hint, warning, error, progress: the same look in every command.
9. **Errors say what, why and what to do.** `error: …`, an indented cause, and `hint: …`, for colctl's errors and for
   argument errors alike. Exit codes don't change.
10. **Ask only when it helps, and only in a terminal.** Confirm before destructive actions (`--yes` skips); offer a
    picker when a required name is missing; page long output through `$PAGER` (`--no-pager`). Never with `--json`,
    in a pipe, or in CI.
11. **Help is a guide.** Commands grouped by task (set up, sync, discover, review), descriptions that match what the
    commands do today, examples in every command's `--help`, and shell completions.
12. **Quiet and verbose are real.** `-q` prints only errors and the final line; `-v` adds tool calls and timings.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | One output system. | Every command's human output goes through the output layer; a test fails if a command prints directly. |
| SC-2 | Answers first. | `status`, `sync`, `add`, `adopt`, `update`, `refresh`, `approve`, `reject`, `review`, `search` each start (or, while running, end) with a one-line verdict; snapshot tests show it. |
| SC-3 | Calm defaults. | On a fixture like the owner's machine (5 profiles, ~100 items, all in sync) `status` prints at most 8 lines; `status --all` lists every item. |
| SC-4 | Live progress. | In a terminal, `sync` shows progress per profile while tools run; in a pipe the same command prints no control characters. |
| SC-5 | Colour discipline. | No escape codes with `NO_COLOR`, `--color never`, or a non-terminal stdout; every coloured state also has a symbol and word. |
| SC-6 | Fits 60, 80 and 120 columns. | Snapshot tests at each width: no wrapped table rows, columns aligned, long names cut with "…". |
| SC-7 | Errors with fixes. | Every error colctl raises renders as error / cause / hint; argument errors use the same look; exit codes unchanged. |
| SC-8 | Helpful interaction, safely. | Destructive commands confirm in a terminal and accept `--yes`; `approve`/`reject`/`review`/`show` offer a picker without a name in a terminal; none of this happens when not a terminal or with `--json`. |
| SC-9 | Help that guides. | Grouped `--help`, accurate one-liners, an example section on every command, `colctl completions <shell>`. |
| SC-10 | Nothing else changes. | `--json` output and exit codes are byte-identical to 2.1 for the same input; earlier suites pass (text assertions updated and listed). |
| SC-11 | Documented and tested. | A reference page for the output system and conventions; snapshot tests per command; strict TDD; ≥ 90% coverage. |

## Scope

**In:** the output layer and the twelve rules applied to every command; `status --all`, `-q`/`-v`, `--color`,
`--yes`, `--no-pager`; pickers and confirmations; paging; grouped help with examples; shell completions; the
follow-ups deferred from cycles 4–5 (review flag grouping and alignment, sync's contradictory wording, connector
fields); docs; release 2.2.0 with human-readable text declared unstable (scripts use `--json`).

**Out (won't build this cycle):**
- **Full-screen or multi-pane views**, and key-driven browsing inside a command. That's the TUI (cycle 8).
- **User-configurable themes** beyond `NO_COLOR` and `--color`.
- **Translations**, and man pages.
- **Any change to `--json` or exit codes.**
- **New features.** This cycle changes how existing commands present and interact, not what they do.
