# Cycle review: codex-cli-support

_Cycle 2 · Evaluate · 2026-11-12_

| | |
|---|---|
| **Cycle** | `20261112-codex-cli-support` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner (Refine gained the "take over linked skills" section at the owner's request). Generate ran **autonomously** under the policy in `.forge/context.md`, extended to this cycle |
| **Intent** | [PRD](../prd/codex-cli-support.md): keep Codex CLI profiles and the shared `~/.agents` skills folder in step from `~/.colctl`, and take over skills that tools like twg link in |
| **Scope delivered** | Everything in the PRD's "In" list, plus AC2-24 to AC2-28. Nothing from "Out" was built |
| **Released** | [v1.1.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v1.1.0): both archives attested; `mise exec github:muradmadi-leadtech/colctl@1.1.0 -- colctl --version` prints `colctl 1.1.0` |
| **PRs** | #12 plan · #13 T1 · #14 T4 · #15 T3 · #17 T5 · #18 T2+T6 · #19 T7 · #20 T8 · #16 release. All squash-merged after `check` passed |
| **Production impact** | None on this machine. The owner's real profiles were only read; the smoke test ran on copies |
| **Disposition** | **Awaiting the owner** |

## Strict TDD: the evidence

Cycle 1 only had a recorded RED → GREEN for one fix. This cycle, every feature PR's **first pushed commit held only
tests (plus compiling stubs), and CI's `check` failed on it**; the second commit made it pass:

| PR | Task | RED run (failed as intended) |
|---|---|---|
| #14 | T4 convert | [43195711796](https://github.com/muradmadi-leadtech/colctl/actions/runs/43195711796) |
| #15 | T3 config | [43196030682](https://github.com/muradmadi-leadtech/colctl/actions/runs/43196030682) |
| #17 | T5 scan | [43196490652](https://github.com/muradmadi-leadtech/colctl/actions/runs/43196490652) |
| #18 | T2+T6 sync | [43197051535](https://github.com/muradmadi-leadtech/colctl/actions/runs/43197051535) |
| #19 | T7 commands | [43197807612](https://github.com/muradmadi-leadtech/colctl/actions/runs/43197807612) |
| #20 | T8 docs | [43198808158](https://github.com/muradmadi-leadtech/colctl/actions/runs/43198808158) |

Exceptions, stated plainly:
- **T1 (#13) was a refactor:** no new tests, with the existing suite as the safety net.
- **Two tests passed from the start because they guard behaviour that already existed:** `ec2_22` (a 1.0 state file
  still loads) and `ec2_02` (no source agents means no agent files).
- **One GREEN commit (#18) also fixed a bug in a test helper,** which wrote the `[codex]` tables twice.

## What changed

47 files changed (+4,555 −492) since cycle 1, excluding `Cargo.lock` and `CHANGELOG.md`:
- **New modules:** `adapter/{mod,codex,agents_dir}`, `tool`, `convert`.
- **Rewritten parts:** `plan`, `apply`, `commands` and `config`, reworked to handle each kind.
- **New command:** `refresh`.
- **New tests:** `examples/fake_codex.rs`, `tests/codex.rs`, `tests/takeover.rs`.
- **Docs:** eleven reference, how-to and explanation pages updated or added.
- **New dependency:** `yaml-rust2`, for agent frontmatter. `serde_yaml` is deprecated.

## AC → test traceability

Every AC2 and EC2 item has a test named after it, and CI enforces it for both cycles (`tests/docs.rs`). Nothing is
deferred.

| ID | Tests |
|---|---|
| AC2-01 | `src/config.rs`::`ac2_01_new_kinds_load` |
| AC2-02 | `src/config.rs`::`ac2_02_the_codex_plugin_list_is_separate` |
| AC2-03 | `src/config.rs`::`ac2_03_ec2_09_excludes_are_checked_per_kind` |
| AC2-04 | `tests/takeover.rs`::`ac2_04_init_finds_codex_profiles_and_the_agents_folder` |
| AC2-05 | `tests/codex.rs`::`ac2_05_agents_md_is_linked_and_unlinked` |
| AC2-06 | `tests/takeover.rs`::`ac2_06_adopt_a_codex_agents_md` |
| AC2-07 | `tests/codex.rs`::`ac2_07_skills_are_linked_into_the_shared_folder` |
| AC2-08 | `src/convert.rs`::`ac2_08_renders_the_generated_toml`<br>`tests/codex.rs`::`ac2_08_agents_become_codex_toml` |
| AC2-09 | `tests/codex.rs`::`ac2_09_generated_agents_follow_their_source` |
| AC2-10 | `tests/codex.rs`::`ac2_10_hand_edited_and_foreign_agent_toml_are_left_alone`<br>`tests/takeover.rs`::`ac2_10_foreign_codex_agent_toml_is_refused` |
| AC2-11 | `src/convert.rs`::`ac2_11_agents_that_cannot_be_converted`<br>`tests/codex.rs`::`ac2_11_unconvertible_agents_are_invalid_for_codex_only` |
| AC2-12 | `tests/codex.rs`::`ac2_12_codex_plugins_are_installed_with_codex` |
| AC2-13 | `tests/codex.rs`::`ac2_13_codex_plugins_are_removed_only_where_colctl_installed_them` |
| AC2-14 | `tests/codex.rs`::`ac2_14_ec2_15_ec2_20_each_codex_profile_gets_its_own_env` |
| AC2-15 | `tests/codex.rs`::`ac2_15_codex_failures_are_reported_in_full`<br>`tests/codex.rs`::`ac2_15_codex_missing_still_syncs_files` |
| AC2-16 | `src/adapter/codex.rs`::`ac2_16_plugin_state_comes_from_config_toml` |
| AC2-17 | `tests/takeover.rs`::`ac2_17_add_and_rm_a_codex_plugin` |
| AC2-18 | `tests/takeover.rs`::`ac2_18_adopt_a_codex_plugin` |
| AC2-19 | `tests/takeover.rs`::`ac2_19_adopt_everything_includes_the_new_kinds` |
| AC2-20 | `tests/codex.rs`::`ac2_20_status_json_shows_the_kinds` |
| AC2-21 | `tests/docs.rs`::`ac2_21_the_cycle_1_suite_is_still_complete` |
| AC2-22 | `tests/docs.rs`::`ac2_22_docs_cover_codex_and_the_agents_folder` |
| AC2-23 | `tests/docs.rs`::`ac_35_ac2_23_every_criterion_and_edge_case_has_a_named_test` |
| AC2-24 | `src/adapter/agents_dir.rs`::`ac2_24_skills_linked_from_another_tool_are_reported_with_their_target`<br>`tests/takeover.rs`::`ac2_24_links_into_other_folders_are_visible` |
| AC2-25 | `tests/takeover.rs`::`ac2_25_adopt_takes_the_target_over` |
| AC2-26 | `tests/takeover.rs`::`ac2_26_origin_changes_and_refresh` |
| AC2-27 | `tests/takeover.rs`::`ac2_27_the_tool_re_links_over_colctl` |
| AC2-28 | `tests/takeover.rs`::`ac2_28_what_adopt_still_refuses` |
| EC2-01 | `tests/codex.rs`::`ec2_01_ec2_03_missing_folders_are_created` |
| EC2-02 | `tests/codex.rs`::`ec2_02_no_source_agents_means_no_agent_files` |
| EC2-03 | `tests/codex.rs`::`ec2_01_ec2_03_missing_folders_are_created` |
| EC2-04 | `src/adapter/codex.rs`::`ec2_04_missing_config_means_nothing_installed` |
| EC2-05 | `src/convert.rs`::`ec2_05_invalid_yaml_reports_the_parse_error` |
| EC2-06 | `src/convert.rs`::`ec2_06_frontmatter_name_wins_over_the_file_name` |
| EC2-07 | `src/adapter/codex.rs`::`ec2_07_unparsable_config_is_an_error` |
| EC2-08 | `src/config.rs`::`ec2_08_codex_plugin_needs_a_declared_marketplace` |
| EC2-09 | `src/config.rs`::`ac2_03_ec2_09_excludes_are_checked_per_kind` |
| EC2-10 | `src/convert.rs`::`ec2_10_claude_only_keys_are_dropped` |
| EC2-11 | `src/convert.rs`::`ec2_11_awkward_text_round_trips_exactly` |
| EC2-12 | `src/adapter/codex.rs`::`ec2_12_agents_md_linked_elsewhere_is_foreign` |
| EC2-13 | `src/adapter/agents_dir.rs`::`ec2_13_a_real_folder_with_a_source_name_is_real` |
| EC2-14 | `tests/codex.rs`::`ec2_14_marketplace_that_names_itself_differently` |
| EC2-15 | `tests/codex.rs`::`ac2_14_ec2_15_ec2_20_each_codex_profile_gets_its_own_env` |
| EC2-16 | `src/adapter/codex.rs`::`ec2_16_disabled_plugins_count_as_installed` |
| EC2-17 | `tests/codex.rs`::`ec2_17_a_hand_made_toml_wins_over_a_source_agent_of_the_same_name` |
| EC2-18 | `tests/codex.rs`::`ec2_18_hung_codex_is_killed` |
| EC2-19 | `tests/codex.rs`::`ec2_19_one_unwritable_codex_profile_does_not_stop_the_other` |
| EC2-20 | `tests/codex.rs`::`ac2_14_ec2_15_ec2_20_each_codex_profile_gets_its_own_env` |
| EC2-21 | `src/adapter/codex.rs`::`ec2_21_formatting_and_order_do_not_matter` |
| EC2-22 | `src/state.rs`::`ec2_22_a_1_0_state_file_loads_unchanged` |
| EC2-23 | `tests/takeover.rs`::`ec2_23_ec2_27_chains_and_relative_links_resolve_to_the_final_folder` |
| EC2-24 | `tests/takeover.rs`::`ec2_24_origin_deleted_later` |
| EC2-25 | `tests/takeover.rs`::`ec2_25_one_origin_under_two_names` |
| EC2-26 | `tests/takeover.rs`::`ec2_26_origin_with_a_git_folder_is_copied_as_is` |
| EC2-27 | `tests/takeover.rs`::`ec2_23_ec2_27_chains_and_relative_links_resolve_to_the_final_folder` |
| EC2-28 | `tests/takeover.rs`::`ec2_28_refresh_without_an_origin` |

## Test results

On `main` after v1.1.0 (`just test`), verbatim:

```
test result: ok. 115 passed   (unit, src/)
test result: ok. 55 passed    (cycle 1 end-to-end, tests/cli.rs)
test result: ok. 19 passed    (Codex end-to-end, tests/codex.rs)
test result: ok. 5 passed     (docs and traceability, tests/docs.rs)
test result: ok. 16 passed    (commands and take-over, tests/takeover.rs)
TOTAL lines 4969, missed 160, cover 96.78%   (gate: 90%)
```

**Changes to cycle 1 tests (AC2-21):** `tests/cli.rs` keeps all 55 tests, and a CI guard checks that.
- `ec_14` changed as AC2-25 intends: a dangling link is still refused, with the new reason.
- The unit tests that build the internal `Action` and `Item` types were updated mechanically for the T1 refactor.
- The unit test `ac_02` used `"codex"` as its example of an unknown kind. It now uses `"cursor"`.

**Smoke test on copies of the owner's real profiles**, using the release binary and the real `claude` and `codex` CLIs:
1. `init` registered all five profiles. `status` showed the 14 twg skills as `linked-elsewhere`, with their targets.
2. `adopt` took over `forge`, the 14 twg skills and superpowers, ending `in sync` (exit 0):
   - 14 origins were recorded.
   - twg's folder was byte-identical afterwards.
   - `.skill-lock.json` was untouched.
3. A simulated twg update was reported as `origin-changed`, and `refresh` pulled it in as a commit.
4. `add agent` linked the agent into both Claude profiles and generated valid TOML in both Codex profiles.
5. `add plugin superpowers@superpowers-dev --codex` ran the real `codex plugin marketplace add` and `plugin add`.
   - In the kilo Codex copy, the real `codex` first failed with `No such file or directory`. My copy was missing
     kilo's `models.json`.
   - colctl reported the error in full and skipped the dependent install, as AC2-15 requires.
   - After copying the file, the next `sync` finished the job.

## Static checks

`just check` passed on every merged PR, covering fmt, clippy with `-D warnings` on all targets, tests with the
coverage gate, typos, actionlint, zizmor, shellcheck and gitleaks.

## Security review

- **Blast radius.** In a codex harness, colctl writes only the `AGENTS.md` symlink and `agents/*.toml` files it
  can prove it generated. In `~/.agents`, it writes only symlinks under `skills/`. Codex's `config.toml` is
  changed only by the `codex` CLI.
- **Taking over** copies from the tool's folder and never writes to it. The copy is staged, hash-checked and renamed
  into place before the link moves, and `refresh` refuses when both sides changed.
- **Credentials.** `codex` calls run with `CODEX_HOME` and every `OPENAI_*` variable removed, and only the
  harness's own `CODEX_HOME` set (tested by AC2-14 and EC2-20). The smoke test deliberately left out Codex
  `auth.json` files.
- **Injection.** Every call uses an argument vector, with no shell. Agent text reaches TOML only through
  `toml_edit`, and EC2-11 round-trips quotes, `"""` and `'''`.

## Autonomy gate: what was not done

- colctl 1.1 has **not** been run on your real profiles. Your `~/.colctl` from 1.0 is untouched.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@1.1.0` (or `mise upgrade`), then `colctl init` to register
      `codex`, `kilo-codex` and `agents`
- [ ] `colctl adopt --dry-run`, then `colctl adopt`, and check that the twg skills now live in `~/.colctl/skills`
- [ ] Start `codex` and `kilo-codex` and check they see `AGENTS.md`, the skills and any agents
- [ ] Decide the disposition
