# Edge cases: Claude profile sync

_Cycle 1 · Refine · 2026-11-12_

Every edge case has at least one test whose name contains its ID (`ec_14_…`).

## Empty / null

| ID | Case | Expected |
|---|---|---|
| EC-01 | No `config.toml` (never initialised) | Every command but `init` exits 2: "no colctl source at <path>; run colctl init" |
| EC-02 | `config.toml` with no harnesses | `status` prints "no harnesses registered" and exits 0; `sync` does nothing |
| EC-03 | Empty source (no `AGENTS.md`, skills or agents) | `sync` creates nothing and removes only owned links/files |
| EC-04 | `AGENTS.md` exists but is empty | Still imported; `CLAUDE.md` is generated |
| EC-05 | `<source>/skills/foo/` without `SKILL.md` | Reported `invalid` in the source; not linked anywhere; exit 1 |

## Invalid

| ID | Case | Expected |
|---|---|---|
| EC-06 | Harness path doesn't exist | Harness reported `harness-missing` with hint "start that profile once so it creates its folder"; skipped; never created |
| EC-07 | Harness path is a file or a symlink | Harness reported `invalid`; skipped |
| EC-08 | Item name outside `^[A-Za-z0-9][A-Za-z0-9._-]*$` (incl. `..`, leading dot, `/`) | Source item reported `invalid`; `add` exits 2 |
| EC-09 | Relative harness path in config | Config error, exit 2 |
| EC-10 | Two harnesses with the same canonical path | Config error naming both, exit 2 |
| EC-11 | After `marketplace add`, Claude records the marketplace under a different name than the config key | Action fails: "config calls it X, the marketplace calls itself Y"; plugin installs from it are skipped |
| EC-12 | `installed_plugins.json` / `known_marketplaces.json` missing | Treated as nothing installed |
| EC-13 | Those files exist but don't parse | Harness's plugins reported `invalid` with the parse error; no plugin action runs for that harness |

## Boundary / ownership

| ID | Case | Expected |
|---|---|---|
| EC-14 | Symlink in a harness pointing outside the source | Foreign; never changed; `adopt` refuses: "remove or replace this link yourself" |
| EC-15 | Generated `CLAUDE.md` edited by hand (hash differs) | Reported `edited`; `sync` leaves it; hint "move your text into <source>/AGENTS.md, delete CLAUDE.md, run sync" |
| EC-16 | `skills/synced/`, and any dot-entry in `skills/` or `agents/` | Ignored entirely; never foreign |
| EC-17 | Harness has no `skills/` or `agents/` folder | Created (mode 0755) when the first link is needed |
| EC-18 | Item listed in a harness's `exclude` | Never reported; an existing owned link for it is removed |
| EC-19 | Plugin in config already installed by hand | Reported `ok`; not recorded as colctl's; never uninstalled by colctl |
| EC-20 | Native harness written as `/Users/me/.claude` or via a symlinked home | Recognised as native by canonical path; `CLAUDE_CONFIG_DIR` unset for it |

## Failure

| ID | Case | Expected |
|---|---|---|
| EC-21 | A `.colctl-tmp-*` entry left by a crashed run | Removed at the start of the next changing run |
| EC-22 | Permission or I/O error on one item | That item fails with the OS error; others continue; exit 1 |
| EC-23 | `.state/state.json` doesn't parse | Exit 2: "state is corrupt at <path>; move it aside and run colctl status (generated files will then show as foreign)" |
| EC-24 | `.state/state.json` missing | Treated as empty state |
| EC-25 | `claude` hangs | Killed after 300 s; action fails with "timed out" |
| EC-26 | No git identity configured | Commit uses `-c user.name=colctl -c user.email=colctl@localhost` |

## Concurrent / timing

| ID | Case | Expected |
|---|---|---|
| EC-27 | Lock held by a pid that isn't running | Lock reclaimed with a notice; command proceeds |
| EC-28 | colctl run from inside a Claude session (`CLAUDE_CONFIG_DIR`, `ANTHROPIC_*` set) | Never inherited by `claude` calls; each harness gets exactly its own env |

## adopt

| ID | Case | Expected |
|---|---|---|
| EC-29 | Copy's tree hash differs from the original after copying | Copy deleted; original untouched; item refused; exit 1 |
| EC-30 | Adopting a plugin from `anthropic-plugin-directory` | Added to `enabled` with no `[marketplaces]` entry |
| EC-31 | `CLAUDE.md` in a harness is itself a symlink | Foreign; `adopt` refuses as in EC-14 |
