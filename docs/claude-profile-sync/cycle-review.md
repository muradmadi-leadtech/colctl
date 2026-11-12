# Cycle review: claude-profile-sync

_Cycle 1 · Evaluate · 2026-11-12_

| | |
|---|---|
| **Cycle** | `20261112-claude-profile-sync` (FORGE, full mode) |
| **Run mode** | Generate and the release ran **autonomously** under the policy in `.forge/context.md`; Focus, Orchestrate and Refine were confirmed with the owner in conversation |
| **Intent** | [PRD](../prd/claude-profile-sync.md): one source of truth in `~/.colctl` kept in step across every Claude Code profile |
| **Scope delivered** | Everything in the PRD's "In" list. Nothing from "Out" was built. |
| **Released** | [v1.0.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v1.0.0): `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu` archives, each attested |
| **Commits on `main`** | #1 `bb1de11`, #2 `c4952ac`, #4 `0b930bf`, #5 `97355a1`, #6 `0fd9fe0`, #7 `21d966c`, #3 `3fffcd8` (release); all pushed, all squash-merged after `check` passed |
| **Production impact** | None on this machine: the owner's real `~/.claude` and kilo profile were only read. colctl isn't installed globally yet. |
| **Disposition** | **Accept**, signed off by the owner on 2026-11-12 after testing it themselves. (Proposed was "Accept with issues"; the owner accepted with no issues.) |

## What changed

About 7,700 lines across 45 files, excluding `Cargo.lock`:

