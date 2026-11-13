# Where the catalogs come from

colctl doesn't run a plugin index of its own. Every harness already downloads and caches catalogs of plugins it can
install, and `search`, `show`, `developers` and `marketplace list` read those caches directly.

| Catalog | Kept by | What colctl reads |
|---|---|---|
| Marketplaces you (or colctl) added to Claude, such as `claude-plugins-official` | Claude, in each profile's `plugins/` | `known_marketplaces.json`, then each marketplace's `.claude-plugin/marketplace.json` |
| Anthropic's plugin directory | Claude, `plugins/plugin-directory-cache-v2.json` | Listings for Claude Code, with each plugin's repo and the commit Anthropic reviewed |
| Codex marketplaces from a folder or a Git repo | Codex, `config.toml` and its marketplace snapshots | Each marketplace's `.agents/plugins/marketplace.json` |
| colctl's own source plugins | colctl, `config.toml` and the clones | What each clone packages |
| Codex's remote catalog (connectors) | OpenAI, through `codex plugin list` | Only for `colctl connectors` |

## Why this way

- **No network for browsing.** Search answers from files that are already on disk, in well under a second, even over
  thousands of plugins.
- **Never staler than the tools.** When Claude refreshes its marketplaces, colctl sees the result.
  `colctl marketplace refresh` asks both tools to refresh.
- **Nothing to trust twice.** colctl reads what the tools themselves would install from. It doesn't add a list of its
  own.

## One plugin, many listings

The same plugin often appears in several catalogs: in a marketplace, in the plugin directory, and in colctl's sources once
you've added it. colctl merges listings that point at the same repo, folder and name into one result. That's why one
plugin can have several ids, and why `search` can say it's "already source plugin superpowers".

## Developers

A developer is the owner of the plugin's repo. It's stable and checkable: `obra` publishes
`github.com/obra/superpowers`. Manifest author names are free text, so they're used only when there's no repo to go by.

## What it doesn't do

It doesn't search GitHub for marketplaces nobody has added, rank by popularity, or rate plugins. To find a new
marketplace, add it to a tool (or to `config.toml`) and it shows up in the next search.
