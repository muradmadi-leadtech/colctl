# Cycle review: solid

_Cycle 9 · Evaluate · 2026-11-15_

| | |
|---|---|
| **Cycle** | `20261115-solid` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase. Generate ran **autonomously** under the policy in `.forge/context.md` |
| **Intent** | [PRD](../prd/solid.md): make what exists fast and complete, so the daily loop never makes you wait or drop to the CLI |
| **Scope delivered** | S1–S7. AC9-01 – 32 (30 kept, 2 dropped on purpose, below) and EC9-01 – 17 each have a named test |
| **Released** | v2.6.0 |
| **PRs** | #127 plan · #128 T1+T2 saved checks · #130 T3 saved status · #131 T4 core jobs · #132 T5 screens · #133 T6 palette · #134 T7 follow-ups · #135 T8 docs, smoke fixes · release PR |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on a copy with credentials and sessions removed, deleted afterwards |
| **Disposition** | _Yours_ |

## Where the 14 seconds went (T1, AC9-01)

On a copy of your profiles with databricks pinned 60 commits back and qdrant-skills 50 back, both updates held,
2.5.1's release build took **14.9 s** for `status` and **14.3 s** for `review`. A git wrapper logged every call
`status` made: **2 250 git processes, 2 227 of them `git cat-file blob`**, one per side of every changed file, read
for the risk flags. Everything else together took under 0.5 s.

## Timings after (S1), same copy, release build

| Command | 2.5.1 | 2.6.0 |
|---|---|---|
| `status`, first run after `update` (nothing saved) | 14.9 s | 0.78 s |
| `status`, after that | 14.9 s | 0.49 s |
| `review` | 14.3 s | 0.47 s |
| `review databricks` (diff read fresh) | ~15 s | 0.78 s |

Two changes: blobs are read through one `git cat-file --batch` process, and each held version's check is saved under
`~/.colctl/.state/cache/reviews/` by `update`/`refresh` and read back by everything after.

## What you get

- **Saved checks** (S1–S3): keyed by exactly what was held plus colctl's version and the risk rules' version, so a
  new hold, a hand-edited pin, an edited snapshot, a new colctl or new patterns all check again. Approve and reject
  forget the check. The diff is never saved. [Why](../explanation/saved-checks.md).
- **The TUI opens on your last status** (S4), marked `as of 3 min ago ·`, and swaps in the fresh one. `colctl status`
  itself still writes nothing; only the TUI saves its status.
- **Every command from the TUI** (S5): Items `x` remove, `c` convert or undo, `f` refresh; Profiles `a` adopt from a
  checklist; Discover Ctrl-V cycles Plugins → Developers → Marketplaces (⏎ refreshes) → Connectors.
- **A command palette**: `:` or Ctrl-K, every action with its key, fuzzy, the ones that work here first and the ones
  that can't run now last with the reason. The Overview menu stays, with **All actions** as its last entry.
- **Follow-ups** (S6), below.

## Follow-ups: closed or dropped (S6)

