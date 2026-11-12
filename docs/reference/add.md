# colctl add

```
colctl add skill <path | git-url> [--subdir <path>] [--name <name>] [--dry-run]
colctl add agent <file.md> [--dry-run]
colctl add plugin <name@marketplace> [--codex] [--marketplace <source>] [--dry-run]
colctl add source <repo> [--rev <commit-ish>] [--name <name>] [--plugin <plugin>] [--dry-run]
```

Each adds one item to the source, commits (`colctl: add <kind> <name>`), then runs [sync](sync.md).

**skill**: copies a folder containing `SKILL.md` to `~/.colctl/skills/<name>`, leaving out any `.git`. A source with
`://` or starting with `git@` is shallow-cloned first; `--subdir` picks the skill's folder inside it. The name is the
folder's (or repo's) name unless `--name` is given, and must match `[A-Za-z0-9][A-Za-z0-9._-]*`.

```sh
colctl add skill https://github.com/scottfeltham/forge-skill --subdir .claude/skills/forge
```

**agent**: copies `<name>.md` to `~/.colctl/agents/<name>.md`.

**plugin**: adds the id to `[plugins].enabled` in `config.toml`, and with `--marketplace` also adds
`<marketplace> = "<source>"` to `[marketplaces]` if it isn't there. Sources: `github:owner/repo`, an `http(s)://`
URL, or an absolute path. Plugins from `anthropic-plugin-directory` need no marketplace. `config.toml` keeps its
comments and formatting.

**plugin --codex**: the same for Codex, in `[codex.plugins]` and `[codex.marketplaces]`; committed as
`colctl: add codex plugin <id>`. Plugins from `openai-curated` (Codex's built-in marketplace) need no marketplace.
Sync installs it in every codex harness with `codex plugin marketplace add` and `codex plugin add`.

```sh
colctl add plugin superpowers@superpowers-dev --codex --marketplace github:obra/superpowers
```

Exit 2, with nothing changed, when the item is already managed, the name or id is invalid, the folder has no
`SKILL.md`, the clone fails, or the marketplace is unknown and `--marketplace` is missing.

**source** (`colctl add source`): clones the repo into `~/.colctl/plugins/<name>`, pins `--rev` (a tag, branch or
commit) or the default branch's head, and checks that it packages the plugin for Claude or Codex. Then it adds a
`[[source]]` entry, commits `colctl: add source <name>`, and syncs. The name defaults to the repo's name, and the
plugin to the repo's name. A repo with no packaging for the plugin is refused (exit 2) and its clone removed.

```sh
colctl add source https://github.com/obra/superpowers
colctl add source github:owner/repo --rev v2.1.0 --plugin the-plugin
```
