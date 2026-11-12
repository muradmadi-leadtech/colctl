# Cycle review: plugins-from-source

_Cycle 3 · Evaluate · 2026-11-12_

| | |
|---|---|
| **Cycle** | `20261112-plugins-from-source` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner. Generate ran **autonomously** under the policy in `.forge/context.md`, extended to this cycle |
| **Intent** | [PRD](../prd/plugins-from-source.md): install plugins once from their author's repo into every Claude and Codex profile, bypassing aggregators, with explicit updates |
| **Scope delivered** | Everything in the PRD's "In" list. Quarantine (cycle 4) and converting packaging between tools (later) were not built |
| **Released** | [v1.2.1](https://github.com/muradmadi-leadtech/colctl/releases/tag/v1.2.1) after the revise (first [v1.2.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v1.2.0): both archives attested; `mise exec github:muradmadi-leadtech/colctl@1.2.0 -- colctl --version` prints `colctl 1.2.0`) |
| **PRs** | #23 plan · #24 T1 · #25 T2 · #27 T3 · #29 T4 · #30 T5+T6 · #31 T7 · #32 T8 · #33 fix · #26 release |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on copies |
| **Disposition** | First evaluation: **Revise**. After the revise: **Accept**, signed off by the owner on 2026-11-12 |

## Your story, on copies of your real profiles

Run with the v1.2 binary, the real `claude`, `codex` and `git`, and the real `github.com/obra/superpowers`:

1. **`status`** reported `superpowers@anthropic-plugin-directory` in both Claude profiles as `from-aggregator`, with
   the hint `source: https://github.com/obra/superpowers; colctl adopt …`.
2. **`adopt`** cloned the repo once into `~/.colctl/plugins/superpowers` at **`8ca22db`**, the commit the plugin
   directory had installed, and added the `[[source]]` with `replaces`. Then it:
   - uninstalled the directory copy in both Claude profiles;
   - installed `superpowers@superpowers-dev` from the clone in **both Claude and both Codex profiles**;
   - ended `in sync`.
3. **`update --check`** reported `superpowers: 8ca22db → 22f0b46 (4 new commits)`. **`update`** committed the new pin, and
   every profile went from **6.4.2 to 7.0.2**.
4. **It found a real bug** (fixed in #33). Your real kilo profile has superpowers installed twice, in **user** and **project**
   scope (`projectPath: ~`, from a session started in your home folder):
   - colctl removed the user copy, then kept retrying the project copy, which Claude refuses.
   - Now colctl only counts user-scope installs. Project and local installs belong to their project.
   - A pending replacement also now shows in `status`; before the fix, `status` wrongly said `in sync`.
   - After the fix, `sync` and `status` were clean on the same copies.

## Strict TDD: the evidence

Each PR pushed its RED commit (tests and stubs) first, and CI's `check` failed on it:

| PR | Task | RED run |
|---|---|---|
| #24 | T1 config | [43219736701](https://github.com/muradmadi-leadtech/colctl/actions/runs/43219736701) |
| #25 | T2 vault | [43220090160](https://github.com/muradmadi-leadtech/colctl/actions/runs/43220090160) |
| #27 | T3 packaging | [43220402328](https://github.com/muradmadi-leadtech/colctl/actions/runs/43220402328) |
| #29 | T4 origin | [43222046724](https://github.com/muradmadi-leadtech/colctl/actions/runs/43222046724), see below |
| #30 | T5+T6 sync | [43222622550](https://github.com/muradmadi-leadtech/colctl/actions/runs/43222622550) |
| #31 | T7 commands | [43223919826](https://github.com/muradmadi-leadtech/colctl/actions/runs/43223919826) |
| #32 | T8 docs | [43225145474](https://github.com/muradmadi-leadtech/colctl/actions/runs/43225145474) |
| #33 | smoke fix | [43226130401](https://github.com/muradmadi-leadtech/colctl/actions/runs/43226130401) |

**Where it slipped, stated plainly:**
- **T4.** #28 conflicted with #27, so CI never ran on it. Force-pushing was blocked, so I moved the same commits to a
  new branch (#29). My conflict resolution left a stray marker in `lib.rs`, which I fixed in a follow-up RED
  commit. That commit's CI run then failed at **format-check**, before the tests ran. The tests did fail locally,
  with all 6 failing, but CI doesn't show it for this task.
- **Some tests passed from the start because they guard existing behaviour:** `ac3_02` (old configs still load)
  and `ec3_17` (old state still loads).
- **Some test fixtures were corrected during GREEN:** the fake CLIs' local-marketplace support (my first edit
  silently didn't apply after `cargo fmt`), the `.git` URL stand-in for GitHub, and `ec3_18`'s scenario.
- **#27 needed a style fix after its GREEN push,** because my command pushed even though clippy had failed. After
  that I changed my routine so the gate runs first and the push only happens if it passes.

## What changed

45 files changed (+4,740 −70) since cycle 2, excluding `Cargo.lock` and `CHANGELOG.md`:
- **New modules:** `vault`, `packaging`, `origin`.
- **Grown:** `config`, `plan`, `apply`, `tool`, `commands`, `cli`, `adapter`.
- **New tests:** `tests/sources.rs`, `tests/source_commands.rs`; the fakes learned local marketplaces and updates.
- **Docs:** eleven reference, how-to and explanation pages.

Six spec changes found while building are recorded in `interfaces.md` under "Changes made during Generate". The
main ones:
- an optional `path` key;
- **re-pointing a marketplace when it is the plugin's own repo served remotely**, for example Codex's
  `superpowers-dev`. This goes slightly beyond "keep the marketplace": the old remote marketplace is removed, and
  the same name then points at colctl's clone;
- reading `remote.origin.url` from git config, because `git remote get-url` applies `insteadOf` rewrites.

## AC → test traceability

Every AC3 and EC3 item has a named test, and CI checks this together with cycles 1 and 2.

| ID | Tests |
|---|---|
| AC3-01 | `src/config.rs`::`ac3_01_invalid_sources_are_rejected_with_the_line`<br>`src/config.rs`::`ac3_01_sources_load` |
| AC3-02 | `src/config.rs`::`ac3_02_older_configs_have_no_sources` |
| AC3-03 | `src/config.rs`::`ac3_03_sources_can_be_excluded_per_harness`<br>`tests/sources.rs`::`ac3_03_a_source_can_be_excluded_from_a_harness` |
| AC3-04 | `src/vault.rs`::`ac3_04_a_missing_clone_is_made_at_the_pin`<br>`tests/sources.rs`::`ac3_04_sync_clones_at_the_pin` |
| AC3-05 | `src/vault.rs`::`ac3_05_a_moved_clone_goes_back_to_the_pin_and_fetches_new_commits`<br>`tests/sources.rs`::`ac3_05_a_moved_pin_moves_the_clone` |
| AC3-06 | `src/vault.rs`::`ac3_06_a_dirty_clone_is_never_touched`<br>`tests/sources.rs`::`ac3_06_a_dirty_clone_blocks_its_source` |
| AC3-07 | `src/packaging.rs`::`ac3_07_the_repo_manifests_decide_the_ids`<br>`tests/sources.rs`::`ac3_07_ac3_10_installs_from_the_clone_with_the_repo_ids` |
| AC3-08 | `src/packaging.rs`::`ac3_08_plugin_only_repos_get_a_colctl_manifest`<br>`tests/sources.rs`::`ac3_08_plugin_only_repos_install_through_a_colctl_manifest` |
| AC3-09 | `src/packaging.rs`::`ac3_09_a_one_tool_repo_is_unsupported_for_the_other`<br>`tests/sources.rs`::`ac3_09_unsupported_tools_are_visible_but_not_drift` |
| AC3-10 | `tests/sources.rs`::`ac3_07_ac3_10_installs_from_the_clone_with_the_repo_ids` |
| AC3-11 | `tests/sources.rs`::`ac3_11_a_moved_pin_updates_every_harness` |
| AC3-12 | `src/adapter/claude.rs`::`ac3_12_only_user_scope_installs_count`<br>`tests/sources.rs`::`ac3_12_ec3_10_replaces_uninstalls_the_aggregator_copy` |
| AC3-13 | `tests/source_commands.rs`::`ac3_13_rm_source` |
| AC3-14 | `tests/source_commands.rs`::`ac3_14_update_check_reports_and_changes_nothing` |
| AC3-15 | `tests/source_commands.rs`::`ac3_15_update_moves_pins_and_every_harness` |
| AC3-16 | `tests/source_commands.rs`::`ac3_16_update_rev` |
| AC3-17 | `tests/source_commands.rs`::`ac3_17_add_source`<br>`tests/source_commands.rs`::`ac3_17_ec3_04_add_source_without_packaging_is_refused` |
| AC3-18 | `src/origin.rs`::`ac3_18_a_plugin_directory_plugin`<br>`tests/source_commands.rs`::`ac3_18_adopt_a_plugin_directory_plugin_moves_it_to_its_source` |
| AC3-19 | `src/origin.rs`::`ac3_19_a_marketplace_plugin_inside_the_marketplace_repo`<br>`src/origin.rs`::`ac3_19_a_marketplace_plugin_pointing_at_another_repo`<br>`tests/source_commands.rs`::`ac3_19_adopt_a_marketplace_plugin_from_inside_the_marketplace_repo` |
| AC3-20 | `src/origin.rs`::`ac3_20_a_codex_plugin`<br>`tests/source_commands.rs`::`ac3_20_adopt_a_codex_plugin` |
| AC3-21 | `tests/source_commands.rs`::`ac3_21_the_same_source_from_another_aggregator_joins_replaces` |
| AC3-22 | `tests/source_commands.rs`::`ac3_22_no_source_found_falls_back_to_the_list` |
| AC3-23 | `tests/sources.rs`::`ac3_23_plugins_from_an_aggregator_are_flagged` |
| AC3-24 | `src/plan.rs`::`ac3_24_a_pending_replacement_shows_in_status`<br>`tests/sources.rs`::`ac3_24_status_lists_sources` |
| AC3-25 | `src/vault.rs`::`ec3_16_ac3_25_a_cloned_pin_needs_no_upstream`<br>`tests/sources.rs`::`ac3_25_status_and_sync_need_no_upstream` |
| AC3-26 | `tests/docs.rs`::`ac3_26_earlier_suites_are_still_complete` |
| AC3-27 | `tests/docs.rs`::`ac3_27_docs_cover_source_plugins` |
| AC3-28 | `tests/docs.rs`::`ac_35_ac2_23_ac3_28_every_criterion_and_edge_case_has_a_named_test` |
| EC3-01 | `tests/sources.rs`::`ec3_01_no_sources_means_nothing_new` |
| EC3-02 | `src/vault.rs`::`ec3_02_a_rev_that_does_not_exist` |
| EC3-03 | `src/vault.rs`::`ec3_03_an_unreachable_repo_leaves_no_half_clone` |
| EC3-04 | `src/packaging.rs`::`ec3_04_no_packaging_at_all`<br>`tests/source_commands.rs`::`ac3_17_ec3_04_add_source_without_packaging_is_refused` |
| EC3-05 | `src/packaging.rs`::`ec3_05_the_plugin_is_picked_from_several` |
| EC3-06 | `src/packaging.rs`::`ec3_06_a_broken_manifest_only_affects_its_tool` |
| EC3-07 | `tests/sources.rs`::`ec3_07_a_marketplace_name_taken_by_another_origin` |
| EC3-08 | `src/vault.rs`::`ec3_08_a_folder_that_is_not_our_clone_is_left_alone` |
| EC3-09 | `src/config.rs`::`ec3_09_reserved_and_invalid_source_names` |
| EC3-10 | `tests/sources.rs`::`ac3_12_ec3_10_replaces_uninstalls_the_aggregator_copy` |
| EC3-11 | `src/origin.rs`::`ec3_11_a_directory_binding_with_a_subpath`<br>`src/packaging.rs`::`ec3_11_a_plugin_in_a_subfolder` |
| EC3-12 | `tests/sources.rs`::`ec3_12_a_source_id_that_is_also_a_list_plugin` |
| EC3-13 | `tests/sources.rs`::`ec3_13_a_failed_update_keeps_the_old_rev` |
| EC3-14 | `src/vault.rs`::`ec3_14_a_failed_checkout_leaves_the_clone_where_it_was` |
| EC3-15 | `tests/source_commands.rs`::`ec3_15_update_cannot_reach_upstream` |
| EC3-16 | `src/vault.rs`::`ec3_16_ac3_25_a_cloned_pin_needs_no_upstream` |
| EC3-17 | `src/state.rs`::`ec3_17_a_1_1_state_file_loads_and_gains_sources` |
| EC3-18 | `src/origin.rs`::`ec3_18_a_local_codex_marketplace_without_a_known_commit`<br>`tests/source_commands.rs`::`ec3_18_unknown_installed_commit_pins_the_head` |

## Test results

On `main` after v1.2.0, verbatim:

```
test result: ok. 146 passed   (unit)
test result: ok. 55 passed    (cycle 1, tests/cli.rs)
test result: ok. 19 passed    (cycle 2, tests/codex.rs)
test result: ok. 16 passed    (cycle 2, tests/takeover.rs)
test result: ok. 16 passed    (cycle 3, tests/sources.rs)
test result: ok. 13 passed    (cycle 3, tests/source_commands.rs)
test result: ok. 7 passed     (docs and traceability)
TOTAL lines 6983, missed 298, cover 95.73%   (gate: 90%)
```

## Security review

- **Code execution.** colctl itself never runs plugin code. It clones the declared repo at a pinned commit, and only
  `update` moves that commit. Plugins can still run code through hooks once a tool loads them; cycle 4's quarantine
  is meant to add a review step there.
- **Integrity.** A clone with local changes, or whose origin isn't the declared repo, is never touched and blocks
  installs. A pin that's missing upstream fails visibly.
- **Network.** Only `update`, `adopt`, `add source` and a first clone contact upstream; `status` and `sync` are
  offline (tested by AC3-25).
- **Scope of changes.** colctl uninstalls only ids listed in `replaces` and plugins it installed. Aggregator
  marketplaces stay, except a marketplace that is the plugin's own repo, which is re-pointed at the clone.
  Project-scope installs are left alone.

## Autonomy gate: what was not done

- colctl 1.2 has **not** been run on your real profiles. Your real kilo profile still has the project-scope superpowers
  install, which colctl will now leave alone.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@1.2.0`
- [ ] `colctl status`: superpowers should show `from-aggregator` in both Claude profiles
- [ ] `colctl adopt`, then `colctl update --check` and `colctl update` (6.4.2 → 7.0.2), then open each harness and check
      superpowers is there
- [ ] Decide the disposition

## Revise (after the owner's first evaluation)

**What the owner found.** On the real profiles, `colctl status` showed superpowers as `ok`. It had been adopted into
`[plugins]` back in cycle 1, so it was never `from-aggregator`, and `adopt` refused it ("not a foreign item"). The
checklist's step 2 couldn't happen. Disposition: **Revise**, back to Refine.

**Refine:** AC3-29, AC3-30 and EC3-19 (in [acceptance-criteria.md](acceptance-criteria.md) and
[edge-cases.md](edge-cases.md)).
- **AC3-29:** `adopt` converts a list plugin whose source is known in one step: one commit, one sync, and the old
  copy uninstalled once per harness.
- **AC3-30:** `status` hints `source available: …` without changing the exit code.
- **EC3-19:** without a known source, nothing changes.

**Generate:**

| PR | What | RED run |
|---|---|---|
| #35 | AC3-29, AC3-30, EC3-19 (+ docs) | [43259880032](https://github.com/muradmadi-leadtech/colctl/actions/runs/43259880032) |
| #37 | The first sync of a source made by colctl 1.1 left `.gitignore` uncommitted (found by the revise smoke test) | Pushed GREEN while the RED run was pending, and CI cancels superseded PR runs, so this RED was verified **locally only** (1 failed) |

`ec3_19`'s first version left out a `sync` step; it was fixed in the GREEN commit.

**Smoke test, on a fresh copy of the owner's actual `~/.colctl` and profiles** (real `claude`, `codex`, `git` and
GitHub):
1. `status` showed `ok … → source available: https://github.com/obra/superpowers; colctl adopt claude plugin …` in both
   Claude profiles.
2. `adopt claude plugin superpowers@anthropic-plugin-directory` was **one commit**: the list entry was removed and
   the `[[source]]` added at `8ca22db`.
3. One sync then ran in every profile:
   - native: uninstalled the old copy, then installed `superpowers@superpowers-dev`;
   - kilo-claude, where colctl had installed it: forgot it, uninstalled it once, then installed from source;
   - both Codex profiles: installed from source.
4. `update`: `8ca22db → 22f0b46 (4 new commits)`, 6.4.2 → 7.0.2. Then `status` was **in sync** with nothing uncommitted.

**Tests on `main` after v1.2.1:** every suite passes. That's 233 tests (146 unit + 87 end-to-end across six suites).
Line coverage is 95.8%, and traceability now covers AC3-01…30 and EC3-01…19.

### Reviewer checklist (revised)

- [ ] `mise use -g github:muradmadi-leadtech/colctl@1.2.1`
- [ ] `colctl status`: superpowers shows `ok … source available: …`
- [ ] `colctl adopt claude plugin superpowers@anthropic-plugin-directory`, then `colctl update` (6.4.2 → 7.0.2), then
      open each harness and check superpowers is there
- [ ] Decide the disposition
