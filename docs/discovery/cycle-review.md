# Cycle review: discovery

_Cycle 5 · Evaluate · 2026-11-13_

| | |
|---|---|
| **Cycle** | `20261113-discovery` (FORGE, full mode) |
| **Run mode** | Focus, Orchestrate and Refine were confirmed with the owner phase by phase. Generate ran **autonomously** under the policy in `.forge/context.md`, extended to this cycle |
| **Intent** | [PRD](../prd/discovery.md): find plugins across every catalog Claude and Codex already keep, by developer too, offline and as JSON for the TUI, and add one from its source in one step |
| **Scope delivered** | Everything in the PRD's "In" list. Not built, as agreed: network/GitHub search, suggested marketplaces, ratings, installing connectors, any UI |
| **Released** | [v2.1.0](https://github.com/muradmadi-leadtech/colctl/releases/tag/v2.1.0): both archives attested (`gh attestation verify` exit 0); `mise exec github:muradmadi-leadtech/colctl@2.1.0 -- colctl --version` prints `colctl 2.1.0` |
| **PRs** | #54 plan · #55 T1 · #57 T2 · #58 T3+T4 · #59 T5+T6 · #60 T7 · #61 T8 · #62 smoke fixes · #56 release |
| **Production impact** | None on this machine. Real profiles were only read; the smoke test ran on a copy |
| **Disposition** | **Accept**, signed off by the owner on 2026-11-13 after using it on the real profiles (`add source qdrant-skills@claude-plugins-official` cloned github.com/qdrant/skills and installed `qdrant@qdrant` in both Claude profiles) |

## Your story, on a copy of your real setup

Run on the copy of your `~/.colctl`, both Claude and both Codex profiles (the same copy as cycle 4), with the real
`claude`, `codex`, `git` and GitHub:

1. **`search api security --limit 5`** answered in **0.24 s** over your real catalogs: 3,979 entries, merged into 3,742
   plugins, from `claude-plugins-official` (317), Anthropic's plugin directory (3,662) and colctl's sources. Top
   result: `42crunch-api-security-testing@claude-plugins-official` by `42crunch-ai`.
2. **`marketplace list`** showed the four marketplaces with their profiles; the two `superpowers-dev` ones (Claude and
   Codex) as `managed`.
3. **`developers`** listed `anthropics` (124 plugins) first; **`developers obra`** gave superpowers.
4. **`show superpowers`** merged its three listings (marketplace, directory, your source) into one, `already source
   plugin superpowers`.
5. **`connectors gmail`** ran the real `codex plugin list` in both Codex profiles and found
   `gmail@openai-curated-remote`.
6. **`add source 1password-developer-tools@anthropic-plugin-directory`** cloned `github.com/1password/1password-claude-plugin`
   at the directory's reviewed commit `006caca`, installed it in both Claude profiles, and marked Codex `unsupported`
   (the repo has no Codex packaging). **It only worked after fix #62**: see below.

### What the smoke test found (fixed in #62, now EC5-22 – 24)

| Found | Fix |
|---|---|
| The plugin directory's slug (`1password-developer-tools`) isn't the plugin's own name (`1password`). `add source <id>` failed: "no Claude or Codex packaging" | A catalog-named plugin falls back to the clone's `plugin.json` name (or the only plugin in its marketplace). `adopt` gets the same fallback. Results merge on repo and folder, not name |
| `show superpowers` paired version 6.4.2 (a catalog's) with commit 22f0b46 (your source's 7.0.2) | A source's version is read from the clone at the pin |
| `colctl search … \| head` **panicked**: "failed printing to stdout: Broken pipe" (every command had this) | colctl exits quietly with 141 |

Also found while testing T7 (fixed in #60): once colctl installs a source, Claude lists it under colctl's local
marketplace with the same id, and that entry replaced colctl's own. The source then looked like an ordinary
marketplace plugin. Sources are now read first.

## Strict TDD: the evidence

Each PR was opened on its RED commit, and CI's `check` failed before GREEN was pushed:

| PR | Task | RED run | Failed tests |
|---|---|---|---|
| #55 | T1 catalog | [43393270832](https://github.com/muradmadi-leadtech/colctl/actions/runs/43393270832) | 11 |
| #57 | T2 search | [43393725867](https://github.com/muradmadi-leadtech/colctl/actions/runs/43393725867) | 7 |
| #58 | T3+T4 search, show, developers | [43394328749](https://github.com/muradmadi-leadtech/colctl/actions/runs/43394328749) | 11 |
| #59 | T5+T6 marketplaces, connectors | [43394815738](https://github.com/muradmadi-leadtech/colctl/actions/runs/43394815738) | 6 |
| #60 | T7 add source <id> | [43395308307](https://github.com/muradmadi-leadtech/colctl/actions/runs/43395308307) | 6 |
| #61 | T8 docs | [43395928570](https://github.com/muradmadi-leadtech/colctl/actions/runs/43395928570) | 2 |
| #62 | smoke fixes | [43396499560](https://github.com/muradmadi-leadtech/colctl/actions/runs/43396499560) | 2 |

T1 and T2 used stubs; T3–T8 and #62 ran the new tests against the previous code.

**Where it slipped, stated plainly:**
- **T3+T4 and T5+T6 shipped as one PR each**, because they share a command module (and, for T5/T6, the fake CLI changes).
- **FORGE's state lagged:** I wrote the Orchestrate and Refine documents but forgot to tick the Orchestrate items, so the
  tool refused to advance until I fixed it. No documents were missing.
- **My fixtures used matching names everywhere,** which is why the slug/name mismatch (EC5-22) reached the smoke test.
  The real catalogs were the better test.
- **Test-support fixes during GREEN:** both fakes wrote their call log with read-modify-write, so parallel calls lost
  lines. They now append. Tests also needed the kilo Codex profile created before `init`, and `.git` aliases for the
  local stand-ins of GitHub repos.
- **A coverage data file (`.profraw`) was committed in #62's RED commit** by a test that ran the binary in the repo
  folder. GREEN removed it (the squash on `main` never had it), the test now runs in its temp folder, and
  `*.profraw` is git-ignored.

## Changes to the plan made during Generate

| What | Why |
|---|---|
| **AC5-25:** a connector id given to `add source` gets "isn't a repo or a plugin id from colctl search", not the no-repo message | Connectors aren't in the offline catalog (by design, they come from `codex`), so there's nothing to look up. Still exit 2, nothing written |
| **EC5-22 – 24** added | Smoke-test findings, above |
| **Merging uses repo + folder**, not repo + folder + name | Catalogs disagree on names (EC5-22) |
| **`origin` understands `git-subdir` sources (AC5-07)** | 99 official plugins use it; cycle 3's `adopt` couldn't find their source either |

## What changed

`git diff --stat v2.0.0..v2.1.0 -- src tests examples`: 12 files, +3,283 / −43.

| File | Nature |
|---|---|
| `src/catalog.rs` (new) | Reads every catalog into entries; installed markers; warnings |
| `src/search.rs` (new) | Merge, match, rank, filter, developers (pure) |
| `src/commands/discovery.rs` (new) | `search`, `show`, `developers`, `marketplace list/refresh`, `connectors` |
| `src/commands/mod.rs` | `add source <id>`; plugin-name fallback in `clone_and_check` |
| `src/origin.rs` | Shared `plugin_source`; `git-subdir`; `plugin_commit` |
| `src/main.rs` | Quiet exit on a closed pipe |
| `src/cli.rs` | New subcommands |
| `examples/fake_*.rs` | `plugin list --available --json`, `marketplace update/upgrade`; append-only call log |
| `tests/discovery.rs` (new) | 29 end-to-end tests |

## AC → test traceability

Every AC5 and EC5 ID has at least one test named after it; `tests/docs.rs` checks this (28 + 24 IDs).

| ID | Test(s) |
|---|---|
| AC5-01 | `src/catalog.rs`::`ac5_01_claude_marketplaces_are_read` |
| AC5-02 | `src/catalog.rs`::`ac5_02_the_plugin_directory_is_read` |
| AC5-03 | `src/catalog.rs`::`ac5_03_codex_marketplaces_on_disk_are_read` |
| AC5-04 | `src/catalog.rs`::`ac5_04_a_source_wins_over_its_own_local_marketplace`<br>`src/catalog.rs`::`ac5_04_ec5_23_colctl_sources_are_entries` |
| AC5-05 | `src/catalog.rs`::`ac5_05_ec5_06_installed_markers_and_one_entry_per_id` |
| AC5-06 | `src/catalog.rs`::`ac5_06_ec5_03_one_bad_catalog_does_not_stop_the_rest` |
| AC5-07 | `src/catalog.rs`::`ac5_07_git_subdir_sources_resolve_in_origin_too` |
| AC5-08 | `src/search.rs`::`ac5_08_the_same_plugin_is_one_result` |
| AC5-09 | `src/search.rs`::`ac5_09_matching_and_ranking` |
| AC5-10 | `src/search.rs`::`ac5_10_filters_and_limit` |
| AC5-11 | `src/search.rs`::`ec5_09_ac5_11_developers_from_hosts_paths_and_authors` |
| AC5-12 | `tests/discovery.rs`::`ac5_12_search_prints_results` |
| AC5-13 | `tests/discovery.rs`::`ac5_13_search_json` |
| AC5-14 | `tests/discovery.rs`::`ac5_14_show` |
| AC5-15 | `tests/discovery.rs`::`ac5_15_search_is_fast` |
| AC5-16 | `tests/discovery.rs`::`ac5_16_reading_writes_nothing_and_calls_nothing` |
| AC5-17 | `src/search.rs`::`ac5_17_developers_by_count_then_name`<br>`tests/discovery.rs`::`ac5_17_listing_developers` |
| AC5-18 | `tests/discovery.rs`::`ac5_18_one_developer` |
| AC5-19 | `tests/discovery.rs`::`ac5_19_marketplace_list` |
| AC5-20 | `tests/discovery.rs`::`ac5_20_marketplace_refresh` |
| AC5-21 | `tests/discovery.rs`::`ac5_21_connectors` |
| AC5-22 | `tests/discovery.rs`::`ac5_22_connectors_stay_out_of_search` |
| AC5-23 | `tests/discovery.rs`::`ac5_23_add_source_from_a_result` |
| AC5-24 | `tests/discovery.rs`::`ac5_24_a_plugin_in_a_subfolder` |
| AC5-25 | `tests/discovery.rs`::`ac5_25_results_without_a_repo` |
| AC5-26 | `tests/discovery.rs`::`ac5_26_already_a_source` |
| AC5-27 | `tests/docs.rs`::`ac5_27_docs_cover_discovery` |
| AC5-28 | `tests/docs.rs`::`ac3_26_ac4_33_ac5_28_earlier_suites_are_still_complete`<br>`tests/docs.rs`::`ac_35_ac2_23_ac3_28_ac4_33_ac5_28_every_criterion_and_edge_case_has_a_named_test` |
| EC5-01 | `tests/discovery.rs`::`ec5_01_no_catalogs` |
| EC5-02 | `src/catalog.rs`::`ec5_02_a_directory_cache_with_another_schema_is_skipped_with_a_warning` |
| EC5-03 | `src/catalog.rs`::`ac5_06_ec5_03_one_bad_catalog_does_not_stop_the_rest` |
| EC5-04 | `src/catalog.rs`::`ec5_04_nameless_and_repeated_entries` |
| EC5-05 | `src/catalog.rs`::`ec5_05_missing_harness_folders_are_skipped_and_excludes_do_not_hide` |
| EC5-06 | `src/catalog.rs`::`ac5_05_ec5_06_installed_markers_and_one_entry_per_id` |
| EC5-07 | `src/search.rs`::`ec5_07_one_repo_several_plugins_stay_separate` |
| EC5-08 | `src/search.rs`::`ec5_08_repo_urls_normalise` |
| EC5-09 | `src/search.rs`::`ec5_09_ac5_11_developers_from_hosts_paths_and_authors` |
| EC5-10 | `tests/discovery.rs`::`ec5_10_nothing_matches` |
| EC5-11 | `tests/discovery.rs`::`ec5_11_show_unknown_or_ambiguous` |
| EC5-12 | `tests/discovery.rs`::`ec5_12_limit_zero_and_a_bad_tool` |
| EC5-13 | `tests/discovery.rs`::`ec5_13_control_characters_are_escaped` |
| EC5-14 | `src/commands/discovery.rs`::`ec5_14_long_descriptions_are_cut_to_the_line` |
| EC5-15 | `tests/discovery.rs`::`ec5_15_connectors_need_codex_and_a_profile` |
| EC5-16 | `tests/discovery.rs`::`ec5_16_one_profile_failing_still_lists_the_rest` |
| EC5-17 | `tests/discovery.rs`::`ec5_17_refresh_without_a_tool` |
| EC5-18 | `tests/discovery.rs`::`ec5_18_a_git_at_repo_is_still_a_repo` |
| EC5-19 | `tests/discovery.rs`::`ec5_19_unknown_ids_and_bare_names` |
| EC5-20 | `tests/discovery.rs`::`ec5_20_an_unreachable_repo_writes_nothing` |
| EC5-21 | `tests/discovery.rs`::`ec5_21_name_and_plugin_override_the_result` |
| EC5-22 | `tests/discovery.rs`::`ec5_22_a_catalog_name_that_differs_from_the_plugins_own` |
| EC5-23 | `src/catalog.rs`::`ac5_04_ec5_23_colctl_sources_are_entries` |
| EC5-24 | `tests/discovery.rs`::`ec5_24_a_closed_pipe_ends_quietly` |

## Test results

On `main` at v2.1.0, verbatim:

```
test result: ok. 183 passed   (unit)
test result: ok. 55 passed    (cycle 1, tests/cli.rs)
test result: ok. 19 passed    (cycle 2, tests/codex.rs)
test result: ok. 29 passed    (cycle 5, tests/discovery.rs)
test result: ok. 9 passed     (docs and traceability)
test result: ok. 51 passed    (cycle 4, tests/quarantine.rs)
test result: ok. 17 passed    (cycle 3, tests/source_commands.rs)
test result: ok. 17 passed    (cycle 3, tests/sources.rs)
test result: ok. 16 passed    (cycle 2, tests/takeover.rs)
TOTAL lines 18877, missed 924, cover 95.11%   (gate: 90%)
```

396 tests. No earlier test was changed this cycle. AC5-15's timing test (4,000 plugins under 1 s) passes under coverage
instrumentation too.

## Static checks

`just check` before every push: `cargo fmt --check`, `clippy -D warnings`, `cargo llvm-cov` ≥ 90% lines, `typos`,
`actionlint`, `zizmor`, `shellcheck`, `gitleaks`. All green on `main`.

## Security review

- **Read-only by default.** `search`, `show`, `developers` and `marketplace list` only read files the tools already
  keep; they write nothing, take no lock and run no tool (AC5-16). Only `marketplace refresh` (through the tools) and
  `add source` use the network.
- **Untrusted text.** Catalog names and descriptions come from third parties. They're escaped in text output (EC5-13);
  JSON carries them raw for the TUI to escape.
- **What `add source <id>` trusts.** It clones the repo a catalog names, at the commit the catalog recorded (for the
  plugin directory, the commit Anthropic reviewed). It's a first-time add, so it isn't held for review (cycle 4's rule).
  Every later update is. A catalog could name any repo; `show` shows it before you add.
- **Tool calls** (`refresh`, `connectors`) use the same environment rules as sync: each profile's own config folder,
  credential variables removed, a timeout.

## Follow-ups

- **Connectors' descriptions are empty and their developer is `unknown`.** Codex's list doesn't include those fields.
  The TUI could show the marketplace instead.
- **`sync`'s "needs attention" block lists `unsupported` lines while ending "in sync"** (pre-existing). Worth a
  wording pass in the TUI cycle.
- Carried from cycle 4: the `ETXTBSY` test flake and `review`'s flag noise.

## Autonomy gate: what was not done

- colctl 2.1 has **not** been run on your real profiles; the smoke test used a copy.
- The release PR was merged by me under the policy; nothing outside the repo was changed.
- The disposition is yours.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.1.0`
- [ ] `colctl search api security`, `colctl show superpowers`, `colctl developers | head`
- [ ] `colctl marketplace list`, `colctl connectors gmail`
- [ ] Optional: `colctl add source <id>` for a plugin you want (it installs everywhere it's packaged; `colctl rm source <name>` undoes it)
- [x] Decide the disposition: **Accept**