| Follow-up | Outcome |
|---|---|
| `allowed-tools` flag noise | **Closed** (#134). Files whose only flag is `allowed-tools` share one line; `-v` and `--json` still list each |
| `-v` printed full paths | **Closed** (#134). Tool calls print `~/…` |
| The `ETXTBSY` test flake | **Closed** (#134). A busy executable is retried (product code, so it also covers a tool upgraded mid-run); the new wrappers in tests are written and renamed |
| `sync`'s "needs attention" | **Closed** (#134). With one thing left, the verdict names it and its fix: `mine foreign → colctl adopt claude skills/mine` |
| `colctl marketplace` alone printed `✗ error:` with nothing after | **Closed** (#134), found during Refine. It prints its help, exit 2 |
| Connectors' empty fields (AC9-28) | **Dropped.** A connector row already leaves out an empty description. Leaving fields out of `--json` would change its shape, which S7 rules out |
| qdrant: status should suggest `adopt` (AC9-30) | **Dropped.** 2.5.0 fixed the case behind it (#114); your profiles show it in sync and no state is left where status suggests the wrong fix |

## Changes to the plan made during Generate

- **Discover's keys.** Refine said `v` cycles views and `u` refreshes marketplaces, but every letter types into
  Discover's search. The view key is **Ctrl-V**, and ⏎ on a marketplace asks to refresh them.
- **Only the TUI saves its status.** Saving it from every `colctl status` broke cycle 1's "status writes nothing"
  test, which is right. The TUI's worker saves each fresh status instead.
- **Connectors aren't in any catalog,** so ⏎ on one shows what `connectors` found instead of running `colctl show`
  (which can't find it). Found in the smoke test.
- **Convert holds the Codex package for review** before it's installed, so "undo" applies once that's approved. The
  tests follow that order.

## Smoke test (copy of your profiles, release build, real `claude`, `codex`, `git`)

- Timings above.
- `colctl` opened on `as of just now · … colctl status` and swapped in the fresh status.
- `:` listed actions, Overview's first; typing `adopt` put **Profiles · a · adopt** first.
- Items: `x` on forge asked "Remove skill forge from ~/.colctl and every profile: claude, kilo-claude, agents"; `n`
  left it. `c` on a skill said "Only source plugins convert for Codex".
- Discover: Ctrl-V showed 3 028 developers, then 8 marketplaces with the 6 colctl manages marked, then Connectors.
  The copy has no Codex login, so it found none and now says "No connectors found" (it said `No connectors match ""`
  before this fix).
- Profiles: `a` on claude and agents said "Nothing in claude that colctl doesn't already have".
- Review: ⏎ on databricks: `! Look before approving`, the curl-to-shell line, 123 flagged files.

## Strict TDD: the evidence

Every task PR opened on a RED commit whose new tests failed in CI, then GREEN: #128, #130, #131, #132, #133, #134.
One slip: #134's RED commit first went up with a lint error (a no-op `replace`), so CI would have failed on lint, not
the tests. It was amended before CI finished. The connector-detail fix in #135 came out of the smoke test; its test was
written with the fix.

## Test results

`just check` passes: fmt, clippy `-D warnings`, the full suite including the ignored timing test, coverage
**93.4% lines**, cargo-deny, typos, shellcheck, gitleaks.

## Security review

- **The cache is derived data.** It lives under `.state`, which `~/.colctl`'s `.gitignore` already excludes, and
  AC9-10 checks nothing under it is ever committed. A tampered entry can only change what `review` *shows* for a
  held version: it can't change what's installed, because approve installs the held commit from git and re-checks
  it's intact. Entries are keyed by the held commit or snapshot hash, so a stale or swapped entry is a miss.
- **Malware recommendation.** Saved checks carry the risk rules' version; new patterns recheck every held version.
- **New writes from the TUI** (rm, adopt, convert, refresh, marketplace refresh) run the CLI's own dispatch with the
  same confirmations shown first, and refuse exactly what the CLI refuses (EC9-17).
- **`ETXTBSY` retry** retries only that error, at most 10 times, with a short back-off.

## Autonomy gate: what was not done

- colctl 2.6.0 has **not** run on your real profiles. Nothing was written there.
- The disposition is yours.

## New follow-ups

- databricks's update flags 123 files, nearly all `script: new script`. The same grouping as `allowed-tools` could
  fold "new script" files under one folder into one line.
- The palette lists "Overview · ⏎ · run", which means little out of context; entries could say what ⏎ does there.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.6.0`, then `colctl --version` says 2.6.0
- [ ] `colctl update`, then `time colctl status` and `time colctl review`: each under a second
- [ ] `colctl`, quit, `colctl` again: the first frame says `as of … ·` and then updates
- [ ] `:` (or Ctrl-K), type `upd`, ⏎: it asks to check for updates
- [ ] Items: `x` on a skill shows where it would be removed from; `n` leaves it
- [ ] Items: `c` on typescript-lsp says why Codex can't use it
- [ ] Profiles: `a` on a profile with something foreign lists it, ticked
- [ ] Discover: Ctrl-V through Developers, Marketplaces and Connectors (your real Codex login lists connectors)
- [ ] `colctl review <a held source>` shows one `allowed-tools` line when many skills declare it; `-v` lists them
- [ ] `colctl marketplace` prints its help
