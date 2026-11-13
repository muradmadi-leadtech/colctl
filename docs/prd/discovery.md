# PRD: Discovery

_Cycle 5 · Focus · 2026-11-13_

## Problem statement

colctl manages plugins you already know about. Finding new ones means leaving it:
- Claude has its own marketplace UI.
- Anthropic's plugin directory is a website.
- Codex has `codex plugin list`.

Each covers only its own tool, and none of them answers "what has this developer published?" or "where does this
plugin's code live?". To install something from source you then have to find its repo by hand and paste it into
`colctl add source`.

The data is already on the machine:
- Claude caches the plugin directory: 3,663 listings, each with its repo URL and reviewed commit.
- Claude keeps every marketplace it knows as a manifest on disk (317 plugins in `claude-plugins-official`).
- Codex lists its catalog as JSON.

colctl can read all of it, offline, and turn a search result straight into a source plugin.

## Story

> I want a plugin for API security testing. `colctl search api security` lists matching plugins from every catalog my
> harnesses know, each with its developer, its repo and whether I already have it. `colctl show 42crunch-api-security-testing`
> tells me more. `colctl developers obra` lists everything obra publishes. When I find one I like,
> `colctl add source <id>` clones it from its repo and installs it everywhere, as cycle 3 does. I didn't paste a URL.

## Target users

colctl users looking for plugins, especially across tools and by developer. Also the cycle 6 TUI, which needs every
view as JSON.

## Data sources (local only)

| Catalog | Read from | Has the repo |
|---|---|---|
| Marketplaces each Claude profile knows | `plugins/known_marketplaces.json` and each marketplace's `.claude-plugin/marketplace.json` | Yes, per plugin (a repo, or a path inside the marketplace repo) |
| Anthropic's plugin directory | Claude's cached `plugins/plugin-directory-cache-v2.json` | Yes: repo URL and reviewed commit |
| Marketplaces each Codex profile knows | `codex plugin list --available --json` per profile, plus local marketplace manifests | Only for git and local plugins; remote connectors have none |
| colctl's own sources | `config.toml` `[[source]]` and the clones | Yes |

Nothing contacts the network except `colctl marketplace refresh`, which asks `claude` and `codex` to update their own
catalogs.

## Solution summary

- **`colctl search [<terms>…]`**: plugins from every catalog, matched on name, description, keywords and developer,
  merged when the same plugin appears in several catalogs, and ranked by match. Filters: `--tool`, `--marketplace`,
  `--developer`, `--installed`, `--limit`. Each result shows its developer, repo, catalogs, where it's installed, and
  the command to add it.
- **`colctl show <id>`**: one plugin in full, including where its code lives and how colctl would install it.
- **`colctl developers [<owner>]`**: developers with their plugin counts, or one developer's plugins. A developer is
  the GitHub owner of the plugin's repo, or the manifest's author name when there's no repo.
- **`colctl marketplace list`**: every marketplace any profile knows, its source, plugin count, which profiles have it,
  and whether colctl manages it. **`colctl marketplace refresh`** updates the tools' catalogs (network, explicit).
- **`colctl connectors [<terms>…]`**: Codex's app connectors (Gmail, Slack…), which have no repo. Kept out of `search`.
- **One-step add**: `colctl add source <id>` accepts a search result's id. It looks up the repo (and the plugin's folder
  in a marketplace repo) and then works as cycle 3 does. Results without a repo say `colctl add plugin …` instead.
  A first-time add, so it isn't quarantined (cycle 4's rule).
- Every command has `--json`, for the cycle 6 TUI.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | One search covers every catalog. | A term that exists in a Claude marketplace, the plugin directory and a Codex marketplace returns all three, from fixture catalogs. |
| SC-2 | The same plugin appears once. | A plugin listed in two catalogs with the same repo is one result naming both. |
| SC-3 | Developers are browsable. | `developers obra` lists every plugin whose repo owner is `obra`, from any catalog; plugins without a repo fall back to their author name. |
| SC-4 | A result becomes a source plugin in one step. | `add source <id>` on a result clones the right repo (and folder) and installs it, with the same outcome as `add source <repo>`. |
| SC-5 | Offline and read-only. | `search`, `show`, `developers`, `marketplace list` and `connectors` write nothing, take no lock and work with the network off. |
| SC-6 | Fast enough to type. | `search` over the owner's real catalogs (about 4,000 plugins) answers in under 1 second. |
| SC-7 | JSON for the TUI. | Every new command's `--json` follows interfaces.md. |
| SC-8 | Nothing regresses; strict TDD; docs. | Earlier suites pass; AC5/EC5 tests named after IDs; ≥ 90% coverage; reference, how-to and explanation pages. |

## Scope

**In:** the commands above, reading the catalogs listed, merging, ranking, `add source <id>`, `marketplace refresh`,
docs.

**Out (won't build this cycle):**
- **Network search** (GitHub or anything else) and any curated or suggested marketplace list.
- **Ratings, install counts or reviews.** Even where a cache has them, ranking stays on the search terms.
- **Installing connectors** through colctl. They're listed only.
- **Holding first-time adds for review.** Unchanged from cycle 4.
- **Any UI beyond the CLI.** That's cycle 6.
