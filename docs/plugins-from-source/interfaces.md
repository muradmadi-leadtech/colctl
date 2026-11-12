# Interfaces: Plugins from source

_Cycle 3 · Refine · 2026-11-12_ — additions to cycles [1](../claude-profile-sync/interfaces.md) and
[2](../codex-cli-support/interfaces.md).

## Command line

```
colctl add source <repo> [--rev <commit-ish>] [--name <name>] [--plugin <plugin>] [--dry-run]
colctl rm source <name> [--dry-run]
colctl update [<name>…] [--check] [--rev <commit-ish>] [--dry-run]
colctl adopt <harness> plugin <id>        # now converts to a source plugin when the source can be found
```

`update` exit codes: 0 done / nothing newer; 1 for `--check` when updates are available, or when any source failed;
2 usage errors (`--rev` without exactly one name, unknown source, unresolvable rev).

## `config.toml`

```toml
[[source]]
name = "superpowers"                          # [A-Za-z0-9][A-Za-z0-9._-]*, unique; also the clone folder name
repo = "https://github.com/obra/superpowers"   # https://, http://, ssh://, git@host:path, file://, absolute path, or github:owner/repo
rev = "8ca22dba9a94f28898bbce59f2537ff4d87c747d"   # full commit, 40 hex characters
plugin = "superpowers"                        # optional: the plugin's name in the repo's manifests (default: name)
replaces = ["superpowers@anthropic-plugin-directory"]   # optional: ids uninstalled wherever they're installed
```

`exclude` on claude and codex harnesses also accepts `sources/<name>`. `github:owner/repo` is cloned as
`https://github.com/owner/repo`.

## Files

```
~/.colctl/plugins/<name>/                     git clone, HEAD at rev; never edited by colctl
~/.colctl/plugins/.manifests/<name>/          only when the repo lacks a tool's marketplace manifest:
  .claude-plugin/marketplace.json             {"name": "colctl-<name>", "owner": {"name": "colctl"},
                                               "plugins": [{"name": "<plugin>", "source": "./plugin"}]}
  .agents/plugins/marketplace.json            {"name": "colctl-<name>", "plugins": [{"name": "<plugin>",
                                               "source": {"source": "url", "url": "./plugin"},
                                               "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"}}]}
  plugin -> ../../<name>[/<subpath>]          symlink to the clone (or its plugin subfolder)
~/.colctl/.gitignore                          gains "plugins/"
```

## Packaging detection (per tool)

| Tool | Marketplace manifest (preferred) | Plugin-only manifest (colctl writes the marketplace) |
|---|---|---|
| Claude | `.claude-plugin/marketplace.json` with a plugin named `<plugin>` | `.claude-plugin/plugin.json` |
| Codex | `.agents/plugins/marketplace.json` with a plugin named `<plugin>` | `.codex-plugin/plugin.json` |

Result per tool: marketplace folder (the clone, or `.manifests/<name>`), marketplace name (from the manifest, or
`colctl-<name>`), plugin id `<plugin>@<marketplace>`; or unsupported with the reason.

## Calls

| Action | Claude | Codex |
|---|---|---|
| Add local marketplace | `claude plugin marketplace add <folder>` | `codex plugin marketplace add <folder>` |
| Install | `claude plugin install <id>` | `codex plugin add <id>` |
| Update to the pin | `claude plugin marketplace update <m>` then `claude plugin update <id>` | `codex plugin add <id>` |
| Uninstall (rm, replaces) | `claude plugin uninstall <id>` | `codex plugin remove <id>` |
| Remove local marketplace (rm) | `claude plugin marketplace remove <m>` | `codex plugin marketplace remove <m>` |

Environment as in cycles 1–2. git: `clone`, `fetch origin`, `checkout --detach <rev>`, `rev-parse`,
`status --porcelain --untracked-files=all` (dirty check), `remote get-url origin`, `ls-remote origin HEAD`,
`rev-list --count <old>..<new>`.

## Finding a plugin's source (adopt, from-aggregator)

| Installed as | Source repo | Commit |
|---|---|---|
| Claude `p@anthropic-plugin-directory` | `plugins/plugin-directory-bindings.json` → `repositoryKey` `host/owner/repo#subpath` → `https://host/owner/repo` | `installed_plugins.json` → `gitCommitSha` |
| Claude `p@m` | `known_marketplaces.json` → m's folder → `.claude-plugin/marketplace.json` → p's `source`: `{github, repo}` → that repo; `{url}` → that URL; `"./path"` → m's own repo (`github` → `https://github.com/…`, `git`/`url` → URL) | `gitCommitSha` if recorded, else m's checkout HEAD |
| Codex `p@m` | `config.toml` `[marketplaces.m]` → m's folder → `.agents/plugins/marketplace.json` → p's `source` as above; relative → m's `source` URL | m's checkout HEAD (git), else upstream head (EC3-18) |

## State

`.state/state.json` → `harnesses.<h>.sources`: `{ "<source name>": { "id": "<plugin id>", "marketplace": "<m>", "rev": "<commit>" } }`
(what colctl installed there). Absent in older state files.

## `status --json`

Items gain kind `"source"` (name = source name) with states `ok`, `missing`, `outdated`, `unsupported`, `invalid`, and
plugin state `from-aggregator`. Top level gains:

```json
"sources": [
  { "name": "superpowers", "repo": "https://github.com/obra/superpowers", "rev": "8ca22db…", "clone": "ok" }
]
```

`clone`: `ok` | `missing` (sync will clone it) | `moved` (sync will check out the pin) | `dirty` | `invalid`.

## Commit messages

`colctl: add source <name>`, `colctl: rm source <name>`, `colctl: update source <name> to <short>`,
`colctl: adopt plugin <id> from source`.

## Changes made during Generate

Found while implementing and recorded here so intent and code agree:

1. `[[source]]` gained an optional `path` (the plugin's folder in the repo), needed to keep EC3-11's subpath.
2. `add source` defaults `--plugin` to the repo's name (not `--name`) and writes `plugin` when they differ.
3. **Marketplace re-pointing:** when a plugin's marketplace *is* its source repo served remotely (e.g. Codex's
   `superpowers-dev` cloned from github.com/obra/superpowers), the source's local marketplace has the same name. Sync
   then uninstalls the plugin, removes that marketplace and adds the clone under the same name. Other marketplaces are
   still never removed.
4. For a plugin installed through a colctl-written manifest, the commit a tool records is `~/.colctl`'s, not the
   clone's, so colctl relies on its own `sources` state there.
5. Clones are checked against `git config --get remote.origin.url` (the URL as configured), because `git remote
   get-url` applies `insteadOf` rewrites and wrongly reported such clones as foreign.
6. Clones are made with a normal `git clone` then moved to the pin (a `--no-checkout` clone looks dirty).
