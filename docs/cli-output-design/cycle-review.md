# Cycle review: CLI output design

_Cycle 6 · Evaluate · 2026-11-13_

| | |
|---|---|
| **Cycle** | `20261113-cli-output-design` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase (T0 added during Refine at the owner's request). Generate ran **autonomously** under the policy in `.forge/context.md` |
| **Intent** | [PRD](../prd/cli-output-design.md): one designed system for how colctl presents results, runs, errors and questions, so the CLI is pleasant on its own; `--json` and exit codes unchanged |
| **Scope delivered** | Everything in the PRD's "In" list, plus T0. Not built, as agreed: full-screen views, themes, translations, man pages |
| **Released** | [v2.1.1](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.1.1) and [v2.1.2](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.1.2) (T0, as patches), then [v2.2.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.2.0). All attested (`gh attestation verify` exit 0); `mise exec …@2.2.0 -- colctl --version` prints `colctl 2.2.0` |
| **PRs** | #65 plan · #66 T0 · #69 T0 fix · #68 T1 · #72 T2 · #73 T3 · #74 T4+T5 · #75 T6 · #76 T7 · #77 smoke fixes · #67, #70, #71 releases |
| **Production impact** | None on this machine. Real profiles were only read; every smoke test ran on a fresh copy |
| **Disposition** | _Yours to decide_ |

## T0: the databricks loop (found during Refine)

Your `colctl add source databricks@claude-plugins-official` never converged. databricks' own manifests give its plugin a
`git-subdir` source on GitHub at `main`, so Claude and Codex ignored the pinned clone, installed `main` (95beba3,
0.2.48) instead of your pin (c196426), and every sync "updated" it again. It also let unreviewed code past quarantine.

- **v2.1.1 (#66):** when a repo's manifest points outside the repo, colctl writes its own wrapper marketplace
  (`colctl-<name>`) pointing at the plugin's folder in the clone, per tool; existing installs are moved over; a source
  still at the wrong commit after updating is reported as a failure.
- **v2.1.2 (#69), found by the smoke test of 2.1.1:** Claude moved over, but Codex failed: it clones a wrapper's `url`
  source with git, and a plugin folder isn't a repo root. Codex's own catalog uses `{"source": "local", "path": …}`;
  colctl does too now. The fake `codex` was made to behave like the real one, which is what made this testable.
- **Verified on a fresh copy of your setup with 2.1.2:** the first sync moved databricks in all four profiles, the second
  did nothing, `status` was in sync, and both Codex profiles hold 0.2.28 (your pin) instead of 0.2.48.

**Your real machine still has the old databricks install.** The first `colctl sync` with 2.1.2 or later moves it.

## Your setup, in 2.2 (fresh copy, real `claude`, `codex`, `git`)

`status`, piped:
```
! 4 things need attention

  !  claude       ~/.claude                    16 ok · 1 missing
  ...
Needs attention
  !  claude       source  databricks  missing  → colctl sync
  ...
```
`sync` then moved databricks everywhere (`✓ Synced 5 profiles · 16 changes`, one block per profile: uninstalled
databricks, removed marketplace databricks-agent-skills, added marketplace colctl-databricks, installed databricks), and
a second `sync` said `✓ Everything was already in sync`. `status` afterwards: 7 lines, `✓ All 5 profiles in sync`.

**In a real pseudo-terminal** (Python `pty`, 100 columns): colour came on (green ✓, bold names, dim paths, cyan
commands) and the width was detected. `sync -v` drew a live line per profile (`⠋ kilo-claude  linking skill forge
1/1`), turned each into `✓ 1 change`, cleared them, then printed the result with every call and its duration.

## Strict TDD: the evidence

| PR | Task | RED run(s) |
|---|---|---|
| #66 | T0 | [43406755738](https://github.com/muradmadi-leadtech/colctl/actions/runs/43406755738) |
| #69 | T0 Codex fix | [43419389858](https://github.com/muradmadi-leadtech/colctl/actions/runs/43419389858) |
| #68 | T1 foundation | [43418362463](https://github.com/muradmadi-leadtech/colctl/actions/runs/43418362463) |
| #72 | T2 status | [43421486713](https://github.com/muradmadi-leadtech/colctl/actions/runs/43421486713) |
| #73 | T3 running commands | [43427274357](https://github.com/muradmadi-leadtech/colctl/actions/runs/43427274357) |
| #74 | T4 interaction + T5 one look | [43428174340](https://github.com/muradmadi-leadtech/colctl/actions/runs/43428174340), [43431035288](https://github.com/muradmadi-leadtech/colctl/actions/runs/43431035288) |
| #75 | T6 help | [43431965557](https://github.com/muradmadi-leadtech/colctl/actions/runs/43431965557) |
| #76 | T7 snapshots and docs | [43434300560](https://github.com/muradmadi-leadtech/colctl/actions/runs/43434300560) |
| #77 | smoke fixes | [43435430435](https://github.com/muradmadi-leadtech/colctl/actions/runs/43435430435) |

**Where it slipped, stated plainly:**
- **T4 and T5 shipped as one PR.** I didn't push T4's GREEN before starting T5 on the same files, and they couldn't be
  separated cleanly. Each kept its own RED commit, run in CI against the unchanged code, then one GREEN. I nearly did
  the same with T6; that time its GREEN went up separately, gated on its own.
- **I popped an unrelated old stash** while moving files between branches; git stopped on a conflict in a completed
  cycle file. I restored that file and left the old stash untouched. Since then a stash is only popped when the push
  made one.
- **The usage limit stopped T3 halfway;** the work was saved on a local branch and resumed from there.
- **My fixtures again hid real-world shapes:** the fake `codex` accepted any `url` source (fixed in #69), and test
  data used short ids (the long-id table problem was found by the smoke test, fixed in #77).

## What the checks found during Generate

| Found by | Problem | Fixed in |
|---|---|---|
| Smoke test of 2.1.1 | Codex wrappers used a `url` source Codex can't clone | #69 |
| Earlier tests | An early return in `refresh` skipped the lock (exit 3 → 0) | #74, before pushing |
| AC-30 (cycle 1) | The new run view showed only the first line of a tool's error | #73: shown in full |
| Width check in the snapshots | `status --all` headings, long item lines, and `marketplace list` at 60 columns ran past the width | #76 |
| Review of the output | Fixes, risk flags, outcomes, plugin ids were cut with "…" | #72–#76: never cut; they wrap or move to the next line |
| Smoke test | One long id cut every id; stacked outcomes left the profile line empty | #77 |

## Changes to the plan made during Generate

| What | Why |
|---|---|
| **Item lines** (`update`, `refresh`, `adopt`, `approve`, `reject`, `marketplace refresh`) keep each item's sentence under a verdict instead of becoming tables | The sentences carry facts the earlier suites check; a table would have dropped words |
| **Installed** is a leading `✓` column with the legend in the footer, not a trailing "✓ installed" | So 80 columns fit long ids |
| **Connectors** get `openai` and their marketplace as display defaults only | Their JSON stays unchanged (AC6-01) |
| **AC6-02**'s allow-list reached zero in T5, not T6 | All printing had moved by then |

## Earlier tests changed

Text assertions only; JSON goldens and every exit code are unchanged (AC6-01).
- **T2 (status):** `ac_04`, `ac_08` ×2, `ec_02`, `ec_07`, `ec_14_ec_31`, `ac2_24`, `ac2_26`, `ac3_09`, `ac3_23`, `ac3_30`,
  `ac4_29` ×2, `ac4_30`.
- **T3 (runs):** `ac_04`, `ac_06` ×2, `ac_07`, `ac_19` ×2, `ac_30`, `ac_31`, `ec_11`, `ac2_04`, `ac2_15` ×2, `ec2_14`,
  `ec3_18`, `ac4_25`, `ac4_26`, `ec4_01` ×2, `ac5_23`.
- **T4+T5:** `ec4_01`, `ac4_19`, `ac4_20`, `ac4_21`, `ec4_08`, `ac5_12`, `ac5_18`, `ac5_19`, `ac5_20`, `ac5_21`, `ec5_01`,
  `ec5_10`, `ec5_17`.
- **T7:** long messages that now wrap are compared as words in `cli.rs`, `quarantine.rs`, `source_commands.rs`,
  `takeover.rs`.

No test was removed or ignored. Tests compare words (`Env::words`) rather than column padding, since columns now size
to their content.

## What changed

`git diff --stat v2.1.0..v2.2.0 -- src tests examples Cargo.toml`: 89 files, +6,328 / −699 (including 49 snapshot files).

| File | Nature |
|---|---|
| `src/ui/` (new: term, style, render, progress, ask, mod) | The output layer: capabilities, tokens, components, live progress, questions, pager |
| `src/commands/status_view.rs`, `run_view.rs` (new) | Status and run results |
| `src/commands/*.rs` | Every command builds lines for `ctx.out`; no direct printing |
| `src/cli.rs` | Global flags, grouped help, examples, completions, error layout |
| `src/packaging.rs`, `plan.rs`, `apply.rs` | T0: wrapper marketplaces, migration, convergence; step events |
| New dependencies | console, indicatif, dialoguer (fuzzy-select), clap_complete; insta (tests) |

## AC → test traceability

Every AC6 and EC6 ID has at least one test named after it; `tests/docs.rs` checks this (33 + 22 IDs).

| ID | Test(s) |
|---|---|
| AC6-01 | `tests/output.rs`::`ac6_01_json_output_is_unchanged` |
| AC6-02 | `tests/output.rs`::`ac6_02_one_output_layer` |
| AC6-03 | `src/ui/term.rs`::`ac6_03_colour_policy` |
| AC6-04 | `src/ui/style.rs`::`ac6_04_symbols_carry_meaning_without_colour` |
| AC6-05 | `src/ui/render.rs`::`ac6_05_tables_never_wrap`<br>`src/ui/term.rs`::`ac6_05_ec6_01_width` |
| AC6-06 | `tests/output.rs`::`ac6_06_errors_say_what_and_what_to_do` |
| AC6-07 | `tests/output.rs`::`ac6_07_global_flags_work_anywhere` |
| AC6-08 | `tests/output.rs`::`ac6_08_all_in_sync_fits_in_eight_lines` |
| AC6-09 | `tests/output.rs`::`ac6_09_problems_only_each_with_its_fix` |
| AC6-10 | `tests/output.rs`::`ac6_10_all_lists_every_item` |
| AC6-11 | `tests/output.rs`::`ac6_11_an_unusable_profile_says_so_with_its_fix` |
| AC6-12 | `src/commands/run_view.rs`::`ac6_12_outcomes_grouped_and_counted`<br>`tests/output.rs`::`ac6_12_outcomes_not_calls` |
| AC6-13 | `tests/output.rs`::`ac6_13_informational_outcomes_do_not_contradict` |
| AC6-14 | `tests/output.rs`::`ac6_14_failures_say_what_failed_and_what_to_do` |
| AC6-15 | `src/ui/progress.rs`::`ac6_15_nothing_is_drawn_without_a_terminal`<br>`tests/output.rs`::`ac6_15_no_control_characters_when_piped` |
| AC6-16 | `tests/output.rs`::`ac6_16_verbose_and_quiet` |
| AC6-17 | `tests/output.rs`::`ac6_17_commands_lead_with_their_result` |
| AC6-18 | `src/ui/ask.rs`::`ac6_18_ec6_12_confirmation_rules`<br>`tests/output.rs`::`ac6_18_ec6_12_without_a_terminal_nothing_asks` |
| AC6-19 | `src/ui/ask.rs`::`ac6_19_ec6_13_pickers`<br>`tests/output.rs`::`ac6_19_ec6_05_without_a_terminal_a_missing_name_is_still_an_error` |
| AC6-20 | `src/ui/ask.rs`::`ac6_20_ec6_15_pager`<br>`tests/output.rs`::`ac6_20_without_a_terminal_long_output_is_not_paged` |
| AC6-21 | `tests/output.rs`::`ac6_21_update_and_refresh_lead_with_a_verdict` |
| AC6-22 | `tests/output.rs`::`ac6_22_review_puts_risk_first_and_colours_the_diff` |
| AC6-23 | `tests/output.rs`::`ac6_23_discovery_output` |
| AC6-24 | `tests/output.rs`::`ac6_24_every_error_has_a_hint` |
| AC6-25 | `src/cli.rs`::`ac6_25_the_grouped_help_lists_every_command`<br>`tests/output.rs`::`ac6_25_ac6_26_help_is_grouped_with_examples` |
| AC6-26 | `src/cli.rs`::`ac6_26_every_command_has_examples`<br>`tests/output.rs`::`ac6_25_ac6_26_help_is_grouped_with_examples` |
| AC6-27 | `tests/output.rs`::`ac6_27_completions` |
| AC6-28 | `tests/output.rs`::`ac6_28_snapshots_of_every_command` |
| AC6-29 | `tests/docs.rs`::`ac6_29_docs_cover_the_output_design` |
| AC6-30 | `tests/docs.rs`::`ac3_26_ac4_33_ac5_28_ac6_30_earlier_suites_are_still_complete`<br>`tests/docs.rs`::`ac_35_ac2_23_ac3_28_ac4_33_ac5_28_ac6_30_every_criterion_and_edge_case_has_a_named_test` |
| AC6-31 | `src/packaging.rs`::`ac6_31_a_manifest_pointing_outside_the_repo_is_wrapped`<br>`tests/install_from_clone.rs`::`ac6_31_a_repo_manifest_pointing_at_github_installs_from_the_clone` |
| AC6-32 | `tests/install_from_clone.rs`::`ac6_32_installs_made_from_the_repo_manifest_move_to_the_clone` |
| AC6-33 | `tests/install_from_clone.rs`::`ac6_33_a_source_that_does_not_converge_fails` |
| EC6-01 | `src/ui/term.rs`::`ac6_05_ec6_01_width` |
| EC6-02 | `src/ui/term.rs`::`ec6_02_ec6_03_empty_no_color_and_clicolor_force_zero` |
| EC6-03 | `src/ui/term.rs`::`ec6_02_ec6_03_empty_no_color_and_clicolor_force_zero` |
| EC6-04 | `src/ui/term.rs`::`ec6_04_ec6_05_ec6_06_ec6_16_who_may_ask_and_draw` |
| EC6-05 | `src/ui/term.rs`::`ec6_04_ec6_05_ec6_06_ec6_16_who_may_ask_and_draw`<br>`tests/output.rs`::`ac6_19_ec6_05_without_a_terminal_a_missing_name_is_still_an_error` |
| EC6-06 | `src/ui/term.rs`::`ec6_04_ec6_05_ec6_06_ec6_16_who_may_ask_and_draw` |
| EC6-07 | `src/ui/render.rs`::`ec6_07_long_names_and_paths`<br>`tests/output.rs`::`ec6_07_long_paths_are_cut_in_the_middle` |
| EC6-08 | `src/ui/render.rs`::`ec6_08_wide_characters_stay_aligned` |
| EC6-09 | `src/ui/render.rs`::`ec6_09_control_characters_are_escaped_before_measuring` |
| EC6-10 | `tests/output.rs`::`ec6_10_ec6_11_empty_profiles_and_none_at_all` |
| EC6-11 | `tests/output.rs`::`ec6_10_ec6_11_empty_profiles_and_none_at_all` |
| EC6-12 | `src/ui/ask.rs`::`ac6_18_ec6_12_confirmation_rules`<br>`tests/output.rs`::`ac6_18_ec6_12_without_a_terminal_nothing_asks` |
| EC6-13 | `src/ui/ask.rs`::`ac6_19_ec6_13_pickers` |
| EC6-14 | `src/ui/ask.rs`::`ec6_14_interrupted_prompts_change_nothing` |
| EC6-15 | `src/ui/ask.rs`::`ac6_20_ec6_15_pager` |
| EC6-16 | `src/ui/term.rs`::`ec6_04_ec6_05_ec6_06_ec6_16_who_may_ask_and_draw` |
| EC6-17 | `tests/output.rs`::`ec6_17_quiet_and_verbose_together` |
| EC6-18 | `tests/output.rs`::`ec6_18_a_typo_gets_a_did_you_mean` |
| EC6-19 | `src/ui/progress.rs`::`ec6_19_progress_is_cleared_before_anything_else_prints` |
| EC6-20 | `tests/output.rs`::`ec6_20_a_closed_pipe_still_ends_quietly` |
| EC6-21 | `src/packaging.rs`::`ec6_21_decided_per_tool` |
| EC6-22 | `tests/install_from_clone.rs`::`ec6_22_a_wrapper_marketplace_name_taken_by_something_else` |

## Test results

On `main` at v2.2.0, verbatim:

```
test result: ok. 207 passed   (unit)
test result: ok. 55 passed    (cycle 1, tests/cli.rs)
test result: ok. 19 passed    (cycle 2, tests/codex.rs)
test result: ok. 29 passed    (cycle 5, tests/discovery.rs)
test result: ok. 10 passed    (docs and traceability)
test result: ok. 4 passed     (cycle 6 T0, tests/install_from_clone.rs)
test result: ok. 30 passed    (cycle 6, tests/output.rs: goldens, snapshots, interaction)
test result: ok. 51 passed    (cycle 4, tests/quarantine.rs)
test result: ok. 17 passed    (cycle 3, tests/source_commands.rs)
test result: ok. 17 passed    (cycle 3, tests/sources.rs)
test result: ok. 16 passed    (cycle 2, tests/takeover.rs)
TOTAL lines 23398, missed 1189, cover 94.92%   (gate: 90%)
```

455 tests. The terminal-only prompt functions (`ui/ask.rs`'s dialoguer calls) can't run without a terminal; every
decision around them is unit-tested with the prompt injected.

## Static checks

`just check` before every push: fmt, clippy `-D warnings`, llvm-cov ≥ 90%, typos, actionlint, zizmor, shellcheck,
gitleaks. All green on `main`.

## Security review

- **T0 closes a quarantine bypass:** a plugin now installs from the pinned, reviewed clone even when its repo's
  manifest points to GitHub.
- **Terminal safety:** text from repos and catalogs is escaped before it's measured or printed (EC6-09); colour codes
  are colctl's own.
- **Prompts never block automation:** they need stdin and stdout to be terminals, no `--json`, no `CI`; `--yes`
  always answers. Without a terminal behaviour is exactly 2.1's (AC6-18, AC6-19).
- **The pager** is `$PAGER` (split on spaces, no shell) or `less`; if it can't start, the text is printed.
- **New dependencies** are widely used, terminal-only crates (console-rs family, clap_complete); no network or file
  access.

## Follow-ups

- `-v` prints absolute paths for link steps; showing them as `~/…` would read better.
- ~~`cut_middle` keeps the tail over `~/Library`~~: replaced by `cut_alike` in the Revise.
- Carried: the `ETXTBSY` test flake; `review` flag noise on real releases (now grouped per file, which helps).

## Autonomy gate: what was not done

- colctl 2.2 has **not** been run on your real profiles; every smoke test used a fresh copy.
- Your real databricks install is still the `main` copy until you run `colctl sync` with 2.1.2 or later.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.2.0`
- [ ] `colctl status`, then `colctl sync` (moves databricks to your pin; watch the live progress), then `colctl status` again
- [ ] `colctl status --all`, `colctl --help`, `colctl update --help`
- [ ] `colctl review`, `colctl search databricks`, `colctl show databricks`
- [ ] In fish: `colctl completions fish > ~/.config/fish/completions/colctl.fish`
- [ ] Optional: `colctl rm source qdrant-skills` to see the confirmation (answer n)
- [ ] Decide the disposition

---

## Revise (2026-11-13, v2.2.1)

Your evaluation of 2.2.0 found three things; all three are fixed in #79 and released as **2.2.1**.

| Your note | Cause | Now | Criteria |
|---|---|---|---|
| Claude says 17 ok, Codex 2 ok | Lines counted states. Codex reads skills from `~/.agents`, so its profiles hold only plugins and `AGENTS.md`; the counts were right but didn't say so | Lines say what each profile has: `14 skills · 3 plugins` vs `2 plugins · skills via ~/.agents · 1 not packaged` | AC6-34 |
| `~/…/kilo-proxy-headless/profiles/claude` next to `~/Library/…/profiles/codex` | Each path was cut on its own, keeping as much as fit; the Codex path is 1 character shorter, so it kept `Library` | Paths are shown whole when they fit; otherwise every path keeps the same parts (`~/Library/…/profiles/claude`, `~/Library/…/profiles/codex`); contents wrap under themselves instead of being cut | AC6-35 |
| Help is a long list for developers | One flat list with no spacing; bare `colctl` printed an empty error | A welcome page: one sentence on what colctl does, **Start here** (init, status, sync), task groups separated by blank lines with one aligned column, short options, More/Docs. Bare `colctl` and `colctl help` show it; `colctl help <command>` works with flags; command help is styled and wrapped to the terminal | AC6-36, EC6-23 |

Also found by the new width tests: the verdict line ran past 60 columns (`✓ All 5 profiles in sync · 0 sources ·
nothing held for review`). Facts that don't fit are now left off.

**Process.** The criteria were added before code (#79's first commit). RED went up with the PR. Its first CI run
stopped at the format check before the tests ran, so I pushed the formatted tests and waited for CI to fail on the 8
tests themselves before pushing GREEN. The PR title also failed its check: `fix(status,help)` isn't a valid scope.

**Verified.** 34 + 1 new tests, coverage 94.87%, `just check` green. Smoke test on a copy of your setup in a real 80- and
60-column pseudo-terminal:

```
✓ All 5 profiles in sync · 3 sources · nothing held for review

  ✓  claude       ~/.claude                    14 skills · 3 plugins
  ✓  kilo-claude  ~/Library/…/profiles/claude  14 skills · 3 plugins
  ✓  codex        ~/.codex                     2 plugins · skills via ~/.agents
                                               1 not packaged
  ✓  kilo-codex   ~/Library/…/profiles/codex   2 plugins · skills via ~/.agents
                                               1 not packaged
  ✓  agents       ~/.agents                    14 skills
```

`1 not packaged` is a source plugin whose repo has no Codex packaging (cycle 7's converter is the fix for that).

### Reviewer checklist (Revise)

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.2.1`
- [ ] `colctl status` in your usual terminal, and in a narrow one
- [ ] `colctl`, `colctl help`, `colctl help sync`
- [ ] Decide the disposition
