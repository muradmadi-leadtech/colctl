# Cycle review: TUI

_Cycle 8 · Evaluate · 2026-11-14_

| | |
|---|---|
| **Cycle** | `20261114-tui` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase. Generate ran **autonomously** under the policy in `.forge/context.md` |
| **Intent** | [PRD](../prd/tui.md): bare `colctl` in a terminal opens a full-screen view over the core loop, and the CLI gets each gap the TUI exposes first |
| **Scope delivered** | All 15 success criteria; AC8-01 – 31 and EC8-01 – 26 each have a named test. Not built, as agreed: screens for adopt, convert, rm, init, refresh, marketplaces, connectors, developers; a command palette; themes; Windows |
| **Released** | [v2.4.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.4.0) |
| **PRs** | #96 focus · #97 orchestrate · #98 refine · #99 T1 · #100 T2 · #101 T3 · #102 T4 · #104 T5 · #105 T6 · #106 T7 · #107 T8 · #108 T9 · #109 T10 · #110 T11 · #111, #112 smoke fixes · #103 release. Before the cycle: #94 (a snapshot held the test day's date), #95 (cycle 7 closed) |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on copies with credentials and sessions removed |
| **Disposition** | _Yours to decide_ |

## Your setup, in the TUI (a copy, real `claude`, `codex`, `git`)

Opened on a copy of `~/.colctl`, `~/.claude`, `~/.codex`, `~/.agents` and both kilo profiles, in a real pseudo-terminal
(pyte), at 120×40, 110×34, 100×30 and 80×24. The binary was built from `main` before release-please bumped the
version, so the screens say 2.3.0:

```
 Overview  Profiles  Items  Review  Discover                                       colctl 2.3.0
────────────────────────────────────────────────────────────────────────────────────────────────
 ✓ All 5 profiles in sync · 3 sources · nothing held for review

 Profiles
   ✓ claude       ~/.claude                    14 skills · 3 plugins
   ✓ kilo-claude  ~/Library/…/profiles/claude  14 skills · 3 plugins
   ✓ codex        ~/.codex                     3 plugins · skills via ~/.agents
   ✓ kilo-codex   ~/Library/…/profiles/codex   3 plugins · skills via ~/.agents
   ✓ agents       ~/.agents                    14 skills
```

- **Items:** 17 items across 5 profiles. The 14 skills are ✓ in claude, kilo-claude and agents (– in the Codex
  profiles, which read them through `~/.agents`). The 3 sources are ✓ in the four Claude and Codex profiles.
- **Discover:** 3,763 plugins from 8 catalogs. Typing `qdrant` drew each key in 12–26 ms. ⏎ showed
  `colctl show`'s text for qdrant-skills.
- **Review:** `u` fetched your three sources from GitHub with progress per source (`✓ superpowers  up to date`,
  `⠏ qdrant-skills  fetching  2/3`) and held two updates: databricks (62 commits, 123 flagged files) and
  qdrant-skills (51 commits, 20 flags). ⏎ opened the qdrant-skills review (2,797 lines), and `]` jumped to the first
  file in the diff.
- **Approve:** approving qdrant-skills ran to the end. It failed in both Claude profiles exactly as
  `colctl approve` does on a second copy (see the follow-ups). The toast's wording is fixed in #112, which has a
  test; I didn't rerun the approve on a copy after that fix.
- **`NO_COLOR`:** 0 coloured cells on every screen, animations included. **Reduced motion:** clean frames from the
  start. Every run exited 0 and gave the terminal back.

## What the smoke test changed (#111, #112)

- **Say when it's loading.** Reading what's held took ~14 s with those two big updates, and the TUI showed the old
  state without a sign. The status line now shows a spinner and "loading…" while loads are in flight.
- **Show a run's failure.** Approving qdrant-skills failed in both Claude profiles, yet the toast said
  "✓ Approved 1 held version". approve and add print their own verdict first and sync's last, and the TUI only
  showed the first. Now every verdict shows, worst first, and a failure stays until a key is pressed:
  `✗ Synced 3 of 5 profiles · 2 failed · ✓ Approved 1 held version`.
- A confirmation is as tall as its text. `] next file` appears only where there's a diff. update's per-source line
  no longer repeats the source's name.

Found while building, before the smoke test: the first frame never drew, because every loop's tick cleared the
redraw flag (a pty smoke test in T5 caught it); in a long list under a header, `G` left the cursor one row off
screen; a stale-lock warning would have printed over the TUI; and a write could have been cut off if the terminal
died. Each now has a test.

## Strict TDD: the evidence

Every task's PR opened on a RED commit that failed CI on its new tests, then GREEN was pushed. Two slips, both
corrected:

- **T9:** the RED commit failed on formatting (I edited the test file after `cargo fmt`). A formatted RED commit was
  pushed and failed on the tests before GREEN.
- **T6:** GREEN was pushed with the gate red (formatting). A format commit followed before the merge. Since then, the
  gate, the commit and the push run as one chained command.

