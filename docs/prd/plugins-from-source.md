# PRD: Plugins from source

_Cycle 3 · Focus · 2026-11-12_

## Problem statement

A plugin like superpowers exists once, in its author's repo, but reaches each harness by a different route:
- Claude installs it from Anthropic's plugin directory, an aggregator that lags behind the source. On the author's
  Mac that copy is 6.4.2 while the repo is at 7.0.2.
- Codex needs a second, separately declared install from the repo's own marketplace.

colctl 1.1 can keep these lists in step, but the owner still picks and maintains a different route per tool, and
nothing in `~/.colctl` holds the plugin itself.

## Story

> I want superpowers in all my harnesses. I install it in Claude the usual way. colctl sees it, finds its original
> repo (not the aggregator), clones that repo into `~/.colctl`, and installs the plugin from that one copy into
> every Claude and Codex profile, using each tool's own packaging from the repo. The aggregator's install goes away.
> When the author releases a new version, I update it once, in colctl.

## Target users

colctl users with Claude and Codex profiles who install plugins whose authors publish them in a git repo.

## What the tools support (verified by probes, see `.forge/learnings.md`)

| Need | Claude Code | Codex CLI |
|---|---|---|
| Install from a local folder | `claude plugin marketplace add <folder>` (source `directory`, read in place) + `claude plugin install <id>` | `codex plugin marketplace add <folder>` (`source_type = "local"`) + `codex plugin add <id>` |
| Follow a new commit of that folder | `claude plugin marketplace update <m>` then `claude plugin update <id>` | `codex plugin add <id>` again |
| Packaging in the repo | `.claude-plugin/marketplace.json` (+ `plugin.json`) | `.agents/plugins/marketplace.json` (+ `.codex-plugin/plugin.json`) |
| No marketplace manifest in the repo | A manifest colctl writes elsewhere, pointing at the clone, works | Same |
| Where the original repo is recorded | `plugins/plugin-directory-bindings.json` (`repositoryKey`) + `gitCommitSha`; `known_marketplaces.json`; the marketplace's own manifest | `[marketplaces.<m>].source` in `config.toml`, and that marketplace's manifest |

## Solution summary

- **Source plugins** are declared in `config.toml` with their repo and a **pinned commit**. colctl keeps one clone
  of each in `~/.colctl/plugins/<name>`, git-ignored by the source repo. The pin in `config.toml` is what gets
  committed.
- **Sync** checks out the pinned commit, then installs the plugin from that clone into every harness whose tool the
  repo packages. It uses the repo's own marketplace manifest, so the plugin keeps its author's id (e.g.
  `superpowers@superpowers-dev`). If the repo has no manifest, colctl writes a one-plugin manifest next to the
  clone, inside `~/.colctl` and never in the repo.
- **Tools the repo doesn't package** get nothing, and `status` shows them as `unsupported`.
- **`colctl update [<plugin>]`** fetches upstream, moves the pin to the newest commit of the default branch (or a
  given `--rev`), commits, and has every harness update from the clone. `colctl update --check` only reports what's
  newer. Nothing changes upstream versions without an explicit `update`.
- **`adopt`** turns an aggregator install into a source plugin:
  1. find the original repo;
  2. clone it **at the commit that's installed now**, so adopting changes no code;
  3. install it everywhere it's packaged for;
  4. uninstall the aggregator's plugin in every harness. Its marketplace stays.
- **`status`** flags aggregator installs whose source can be found (`from-aggregator`, hint `colctl adopt …`).

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | One copy of each source plugin under colctl. | Each declared source plugin has exactly one clone in `~/.colctl/plugins`, at its pinned commit. |
| SC-2 | The plugin reaches every harness its repo supports, from that copy. | After `sync`, every claude/codex harness with the matching packaging has `<plugin>@<marketplace>` installed from a local marketplace whose path is in `~/.colctl/plugins`. |
| SC-3 | The aggregator is bypassed. | After adopting `superpowers@anthropic-plugin-directory`, no harness has it installed, and every harness has `superpowers@superpowers-dev` at the same commit that was installed before. |
| SC-4 | Updates are explicit and single. | `colctl update superpowers` moves the pin once, commits it, and every harness then reports the new version. Without `update`, sync never changes the commit. |
| SC-5 | Unsupported tools are visible. | A Claude-only repo shows `unsupported` for codex harnesses in `status`, and sync never calls `codex` for it. |
| SC-6 | Nothing regresses. | Cycle 1 and 2 suites pass. Plugins in `[plugins]` and `[codex.plugins]` behave exactly as in 1.1. |
| SC-7 | Test-heavy and strict TDD, as in cycle 2. | AC3/EC3 tests named after IDs; ≥ 90% coverage; RED pushed before GREEN on each PR. |
| SC-8 | Docs. | Reference, how-to and explanation pages for source plugins, `update`, and the migration path. |

## Scope

**In:**
- Source plugins in `config.toml`, with a pinned clone in `~/.colctl/plugins`.
- Installing from it into claude and codex harnesses, using the repo's packaging or a colctl-written manifest.
- `update` (and `--check`, `--rev`).
- `adopt` from Claude's plugin directory, from marketplaces (including aggregators whose manifest points at another
  repo), and from Codex marketplaces.
- Uninstalling the aggregator's plugin when adopting.
- `status`/`sync`/`rm` support, and docs.

**Out (won't build this cycle):**
- **Quarantine:** reviewing or approving new versions before they install. That's cycle 4.
- **Converting** a plugin for a tool its repo doesn't package. That's a later cycle.
- Private repos that need credentials beyond what `git` already has, and non-git sources (npm, archives).
- Removing aggregator marketplaces. Only their plugins are uninstalled.
- Monorepos holding many plugins beyond picking one plugin by name from its marketplace manifest.
