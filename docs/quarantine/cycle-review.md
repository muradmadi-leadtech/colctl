# Cycle review: quarantine

_Cycle 4 · Evaluate · 2026-11-12_

| | |
|---|---|
| **Cycle** | `20261112-quarantine` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase. Generate ran **autonomously** under the policy in `.forge/context.md`, extended to this cycle |
| **Intent** | [PRD](../prd/quarantine.md): new versions of source plugins and taken-over skills wait for the owner's review and approval before they reach any harness |
| **Scope delivered** | Everything in the PRD's "In" list. Not built, as agreed: holding first-time adds, holding list-plugin versions, code scanning, background checks, any UI |
| **Released** | [v2.0.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.0.0): both archives attested (`gh attestation verify` exit 0); `mise exec github:muradmadi-leadtech/colctl@2.0.0 -- colctl --version` prints `colctl 2.0.0`, and the released binary reports `in sync` on the smoke copy. Breaking: `update` and `refresh` hold instead of install; list plugins are `unreviewed` |
| **PRs** | #40 plan · #41 T1 · #42 T2 · #44 T3 · #45 T4 · #46 T5 · #47 T6 · #48 T7 · #49 T8 · #50 T9 · #51 typos fix · #43 release |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on a copy |
| **Disposition** | _Yours to decide_ |

## Your story, on a copy of your real setup

Run with the 2.0 binary, the real `claude`, `codex` and `git`, and the real `github.com/obra/superpowers`, on a copy of
your `~/.colctl`, both Claude profiles, both Codex profiles, `~/.agents` and twg's skills folder (absolute paths
re-pointed at the copy). It started **in sync**, as your machine is.

Upstream superpowers is at your pin (v7.0.2), so there was nothing new to hold. To exercise the flow on real code, older
releases were held with `--rev`:

1. **`update --rev v7.0.1`** held `2ce8aa1` ("goes back 1 commit"); nothing was installed. **`review`** listed it, and
   **`review superpowers`** showed the removed commit, 61 changed files and **46 flags**, including
   `hooks/hooks.json` (hook), `hooks/session-start` (executable, script) and the Claude and Codex manifests.
2. **`reject`** dropped it; **`update --rev v7.0.1`** again printed `2ce8aa1 was rejected; nothing held`.
3. **`update --rev v6.4.2`** held `8ca22db`; `status` said `pending-review … → colctl review superpowers` and
   `in sync; 1 held for review`. **`approve`** committed the pin and the real CLIs updated all four profiles.
   Claude's `installed_plugins.json` (both profiles) and Codex's plugin cache (both profiles) then said **6.4.2**.
4. **`update`** held upstream's head again (`8ca22db → 22f0b46, 4 new commits`), and **`approve --all`** put every
   profile back on 7.0.2.
5. **twg skills:** in the copy of twg's folder, `twg-jira` gained a `!`twg doctor`` line and `twg-artifacts` gained
   `allowed-tools: Bash(twg:*)`. `status` showed both as `origin-changed`. **`refresh`** held both without touching
   colctl's copies, and **`review`** flagged them `inline-shell` and `allowed-tools … grants Bash: Bash(twg:*)`.
   **`approve skills/twg-artifacts`** replaced the copy (one commit, every profile followed through its link).
   **`reject skills/twg-jira`**, then `refresh`, printed `this version was rejected; nothing held`.
6. Final `status`: **in sync**, exit 0.

## Strict TDD: the evidence

Each task pushed its RED commit first, and CI's `check` failed on it:

