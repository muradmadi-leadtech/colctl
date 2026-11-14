# PRD: TUI

_Cycle 8 · Focus · 2026-11-14_

## Problem statement

colctl's CLI is now complete and consistent (cycles 1–7), but working with it is still a series of separate
commands. The daily loop runs `status`, then `update`, then `review <name>` for each held item, then `approve`, then
`sync`, and finding something new runs `search`, `show`, then `add`. Each step prints its answer and exits. To see the
whole picture you hold it in your head, and to act on what you see you retype names.

A full-screen UI fixes that. It shows the state of every profile at once, keeps the review queue in front of you, and
lets you act on what you're looking at with one key. The bar is Claude Code-level polish: calm, fast, animated with
purpose, keyboard first.

The TUI also tests the CLI. A screen that needs a fact or action the CLI doesn't expose shows where the CLI falls short.
This cycle closes those gaps in the CLI first, so the TUI stays a view over logic the CLI already has.

## Target users

- **The owner**, daily, in a terminal on macOS and Linux, keeping 5 profiles (Claude, Codex, kilo) in sync.
- **Someone new to colctl**, who learns what it manages by looking around instead of reading `--help`.

Scripts and CI keep using the CLI. The TUI never opens for them.

## Decisions (owner, 2026-11-14)

- **Scope: the core loop.** Overview, profiles, items, sync, the review queue (update, review, approve, reject) and
  discovery (search, show, add). `adopt`, `convert`, `rm`, `init`, marketplaces, connectors, developers and the
  refinement pass come in later cycles.
- **CLI first, same cycle.** Each gap the TUI exposes becomes a CLI task: a command, a flag or a `--json` field, with
  its own tests. That task lands before the screen that uses it. The TUI has no logic of its own.
- **Layout: tabs across the top, one full-width view per tab.** Detail opens as a drill-in or an overlay. A key bar
  sits along the bottom.
- **Keyboard first, mouse too.** Every action has a key. Clicking tabs and rows, and scrolling with the wheel, also
  work.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | Opens where it should, only there. | Bare `colctl` with stdin and stdout both terminals opens the TUI. Piped, redirected, `TERM=dumb`, or `CI` set, it prints the welcome help exactly as in 2.3. `colctl help` stays help. Tests cover each case. |
| SC-2 | Five tabs. | Overview, Profiles, Items, Review (with the held count) and Discover. Each has a snapshot test at 80×24 and 120×40. |
| SC-3 | The answer first. | Overview's first line is the same verdict `colctl status` gives. Below it come one line per profile and what needs you, each with its fix as an action you can run in place. |
| SC-4 | Sync with live progress. | Syncing from the TUI shows progress per profile while the tools run and ends with the verdict. The profiles end up in the same state as after `colctl sync`. |
| SC-5 | The review loop. | From the Review tab: check for updates, open a held item's changes and risk flags, approve or reject it after a confirmation. Each ends in the same state as the CLI command. |
| SC-6 | Discovery. | Fuzzy search (nucleo) across every catalog, updated as you type. Open a plugin's full detail and add it. The result matches `search`, `show` and `add`. |
| SC-7 | One logic. | Every TUI action calls the code the CLI command calls. A parity test runs each action both ways on the same fixture and compares the resulting state. |
| SC-8 | CLI gaps closed in the CLI. | Each gap found is a CLI change with its own tests, merged before the screen that needs it, and listed in the cycle review. |
| SC-9 | Designed before built. | A design spec (colour tokens, keymap, components, motion) is written and agreed in Refine, before any TUI code. Screens follow it. |
| SC-10 | Polished motion that respects you. | tachyonfx transitions between tabs and on overlays. No motion with `COLCTL_REDUCED_MOTION=1`. No colour with `NO_COLOR` or `--color never`, with symbols and words still carrying meaning. |
| SC-11 | Keyboard and mouse. | Every action has a key, shown in the key bar and in a `?` overlay. Clicks on tabs and rows and the wheel work. Key-driven tests cover each screen. |
| SC-12 | Never stuck, never broken. | Tool calls and fetches run off the UI thread: the UI keeps redrawing and Esc cancels where the CLI allows it. Below 60×20 it says so instead of drawing garbage. The terminal is restored on quit, Ctrl-C, error and panic. |
| SC-13 | Fast to open. | The first frame draws from local state only, without network or tool calls. Catalogs load in the background. |
| SC-14 | Nothing else changes. | CLI human output, `--json` and exit codes stay as in 2.3, apart from additive gap fixes. All earlier suites pass. |
| SC-15 | Documented and tested. | A tutorial and reference page for the TUI. Strict TDD, tests named after AC/EC ids, ≥ 90% coverage. Released as 2.4.0. |

## Scope

**In:** the TUI's shell (launch rule, tabs, key bar, help overlay, mouse), the five tabs,
sync, update, review, approve, reject, search, show and add from the TUI; the CLI gap fixes those need; the design
spec; motion with a reduced-motion switch; snapshot and key-driven tests; docs; release 2.4.0.

**Out (won't build this cycle):**
- **Screens for `adopt`, `convert`, `rm`, `init`, `refresh`, marketplaces, connectors and developers.** These come in
  later cycles, alongside the refinement pass.
- **A command palette.** Tabs and keys first; a palette can come with the refinement pass.
- **User themes or a config file for the TUI.** Only `NO_COLOR`, `--color` and reduced motion.
- **Editing `~/.colctl` files, AGENTS.md or settings in the TUI**, or running `claude`/`codex` inside it.
- **Windows.** colctl ships for macOS and Linux.
- **Logic that exists only in the TUI.** If a screen needs it, the CLI gets it first.