| Area | Files | Lines | Nature |
|---|---|---|---|
| Core | `config`, `ctx`, `error`, `fsx`, `git`, `runner`, `source`, `state` | ~2,000 | New. Config parsing and edits, safe filesystem primitives, process runner, git, state and lock |
| Sync engine | `adapter`, `plan`, `apply` | ~2,000 | New. Scan a Claude profile, pure planner, executor |
| Commands | `commands`, `cli`, `main` | ~1,200 | New. init, status, sync, add, rm, adopt |
| Tests | `tests/cli.rs`, `tests/docs.rs`, `tests/support`, `examples/fake_claude.rs` | ~1,800 | New. End-to-end tests against a temp home and a fake `claude`; repo checks |
| Docs | `README.md`, `docs/{tutorials,how-to,reference,explanation}` | ~560 | New. Diataxis user docs |
| Intent | `docs/claude-profile-sync/{interfaces,acceptance-criteria}.md` | +32 | Five spec changes found during Generate, recorded in "Changes made during Generate" |
| Tooling | `publish.yml`, `justfile`, `mise.toml`, `release-please-config.json` | ~70 | Binary releases, 90% coverage gate, cargo-llvm-cov pin, 1.0.0 pin (removed in #8) |

## AC → test traceability

Every acceptance criterion and edge case has at least one test named after it. `tests/docs.rs::ac_35_…` enforces
this in CI, so a gap can't appear silently. None are deferred.

| ID | Tests |
|---|---|
| AC-01 | `src/config.rs`::`ac_01_valid_config_loads_with_tilde_expanded` |
| AC-02 | `src/config.rs`::`ac_02_other_invalid_values`<br>`src/config.rs`::`ac_02_unknown_key_names_the_line`<br>`src/config.rs`::`ac_02_unknown_kind_duplicate_name_and_bad_plugin_id` |
| AC-03 | `src/ctx.rs`::`ac_03_colctl_home_overrides_source` |
| AC-04 | `tests/cli.rs`::`ac_04_first_init_creates_the_source` |
| AC-05 | `tests/cli.rs`::`ac_05_init_registers_the_harnesses_it_finds` |
| AC-06 | `src/config.rs`::`ac_06_append_harness_keeps_existing_lines`<br>`tests/cli.rs`::`ac_06_name_clash_gets_a_suffix`<br>`tests/cli.rs`::`ac_06_running_init_again_only_adds` |
| AC-07 | `tests/cli.rs`::`ac_07_init_dry_run_changes_nothing` |
| AC-08 | `tests/cli.rs`::`ac_08_status_is_zero_when_in_sync`<br>`tests/cli.rs`::`ac_08_status_reports_every_kind_of_drift` |
| AC-09 | `src/plan.rs`::`ac_09_missing_skill_is_linked`<br>`tests/cli.rs`::`ac_09_ac_10_ac_11_sync_links_skills_agents_and_imports_agents_md` |
| AC-10 | `src/plan.rs`::`ac_10_missing_agent_is_linked`<br>`tests/cli.rs`::`ac_09_ac_10_ac_11_sync_links_skills_agents_and_imports_agents_md` |
| AC-11 | `src/plan.rs`::`ac_11_claude_md_is_generated`<br>`tests/cli.rs`::`ac_09_ac_10_ac_11_sync_links_skills_agents_and_imports_agents_md`<br>`tests/cli.rs`::`ac_11_colctl_home_is_written_as_an_absolute_import` |
| AC-12 | `src/apply.rs`::`ac_12_ec_28_plugin_calls_use_the_harness_env`<br>`src/plan.rs`::`ac_12_marketplaces_then_plugins`<br>`tests/cli.rs`::`ac_12_plugins_are_installed_with_claudes_cli` |
| AC-13 | `src/plan.rs`::`ac_13_ac_14_broken_and_orphaned_links`<br>`tests/cli.rs`::`ac_13_broken_owned_links_are_repaired` |
| AC-14 | `src/plan.rs`::`ac_13_ac_14_broken_and_orphaned_links`<br>`tests/cli.rs`::`ac_14_removed_source_items_are_removed_from_harnesses` |
| AC-15 | `src/plan.rs`::`ac_15_ec_14_foreign_items_get_no_actions`<br>`tests/cli.rs`::`ac_15_foreign_items_are_never_touched` |
| AC-16 | `src/plan.rs`::`ac_16_removed_plugins_are_uninstalled_only_if_ours`<br>`tests/cli.rs`::`ac_16_ec_19_removed_plugins_are_uninstalled_only_where_colctl_installed_them` |
| AC-17 | `tests/cli.rs`::`ac_17_sync_commits_pending_source_changes_first` |
| AC-18 | `tests/cli.rs`::`ac_18_sync_can_target_one_harness` |
| AC-19 | `tests/cli.rs`::`ac_19_dry_run_never_changes_anything`<br>`tests/cli.rs`::`ac_19_dry_run_says_nothing_to_do` |
| AC-20 | `tests/cli.rs`::`ac_20_add_skill_from_a_folder`<br>`tests/cli.rs`::`ac_20_add_skill_from_a_git_url_with_subdir` |
| AC-21 | `tests/cli.rs`::`ac_21_add_agent` |
| AC-22 | `src/config.rs`::`ac_22_add_and_remove_plugin_keep_comments`<br>`tests/cli.rs`::`ac_22_add_plugin` |
| AC-23 | `tests/cli.rs`::`ac_23_rm_skill_agent_plugin` |
| AC-24 | `src/commands.rs`::`ac_24_adopt_entry_copies_then_links`<br>`tests/cli.rs`::`ac_24_adopt_a_skill` |
| AC-25 | `tests/cli.rs`::`ac_25_adopt_with_an_existing_source_item` |
| AC-26 | `tests/cli.rs`::`ac_26_adopt_claude_md` |
| AC-27 | `tests/cli.rs`::`ac_27_adopt_a_plugin_and_its_marketplace`<br>`tests/cli.rs`::`ac_27_unknown_marketplace_origin_is_refused` |
| AC-28 | `tests/cli.rs`::`ac_28_adopt_everything` |
| AC-29 | `src/state.rs`::`ac_29_second_lock_is_refused_while_held`<br>`tests/cli.rs`::`ac_29_one_colctl_at_a_time` |
| AC-30 | `src/apply.rs`::`ac_30_failures_carry_command_code_and_stderr`<br>`tests/cli.rs`::`ac_30_claude_failures_are_reported_in_full` |
| AC-31 | `src/apply.rs`::`ac_31_claude_missing_skips_plugin_actions`<br>`tests/cli.rs`::`ac_31_claude_missing_still_syncs_files` |
| AC-32 | `src/git.rs`::`ac_32_git_missing_is_a_usage_error`<br>`tests/cli.rs`::`ac_32_git_missing`<br>`tests/cli.rs`::`ac_32_status_works_without_git` |
| AC-33 | `tests/docs.rs`::`ac_33_publish_builds_attested_archives_for_both_targets` |
| AC-34 | `tests/docs.rs`::`ac_34_docs_exist_for_everything_shipped` |
| AC-35 | `tests/docs.rs`::`ac_35_every_criterion_and_edge_case_has_a_named_test` |
| EC-01 | `src/config.rs`::`ec_01_missing_config_says_run_init`<br>`tests/cli.rs`::`ec_01_commands_before_init_say_run_init` |
| EC-02 | `src/config.rs`::`ec_02_no_harnesses_is_valid`<br>`tests/cli.rs`::`ec_02_no_harnesses_registered` |
| EC-03 | `src/source.rs`::`ec_03_empty_source` |
| EC-04 | `src/plan.rs`::`ec_04_foreign_and_orphaned_claude_md` |
| EC-05 | `src/source.rs`::`ec_05_skill_without_skill_md_is_invalid` |
| EC-06 | `src/adapter.rs`::`ec_06_ec_07_missing_and_invalid_roots`<br>`src/plan.rs`::`ec_06_missing_harness_has_no_items`<br>`tests/cli.rs`::`ec_06_missing_harness_is_never_created` |
| EC-07 | `src/adapter.rs`::`ec_06_ec_07_missing_and_invalid_roots`<br>`tests/cli.rs`::`ec_07_harness_path_that_is_a_file_is_invalid` |
| EC-08 | `src/source.rs`::`ec_08_bad_names_and_shapes_are_invalid`<br>`tests/cli.rs`::`ec_08_add_skill_rejects_bad_names` |
| EC-09 | `src/config.rs`::`ec_09_relative_harness_path_is_rejected` |
| EC-10 | `src/config.rs`::`ec_10_two_harnesses_with_the_same_folder` |
| EC-11 | `src/apply.rs`::`ec_11_marketplace_name_mismatch_skips_its_plugins`<br>`tests/cli.rs`::`ec_11_marketplace_name_mismatch` |
| EC-12 | `src/adapter.rs`::`ec_12_missing_plugin_files_mean_nothing_installed`<br>`tests/cli.rs`::`ec_12_ec_13_plugin_files` |
| EC-13 | `src/adapter.rs`::`ec_13_unparsable_plugin_files_are_errors`<br>`src/plan.rs`::`ec_13_dangling_owned_link_without_source_is_removed`<br>`src/plan.rs`::`ec_13_unreadable_plugin_state_blocks_plugin_actions`<br>`tests/cli.rs`::`ec_12_ec_13_plugin_files` |
| EC-14 | `src/plan.rs`::`ac_15_ec_14_foreign_items_get_no_actions`<br>`tests/cli.rs`::`ec_14_ec_31_foreign_links_are_refused` |
| EC-15 | `src/plan.rs`::`ec_15_edited_claude_md_is_left_alone`<br>`tests/cli.rs`::`ec_15_edited_generated_claude_md_is_left_alone` |
| EC-16 | `src/adapter.rs`::`ec_16_synced_skills_are_ignored` |
| EC-17 | `src/plan.rs`::`ec_17_links_are_planned_even_without_folders`<br>`tests/cli.rs`::`ec_17_missing_skills_folder_is_created` |
| EC-18 | `src/plan.rs`::`ec_18_excluded_items_are_silent_and_owned_ones_removed`<br>`tests/cli.rs`::`ec_18_excluded_items` |
| EC-19 | `src/plan.rs`::`ec_19_installed_plugins_are_ok_and_not_claimed`<br>`tests/cli.rs`::`ac_16_ec_19_removed_plugins_are_uninstalled_only_where_colctl_installed_them` |
| EC-20 | `src/apply.rs`::`ec_20_native_profile_runs_without_claude_config_dir`<br>`tests/cli.rs`::`ec_20_ec_28_each_profile_gets_exactly_its_own_env` |
| EC-21 | `src/adapter.rs`::`ec_21_temps_are_reported`<br>`src/fsx.rs`::`ec_21_temps_are_found_and_removable`<br>`src/plan.rs`::`ec_21_temps_are_cleaned_first`<br>`tests/cli.rs`::`ec_21_leftover_temps_are_removed` |
| EC-22 | `src/apply.rs`::`ec_22_one_failing_item_does_not_stop_the_rest`<br>`tests/cli.rs`::`ec_22_one_unwritable_harness_does_not_stop_the_other` |
| EC-23 | `src/state.rs`::`ec_23_corrupt_state_is_an_error`<br>`tests/cli.rs`::`ec_23_corrupt_state` |
| EC-24 | `src/state.rs`::`ec_24_missing_state_is_empty` |
| EC-25 | `src/apply.rs`::`ec_25_slow_claude_times_out`<br>`src/runner.rs`::`ec_25_timeout_kills_the_child`<br>`tests/cli.rs`::`ec_25_hung_claude_is_killed` |
| EC-26 | `src/git.rs`::`ec_26_commits_without_a_configured_identity`<br>`tests/cli.rs`::`ec_26_commits_without_git_identity` |
| EC-27 | `src/state.rs`::`ec_27_stale_lock_is_reclaimed`<br>`tests/cli.rs`::`ec_27_stale_lock_is_reclaimed` |
| EC-28 | `src/apply.rs`::`ac_12_ec_28_plugin_calls_use_the_harness_env`<br>`tests/cli.rs`::`ec_20_ec_28_each_profile_gets_exactly_its_own_env` |
| EC-29 | `src/commands.rs`::`ec_29_bad_copy_leaves_the_original_untouched` |
| EC-30 | `tests/cli.rs`::`ec_30_adopt_a_plugin_directory_plugin` |
| EC-31 | `tests/cli.rs`::`ec_14_ec_31_foreign_links_are_refused` |

## Test results

From the CI run on the v1.0.0 commit (`just ci` on ubuntu-latest), verbatim:

```
test result: ok. 89 passed; 0 failed; 0 ignored   (unit, src/)
test result: ok. 55 passed; 0 failed; 0 ignored   (end-to-end, tests/cli.rs)
test result: ok. 3 passed; 0 failed; 0 ignored    (repo checks, tests/docs.rs)
TOTAL  lines 3703, missed 86, cover 97.68%        (gate: 90%)
```

No pre-existing failures; this is the first code in the repo.

**Smoke test (SC-6):** the release binary ran against a **copy** of the owner's two real profiles, with the real
`claude` CLI, in a scratch home. `init` registered both profiles. `status` showed `forge` and superpowers as
foreign and ignored `skills/synced`, `.trash` and `.DS_Store`. `adopt` adopted both, linked `forge` into the kilo
copy, and ran `claude plugin install superpowers@anthropic-plugin-directory` there successfully. `status` afterwards
exited 0. The real `claude` added only `enabledPlugins` to the kilo copy's `settings.json` and left kilo's keys
unchanged.

**Install (SC-7):** `mise exec github:muradmadi-leadtech/colctl@1.0.0 -- colctl version` printed `colctl 1.0.0`.
`gh attestation verify colctl-aarch64-apple-darwin.tar.gz --repo muradmadi-leadtech/colctl` exited 0.

## Static checks

`just check` passes on every merged PR: `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, tests with
the coverage gate, `typos`, `actionlint`, `zizmor` (online audits in CI), `shellcheck`, and `gitleaks` over every
commit. `unsafe_code` is forbidden crate-wide.

## Security review

- **Blast radius:** colctl writes only inside the source and, in a profile, symlinks under `skills/` and `agents/`
  plus its own `CLAUDE.md`. It never writes `settings.json` or Claude's plugin files, and never removes anything it
  can't prove it created (see [ownership](../explanation/ownership.md)). Edge-case tests EC-14, EC-15, EC-18, EC-19,
  EC-22 and EC-29 cover this.
- **Credentials:** none are handled. Calls to `claude` remove `ANTHROPIC_*`, `CLAUDE_CONFIG_DIR` and
  `CLAUDE_SECURESTORAGE_CONFIG_DIR` from the environment (EC-28), so a token in the calling shell never reaches
  another profile.
- **Command injection:** every external call uses an argument vector; there's no shell. Config values passed to
  `claude` are validated: names start with an alphanumeric, and sources are `github:o/r`, `http(s)://…` or an
  absolute path, so none can start with `-`. **Found and fixed in #7:** `add skill <url>` passed the URL to
  `git clone` without `--`. It wasn't exploitable (the target folder became the repository), and the URL comes from
  the user's own command line.
- **Reversibility:** every change to the source is a git commit; links can be removed; `.state` can be moved aside.
- **Supply chain:** release archives are built in CI and attested. Actions are pinned by SHA (sadr's defaults).

## Autonomy gate: what was not done

- **colctl was not run against the owner's real `~/.claude` or kilo profile**, and not installed globally. That
  changes real profiles, which the policy reserves for a human. It's the first item below.
- **No disposition recorded.** FORGE requires a human to sign off on an autonomous cycle.
- **Repo setting changed outside `sadr init`:** the default branch was renamed `master` → `main`, because sadr
  pushed `master` while its workflows only run on `main`. This is worth fixing in sadr (it should name the branch
  explicitly).

## Issues (why "Accept with issues")

1. **TDD was not strict test-by-test for the bulk of the code.** Tests were written with each module and every one
   is named after its criterion, but most weren't individually seen failing first. Only the #7 fix went through a
   recorded RED → GREEN. Coverage (97.7%) and full AC/EC traceability make up for much of that, but not all.
2. **`init` only knows kilo's macOS profile path**, as specified (AC-05). Linux kilo users add it by hand.
3. **No LICENSE file.** The repo is public but unlicensed, which means "all rights reserved". That's the owner's
   call.
4. **Release PRs need their workflows approved.** This was done here through the API
   (`gh api -X POST repos/…/actions/runs/<id>/approve`), which is easier than the button sadr's docs describe.

## Reviewer checklist

- [ ] Read the [tutorial](../tutorials/first-sync.md), then run it for real:
      `mise use -g github:muradmadi-leadtech/colctl && colctl init && colctl adopt --dry-run`
- [ ] Run `colctl adopt`, then start `claude` and `kilo-claude` and check both see `forge` and superpowers
- [ ] Decide on a LICENSE
- [ ] Decide the disposition (Accept / Accept with issues / Revise) and complete the cycle:
      `uv run ~/.claude/skills/forge/tools/forge_cycle.py complete 20261112-claude-profile-sync`
- [ ] Optionally fix sadr's default-branch bug upstream