| PR | Task | RED run | Failed tests |
|---|---|---|---|
| #41 | T1 storage | [43271240757](https://github.com/muradmadi-leadtech/colctl/actions/runs/43271240757) | 3 |
| #42 | T2 risk flags | [43271587143](https://github.com/muradmadi-leadtech/colctl/actions/runs/43271587143) | 7 |
| #44 | T3 update holds | [43272320953](https://github.com/muradmadi-leadtech/colctl/actions/runs/43272320953) | 12 |
| #45 | T4 refresh holds | [43273298585](https://github.com/muradmadi-leadtech/colctl/actions/runs/43273298585) | 6 |
| #46 | T5 review | [43274080259](https://github.com/muradmadi-leadtech/colctl/actions/runs/43274080259) | 11 |
| #47 | T6 approve | [43274826441](https://github.com/muradmadi-leadtech/colctl/actions/runs/43274826441) | 13 |
| #48 | T7 reject | [43275384018](https://github.com/muradmadi-leadtech/colctl/actions/runs/43275384018) | 4 |
| #49 | T8 status | [43276359879](https://github.com/muradmadi-leadtech/colctl/actions/runs/43276359879) | 3 |
| #50 | T9 docs | [43277257384](https://github.com/muradmadi-leadtech/colctl/actions/runs/43277257384) | 2 |

For T3–T9, RED was the new tests run against the previous code, so they fail on real behaviour. T1 and T2 used stubs.

**Where it slipped, stated plainly:**
- **T1's RED never ran in CI as a PR**, because I pushed RED and GREEN before opening the PR (CI only runs on PRs). I
  started CI by hand on a branch at the RED commit (run above). From T2 on, the PR was opened on RED.
- **#40 (the plan) failed CI** because I pushed it without running the gate: `typos` read the crate name "ratatui" in
  the roadmap as a typo. That broke the rule from cycle 3 (gate before every push). Fixed with `_typos.toml`.
- **In-suite RED counts understate** some tasks: test binaries stop at the first failing one, so adjusted earlier tests
  (`ac3_30`, `ec3_19` in T8) weren't reached in the RED run. They fail against the old code locally.
- **Test-fixture fixes during GREEN:** a helper wrote `"quarantine": null` into state (T3), a helper didn't create
  `~/.agents/skills` (T4), and a test used a marketplace source the config rejects (T8). None were code bugs.
- **A unit test (`ec4_18`) lives in the GREEN commit only**, because RED stashed `src/` and the test sits in
  `src/commands/quarantine.rs`.

## Changes to the plan made during Generate

| What | Why |
|---|---|
| **AC4-29:** `in_sync` doesn't count pending reviews; `status` still exits 1 while anything is held | Counting them made `sync` and `approve` exit 1 whenever any other version was still held |
| **AC4-31** moved from T8 to T6 | `approve` needed it: a still-held skill showed as `origin-changed` and made sync report drift |
| **EC4-20:** the lock error exits **3**, not 2 | That's colctl's existing lock error; the spec had it wrong |
| **`review --json` gained `warning`** (EC4-14) | So the TUI can show "pin changed by hand" without parsing text |
| `commands.rs` became `commands/mod.rs` + `commands/quarantine.rs` | It was 1,861 lines; the quarantine commands live in their own file |

## Earlier tests changed (SC-6)

| Test | Change |
|---|---|
| `tests/source_commands.rs`::`ac3_15` | `update` holds; the 1.2 outcome (pin, clone, every profile) is checked after `approve` |
| `tests/source_commands.rs`::`ac3_16` | `--rev` holds the commit instead of pinning it |
| `tests/source_commands.rs`::`ac3_30` | list plugin is `unreviewed` (was `ok`), same source hint |
| `tests/source_commands.rs`::`ec3_19` | the adopt refusal says `state: unreviewed` |
| `tests/takeover.rs`::`ac2_26` | `refresh` holds; the 1.1 outcome is checked after `approve` |

No test was removed or ignored. The guard now also counts `sources.rs` (17) and `source_commands.rs` (17).

## What changed

`git diff --stat v1.2.1..main -- src tests`: 14 files, +3,737 / −233.

| File | Nature |
|---|---|
| `src/commands/quarantine.rs` (new, 956) | `update`, `refresh`, `review`, `approve`, `reject`, status helpers |
| `src/risk.rs` (new, 587) | Pure risk flagger with table tests |
| `src/review.rs` (new, 400) | Commits, changed files and diffs from `git`; terminal-safe escaping |
| `src/state.rs` (+188) | `quarantine`, `rejected`, validation, snapshot paths |
| `src/vault.rs` (+63) | `refs/colctl/candidate`: hold, held, release |
| `src/commands/mod.rs` (−258 net) | `update`/`refresh` moved out; `status` and `rm` changes; `origin-changed` leaves out decided content |
| `src/plan.rs` (+49) | `unreviewed` |
| `src/cli.rs`, `src/git.rs`, `src/lib.rs` | New subcommands; `Git::bytes` |
| `tests/quarantine.rs` (new, 1,296) | 51 end-to-end tests |

