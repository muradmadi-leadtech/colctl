# Cycle review: convert

_Cycle 7 · Evaluate · 2026-11-13_

| | |
|---|---|
| **Cycle** | `20261113-convert` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase. Generate ran **autonomously** under the policy in `.forge/context.md` |
| **Intent** | [PRD](../prd/convert.md): a source plugin that only packages for Claude can be used in Codex too, opted in per source and held for review; skills and the hooks Codex runs carry over, everything else is named |
| **Scope delivered** | All 8 success criteria. Not built, as agreed: Codex → Claude, other tools, commands → skills, agents and MCP servers in Codex, trusting hooks for you |
| **Released** | [v2.3.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.3.0), attested |
| **PRs** | #83 plan · #84 T1 · #85 T2 · #87 T3 · #88 T4 · #89 T5 · #90 T6 · #91 T7 · #92 smoke fix · #86 release |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on a fresh copy |
| **Disposition** | Accept (owner, 2026-11-14) |

## Your setup, in 2.3 (fresh copy, real `claude`, `codex`, `git`)

A copy of `~/.claude`, `~/.codex`, `~/.agents`, `~/.colctl` and both kilo profiles (credentials left out), with
paths and links pointed at the copy:

```
$ colctl convert qdrant-skills
! Held qdrant-skills's Codex package for review · built from d2127f6

  Carries   13 skills

→ colctl review qdrant-skills
```

```
$ colctl approve qdrant-skills
✓ Approved 1 held version
  ✓ approved qdrant-skills's Codex package

  ✓  codex        added marketplace colctl-qdrant-skills · installed qdrant
  ✓  kilo-codex   added marketplace colctl-qdrant-skills · installed qdrant

$ colctl status
✓ All 5 profiles in sync · 3 sources · nothing held for review

  ✓  claude       ~/.claude                                        14 skills · 3 plugins
  ✓  kilo-claude  ~/Library/…/kilo-proxy-headless/profiles/claude  14 skills · 3 plugins
  ✓  codex        ~/.codex                                         3 plugins · skills via ~/.agents
  ✓  kilo-codex   ~/Library/…/kilo-proxy-headless/profiles/codex   3 plugins · skills via ~/.agents
  ✓  agents       ~/.agents                                        14 skills
```

The real `codex plugin list` shows `qdrant@colctl-qdrant-skills  installed, enabled  1.0.0`, with its skills in
Codex's cache. Two more checks against the real CLI: `codex plugin add` refreshes the cache when the package changes
under the same version (so updates reach Codex), and `colctl convert --undo` uninstalled it from both Codex profiles.

## What the smoke test changed (#92)

qdrant's plugin folder is its whole repo. The first package carried all of it, 140 files: CI workflows, eval and
scoring scripts. Review flagged 36 files that Codex never reads. A package without hooks now carries only the
skills folder and the top-level README/LICENSE/NOTICE: 36 files, 9 flags, all about the skills (their
`allowed-tools`). With a hook kept, the whole folder still comes along, because the scripts a hook runs can be
anywhere. AC7-02 was updated to say so.

## Strict TDD: the evidence

Each PR was opened on its RED commit (formatted, so CI reached the tests), CI failed on the new tests, then GREEN was
pushed. The next task started only after the GREEN was pushed.

| PR | RED on CI |
|---|---|
| #84 T1 | `colctl::repackage` didn't exist (compile error) |
| #85 T2 | 5 hook tests failed |
| #87 T3 | `HeldConversion` and `set_source_convert` didn't exist |
| #88 T4 | 15 convert tests failed |
| #89 T5 | 9 sync/status tests failed |
| #90 T6 | 3 update tests failed |
| #91 T7 | the docs test failed (pages missing) |
| #92 fix | 1 test failed |

## Changes to the plan made during Generate

- **T1/T2 were split** as planned, but the hook mapper landed in `repackage.rs` (no separate module).
- **Status hint** (AC7-13): the fix reads `the repo has no Codex packaging; colctl convert <name> packages it`. A
  bare `colctl convert <name>` read badly in sync's output. AC7-13 updated.
- **Undo uninstalls through the plan**: a source whose Codex packaging is gone, but which colctl installed before, is
  now uninstalled by sync. That's what `--undo` relies on, and it also tidies up any other packaging that disappears.
- **Reviews of converted updates** build the new package from a throwaway `git clone --shared` of the clone, so the
  clone itself is never touched. `status` does this too while such an update is held, which makes it a little slower
  then.

## Earlier tests changed

None. `ac3_09` and `ac6_13` still pass because the new hint keeps the old wording.

## AC → test traceability

Every AC7 and EC7 id has a test named after it (`tests/docs.rs` checks: 17 AC7, 20 EC7). The tests are in
`tests/repackage.rs` (16: the package and the hook mapping), `tests/convert_records.rs` (6: config and state) and
`tests/convert.rs` (32: the command, review, approve, reject, sync, status, updates, undo, lock, failure, JSON).

## Test results

515 tests pass; coverage 94.61% of lines (floor 90%). `just check` is green: fmt, clippy `-D warnings`, llvm-cov,
typos, actionlint, zizmor, shellcheck, gitleaks.

## Security review

- **Untrusted content stays reviewed.** A conversion is held like an update. Approve installs exactly the package
  that was reviewed (the digest is checked against a rebuild from the pinned commit), and an update's package is
  shown in the same review.
- **Hooks are code.** They're copied unchanged and never rewritten, flagged in review, and Codex asks you to trust
  them again (`/hooks`). colctl says so when you approve.
- **No escape from the plugin folder.** Manifest `skills`/`hooks` paths that leave the folder are refused. Symlinks
  pointing outside aren't followed, and they're listed in the report.
- **The clone is never written.** Packages are built elsewhere, in a temp folder, and renamed into place, so Codex
  never reads half a package.
- **Commands run with fixed arguments.** `git` gets a 40-hex commit and paths colctl built; no shell.

## Follow-ups

- Codex → Claude, if a Codex-only plugin shows up.
- Commands could become skills for Codex (left out on purpose this cycle).
- `review` still flags `allowed-tools` on every skill; grouping those flags would make reviews shorter.
- Carried: tilde paths in `-v` output; the `ETXTBSY` test flake.

## Autonomy gate: what was not done

- colctl 2.3 has **not** run on your real profiles. qdrant-skills is still Claude-only there until you convert it.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.3.0`
- [ ] `colctl status --all`: the qdrant-skills rows on Codex now point at `colctl convert`
- [ ] `colctl convert qdrant-skills`, then `colctl review qdrant-skills`
- [ ] `colctl approve qdrant-skills`, then `colctl status`: both Codex profiles say `3 plugins`
- [ ] In Codex: `/plugins` lists qdrant; its skills are there
- [ ] Optional: `colctl convert --undo qdrant-skills --dry-run`
- [ ] Decide the disposition

See `docs/tasks/convert.md` (T1–T8), `docs/convert/containers.md` and `components.md`.

**Disposition: Accept** (owner, 2026-11-14).