## Changes to the plan made during Generate

- **Runs return the CLI's text.** `core::run` returns the lines the command would print, plus progress through
  `Observer`, rather than one typed struct per command. The status snapshot, the held list and the catalog are typed.
  `review` and `show` are the CLI's text, captured at the overlay's width. This kept every command byte-identical,
  with less risk, and gives the TUI exactly the CLI's words.
- **`core` borrows from `commands`.** It reaches some of `commands`' internals (`pub(crate)`) instead of moving them.
  The rule that matters is enforced by a test: `src/tui` never imports `commands`, `plan`, `apply` or `adapter`.
- **Tool and git calls get `HOME` from the context.** This makes in-process runs safe: a test can never reach this
  machine's real `~/.claude`.
- **Fuzzy search has a lower tier.** 2.3's matches and order are kept. Letters-in-order only matches names and
  keywords, since almost any description contains a term's letters in order.
- **Motion runs in the runtime, not the model.** The effects keep state, so they live beside the terminal, and the
  model stays pure. An overlay closes without an animation. Rows in a reloaded list re-form together, not one by one.
- **Without colour, motion uses characters.** tachyonfx fades interpolate colours, so with `NO_COLOR` only coalesce
  runs.

## Earlier tests changed

None. The earlier suites' counts are pinned, and every CLI snapshot passed unchanged through the core extraction.
`tests/docs.rs` gained cycle 8 in its traceability check.

## AC → test traceability

Every AC8 and EC8 id has at least one test whose name contains it, checked by
`tests/docs.rs::…every_criterion_and_edge_case_has_a_named_test`. The suites are `tests/core.rs` (core API, 18
tests) and `tests/tui.rs` (77 tests: model, view and keys driven end to end on fixtures, with snapshots at 60×20,
80×24, 100×30 and 120×40). AC8-30 has one parity test per action: sync, sync one profile, update, approve, approve
all, reject and add each run through the TUI on one machine and through the CLI on an identical one, and the results
are compared.

## Test results

`just check`: fmt, clippy `-D warnings`, 618 tests, audit (cargo-deny, typos, shellcheck, gitleaks). Line coverage
93.6% (gate 90%). The only code not exercised is the real terminal loop (`tui::run`, `frames`, the guard), which the
pty smoke test covered.

## Security review

- **Nothing reaches the terminal raw.** Names, paths, descriptions and diffs are escaped (`review::escape`) before
  they're drawn (EC8-21, tested with an escape sequence in a skill's name).
- **Writes ask first and say what they'll do.** Only `y` confirms. ⏎ alone does nothing in a confirmation, and Esc
  and `n` change nothing. One write runs at a time, under the same lock as the CLI.
- **Untrusted updates stay reviewed.** Approve installs exactly what `review` showed (cycle 4's digest check,
  unchanged), and the TUI shows the same review text.
- **The terminal is always given back:** on quit, Ctrl-C, error, panic (exit 101, reported on the normal screen) and
  a dead terminal, which first lets a running write finish.
- **Tests can't touch real profiles.** Tool and git calls get `HOME` from the context, never from the test process.

## Follow-ups (the CLI, found by the TUI)

- **Held-version inspection is slow.** `colctl status` and `colctl review` take ~14 s with two big updates held (62
  and 51 commits). Caching the review per held commit would make both instant.
- **qdrant in your Claude profiles comes from `qdrant@qdrant`**, not from colctl's clone, so approving
  qdrant-skills' update fails there ("the plugin comes from somewhere other than colctl's clone"). `colctl adopt`
  moves it to its source, the same fix as in cycle 3. Status could name that fix instead of `colctl sync`.
- Carried: tilde paths in `-v` output; the `ETXTBSY` test flake.

## Autonomy gate: what was not done

- colctl 2.4 has **not** run on your real profiles. Your installed colctl is 2.3.0.
- The two updates held on the copy (databricks, qdrant-skills) are **not** held on your machine. `colctl update`
  there would hold them.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.4.0`, then `colctl --version` says 2.4.0
- [ ] `colctl` in your terminal opens on Overview with the same verdict as `colctl status`; `colctl | cat` prints the
  help instead
- [ ] Tab through the five tabs (or click them); `?` lists the keys; `q` quits and your terminal is as it was
- [ ] Items: your 14 skills and 3 sources, a column per profile
- [ ] Review: `u` checks for updates (it fetches your three sources), then ⏎ on one shows its review; `]` jumps
  between files; `x` and `n` leave it held
- [ ] Discover: `/` then type `qdrant`; ⏎ shows the plugin
- [ ] `NO_COLOR=1 colctl` and `COLCTL_REDUCED_MOTION=1 colctl` look right to you
- [ ] Make the window smaller than 60×20, then bigger again
- [ ] `colctl search sprpw` finds superpowers
- [ ] Decide the disposition

See `docs/tasks/tui.md` (T1–T12), `docs/tui/design-spec.md`, `docs/reference/tui.md`.