## AC → test traceability

Every AC4 and EC4 ID has at least one test named after it; `tests/docs.rs` checks this (33 + 23 IDs).

| ID | Test(s) |
|---|---|
| AC4-01 | `src/state.rs`::`ac4_01_older_state_files_load_and_new_fields_appear_only_when_used` |
| AC4-02 | `tests/quarantine.rs`::`ac4_02_held_content_is_never_committed` |
| AC4-03 | `src/vault.rs`::`ac4_03_a_held_commit_survives_force_push_and_gc` |
| AC4-04 | `src/risk.rs`::`ac4_04_hooks_are_flagged` |
| AC4-05 | `src/risk.rs`::`ac4_05_mcp_and_lsp_servers_are_flagged` |
| AC4-06 | `src/risk.rs`::`ac4_06_code_that_can_run_is_flagged` |
| AC4-07 | `src/risk.rs`::`ac4_07_symlinks_are_flagged_with_their_target` |
| AC4-08 | `src/risk.rs`::`ac4_08_permissions_and_inline_shell_are_flagged` |
| AC4-09 | `src/risk.rs`::`ac4_09_manifests_are_flagged` |
| AC4-10 | `src/risk.rs`::`ac4_10_nothing_risky_has_no_flags` |
| AC4-11 | `tests/quarantine.rs`::`ac4_11_update_holds_instead_of_installing` |
| AC4-12 | `tests/quarantine.rs`::`ac4_12_nothing_newer_holds_nothing` |
| AC4-13 | `tests/quarantine.rs`::`ac4_13_a_newer_commit_replaces_the_held_one` |
| AC4-14 | `tests/quarantine.rs`::`ac4_14_update_rev_holds_the_named_commit` |
| AC4-15 | `tests/quarantine.rs`::`ac4_15_update_check_says_what_is_held_or_rejected` |
| AC4-16 | `tests/quarantine.rs`::`ac4_16_sync_never_installs_a_candidate` |
| AC4-17 | `tests/quarantine.rs`::`ac4_17_refresh_holds_instead_of_copying` |
| AC4-18 | `tests/quarantine.rs`::`ac4_18_refusals_stay_and_held_items_are_known` |
| AC4-19 | `tests/quarantine.rs`::`ac4_19_review_lists_what_is_held` |
| AC4-20 | `tests/quarantine.rs`::`ac4_20_reviewing_a_source_plugin` |
| AC4-21 | `tests/quarantine.rs`::`ac4_21_reviewing_a_skill` |
| AC4-22 | `tests/quarantine.rs`::`ac4_22_review_json` |
| AC4-23 | `tests/quarantine.rs`::`ac4_23_approving_a_source_plugin` |
| AC4-24 | `tests/quarantine.rs`::`ac4_24_approving_a_skill_and_an_agent` |
| AC4-25 | `tests/quarantine.rs`::`ac4_25_what_is_approved_is_what_was_reviewed` |
| AC4-26 | `tests/quarantine.rs`::`ac4_26_several_at_once_all_and_dry_run` |
| AC4-27 | `tests/quarantine.rs`::`ac4_27_rejecting` |
| AC4-28 | `tests/quarantine.rs`::`ac4_28_removing_an_item_drops_its_candidate` |
| AC4-29 | `tests/quarantine.rs`::`ac4_29_held_candidates_show_in_status`<br>`tests/quarantine.rs`::`ac4_29_status_lists_candidates_without_git` |
| AC4-30 | `tests/quarantine.rs`::`ac4_30_list_plugins_are_unreviewed` |
| AC4-31 | `tests/quarantine.rs`::`ac4_31_origin_changed_leaves_out_what_is_held_or_rejected` |
| AC4-32 | `tests/docs.rs`::`ac4_32_docs_cover_quarantine` |
| AC4-33 | `tests/docs.rs`::`ac3_26_ac4_33_earlier_suites_are_still_complete`<br>`tests/docs.rs`::`ac_35_ac2_23_ac3_28_ac4_33_every_criterion_and_edge_case_has_a_named_test` |
| EC4-01 | `tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_approve_arguments`<br>`tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_reject_arguments_and_dry_run`<br>`tests/quarantine.rs`::`ec4_01_update_with_no_sources` |
| EC4-02 | `tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_approve_arguments`<br>`tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_reject_arguments_and_dry_run`<br>`tests/quarantine.rs`::`ec4_02_ec4_03_review_names` |
| EC4-03 | `tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_approve_arguments`<br>`tests/quarantine.rs`::`ec4_01_ec4_02_ec4_03_reject_arguments_and_dry_run`<br>`tests/quarantine.rs`::`ec4_02_ec4_03_review_names` |
| EC4-04 | `src/state.rs`::`ec4_04_a_malformed_quarantine_entry_is_corrupt_state` |
| EC4-05 | `tests/quarantine.rs`::`ec4_05_update_rev_at_the_pin_is_up_to_date` |
| EC4-06 | `tests/quarantine.rs`::`ec4_06_upstream_going_back_to_the_pin_keeps_the_candidate` |
| EC4-07 | `tests/quarantine.rs`::`ec4_07_a_rejected_head_holds_nothing` |
| EC4-08 | `tests/quarantine.rs`::`ec4_08_review_of_an_older_commit_goes_back`<br>`tests/quarantine.rs`::`ec4_08_update_rev_to_an_older_commit_goes_back` |
| EC4-09 | `tests/quarantine.rs`::`ec4_09_review_covers_the_whole_repo` |
| EC4-10 | `tests/quarantine.rs`::`ec4_10_an_origin_back_at_its_adopted_content_keeps_the_held_copy` |
| EC4-11 | `tests/quarantine.rs`::`ec4_11_approving_one_leaves_the_others` |
| EC4-12 | `tests/quarantine.rs`::`ec4_12_a_tampered_snapshot_is_refused`<br>`tests/quarantine.rs`::`ec4_12_approve_refuses_a_tampered_snapshot` |
| EC4-13 | `tests/quarantine.rs`::`ec4_13_a_missing_held_commit_is_refused`<br>`tests/quarantine.rs`::`ec4_13_approve_refuses_a_missing_commit` |
| EC4-14 | `tests/quarantine.rs`::`ec4_14_approve_refuses_when_the_pin_changed_by_hand`<br>`tests/quarantine.rs`::`ec4_14_review_warns_when_the_pin_changed_by_hand` |
| EC4-15 | `tests/quarantine.rs`::`ec4_15_approve_refuses_a_source_copy_edited_since_it_was_held` |
| EC4-16 | `tests/quarantine.rs`::`ec4_16_approve_refuses_a_dirty_clone` |
| EC4-17 | `tests/quarantine.rs`::`ec4_17_an_unreachable_upstream_holds_nothing_and_others_continue` |
| EC4-18 | `src/commands/quarantine.rs`::`ec4_18_a_bad_snapshot_copy_holds_nothing_and_leaves_nothing` |
| EC4-19 | `tests/quarantine.rs`::`ec4_19_a_sync_failure_after_approve_keeps_the_approval` |
| EC4-20 | `tests/quarantine.rs`::`ec4_20_reject_and_approve_wait_for_the_lock`<br>`tests/quarantine.rs`::`ec4_20_update_waits_for_the_lock_but_check_does_not` |
| EC4-21 | `src/risk.rs`::`ec4_21_a_symlink_leaving_a_skill_is_flagged_outside`<br>`tests/quarantine.rs`::`ec4_21_a_symlink_leaving_a_held_skill_stays_a_symlink` |
| EC4-22 | `src/review.rs`::`ec4_22_control_characters_are_escaped`<br>`tests/quarantine.rs`::`ec4_22_review_escapes_control_characters` |
| EC4-23 | `src/risk.rs`::`ec4_23_dot_dot_and_absolute_targets_are_only_reported` |

