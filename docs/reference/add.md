# colctl add

```
colctl add skill <path | git-url> [--subdir <path>] [--name <name>] [--dry-run]
colctl add agent <file.md> [--dry-run]
colctl add plugin <name@marketplace> [--codex] [--marketplace <source>] [--dry-run]
colctl add source <repo-or-id> [--rev <commit-ish>] [--name <name>] [--plugin <plugin>] [--dry-run]
colctl add command <file.md> [--dry-run]
colctl add mcp <name> [--only claude|codex] [--env K=V]... [--env-from K=NAME]... [--keychain K=ITEM]...
               (-- <command> [args]... | --url <url> [--header K=V]... [--bearer-from NAME | --bearer-keychain ITEM])
               [--dry-run]
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

### From a search result

`colctl add source <id>` also takes a plugin id from [`colctl search`](search.md) (`name@marketplace`). colctl looks the
plugin up in the [catalogs](../explanation/catalogs.md) and uses:

- its **repo**, and its **folder** in that repo when it lives in one (a marketplace repo, a `git-subdir` source, a
  directory listing with a path), stored as the source's `path`;
- the **commit** the catalog recorded (the plugin directory's reviewed commit, a `sha` in the marketplace), unless you
  pass `--rev`; otherwise the default branch's head;
- the plugin's **name** as the source name and plugin name, unless you pass `--name` or `--plugin`.

```sh
colctl add source superpowers@claude-plugins-official
colctl add source frontend-design@anthropic-plugin-directory --name frontend
```

An argument that is a valid repo (a URL, `git@host:path`, an absolute path or `github:owner/repo`) is always a repo,
never looked up. Refused with exit 2: an id the catalogs don't have, a plugin with no repo to clone (use
`colctl add plugin <id>` to keep it on the marketplace list), and a plugin that's already a source plugin.
Adding from a result is a first-time add, so it isn't held for review.

## add command

Copies `<name>.md` to `~/.colctl/commands/<name>.md` (the name must match `[a-z0-9][a-z0-9_-]*`). Sync links it into
every Claude profile's `commands/`, so it's `/<name>` there, and writes it as a skill in `~/.agents/skills/<name>/`
for Codex, which has no commands of its own: the skill's description is the command's `description:` (or its first
line), and `$ARGUMENTS` reads "the user's request". Edit the command, not the skill: sync rewrites the skill when the
command changes, and leaves it alone once you've edited it.

## add mcp

Adds an `[[mcp]]` entry to `config.toml` (see [config](config.md#mcp-servers)), commits it (`colctl: add mcp <name>`)
and syncs, which runs `claude mcp add -s user` and `codex mcp add` in each profile. A server goes to every Claude and
Codex profile unless `--only` says one tool.

- `-- <command> [args]`: a server colctl's tools start. Everything after `--` is the command, so flags like
  `--stdio` are the server's.
- `--url <url>`: a streamable HTTP server. `--header` is Claude only (`codex mcp add` can't set headers).
- `--env K=V`: a plain value. A value that looks like a secret is refused: "That looks like a secret. Use --env-from
  K=NAME or --keychain K=ITEM."
- `--env-from K=NAME`: `K` gets your `$NAME`. Claude keeps it as `${NAME}` and reads it when it starts the server;
  Codex gets the value colctl sees when it syncs.
- `--keychain K=ITEM`: `K` gets the macOS keychain item's value (`security find-generic-password -s ITEM -w`), read
  at sync and passed to the tool.
- `--bearer-from NAME` / `--bearer-keychain ITEM`: `Authorization: Bearer …` for a URL server. Codex takes the token
  from an environment variable only.

```sh
colctl add mcp github --env-from GITHUB_TOKEN=GH_TOKEN -- npx -y @modelcontextprotocol/server-github
colctl add mcp docs --url https://docs.example/mcp --bearer-from DOCS_TOKEN
```