## Test results

On the T9 branch (same code as `main`), verbatim:

```
test result: ok. 162 passed   (unit)
test result: ok. 55 passed    (cycle 1, tests/cli.rs)
test result: ok. 19 passed    (cycle 2, tests/codex.rs)
test result: ok. 8 passed     (docs and traceability)
test result: ok. 51 passed    (cycle 4, tests/quarantine.rs)
test result: ok. 17 passed    (cycle 3, tests/source_commands.rs)
test result: ok. 17 passed    (cycle 3, tests/sources.rs)
test result: ok. 16 passed    (cycle 2, tests/takeover.rs)
TOTAL lines 15675, missed 805, cover 94.86%   (gate: 90%)
```

345 tests. **Pre-existing flake:** `apply::tests::ec_11_marketplace_name_mismatch_skips_its_plugins` (cycle 1) failed
once on the release PR with `Text file busy (os error 26)` and passed on rerun. It's a Linux race in the test setup
(a fake script written while another test thread forks), not cycle 4 code. Follow-up below.

## Static checks

`just check` on every push: `cargo fmt --check`, `clippy -D warnings`, `cargo llvm-cov` ≥ 90% lines, `typos`,
`actionlint`, `zizmor`, `shellcheck`, `gitleaks`. All green on `main`.

## Security review

This cycle is a security feature, so the review looks at how it could be bypassed.
- **Nothing held reaches a harness without `approve`.** `update` and `refresh` never move a pin, change a source copy,
  commit, sync, or call `claude`/`codex` (AC4-11, AC4-17). `sync` installs only the pin (AC4-16).
- **What's approved is what was reviewed.** A held commit is immutable and pinned by a ref that survives force-pushes
  and `gc` (AC4-03). A held copy is hash-checked before `review` and `approve` (EC4-12). A hand-moved pin or an
  edited source copy is refused (EC4-14, EC4-15). Upstream moving on after review is ignored (AC4-25).
- **Held content stays out of history.** Snapshots live in the git-ignored `.state/` (AC4-02). Symlinks are copied as
  links, never followed (EC4-21).
- **Terminal injection.** File names, commit subjects and authors are escaped in text output (EC4-22). The diff keeps
  only tabs and newlines.
- **No code from a candidate runs.** `review` reads blobs with `git cat-file` and files with `read`; it never checks
  out a held commit or executes anything.
- **Limits, by design:** flags are pattern matches, not a scanner. Plain-text skill instructions can still steer an
  agent; list plugins are only labelled `unreviewed`. The explanation page says so.
- **Blast radius and reversibility:** each approval is one commit in `~/.colctl`; `git revert` + `sync` undoes it.
  `reject` touches only `.state/`.

## Observations from the smoke test (not blocking)

- **Noisy flags on real releases.** One superpowers release gave 46 flags, because a changed executable script gets both
  `executable` and `script`, and test scripts count too. Grouping per file, or separating test folders, would help;
  the cycle 6 TUI is a natural place.
- **Column alignment** in `review`'s flags breaks for paths longer than 32 characters.
- **`origin-changed` lines in `status`** print the origin's absolute path rather than `~/…` (pre-existing).
- **Manifests of other harnesses** (`.cursor-plugin/`, `.opencode/`, …) aren't flagged `manifest`, by the spec; their
  scripts are still flagged.

## Follow-ups

- Fix the `ETXTBSY` flake in `apply`'s test setup: retry the first spawn of a freshly written fake script.
- Consider the flag-noise and alignment points above in cycle 6 (TUI) or a small HIL cycle.

## Autonomy gate: what was not done

- colctl 2.0 has **not** been run on your real profiles; the smoke test used a copy.
- The release PR and the typos fix were merged by me under the policy; nothing else outside the repo was changed.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.0.0`, then `colctl --version`
- [ ] `colctl status`: should be `in sync`, with no list plugins (yours are all source plugins now)
- [ ] `colctl update --check`, then `colctl update superpowers --rev v7.0.1` and `colctl review superpowers` to see a
      real review; then `colctl reject superpowers` (nothing gets installed)
- [ ] Optional: when twg updates its skills, `colctl refresh`, `colctl review`, `colctl approve …`
- [ ] Decide the disposition
