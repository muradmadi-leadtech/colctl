# config.toml

`~/.colctl/config.toml` registers harnesses and lists marketplaces and plugins. Unknown keys and invalid values are
errors that name the line (`config.toml: line 7: duplicate harness name "claude"`), and every command refuses to run
until they're fixed.

```toml
version = 1                      # required, must be 1

[[harness]]                      # zero or more
name = "kilo-claude"             # required, unique, [a-z0-9-]+
kind = "claude"                  # required: "claude", "codex" or "agents"
path = "~/Library/Application Support/kilo-proxy-headless/profiles/claude"   # absolute or ~/…
exclude = ["skills/foo"]         # optional

[[harness]]
name = "kilo-codex"
kind = "codex"                   # a Codex CLI profile (a CODEX_HOME)
path = "~/Library/Application Support/kilo-proxy-headless/profiles/codex"

[[harness]]
name = "agents"
kind = "agents"                  # the shared ~/.agents folder (Codex reads skills from it)
path = "~/.agents"

[marketplaces]                   # optional; name = source
sadr = "github:muradmadi/sadr"   # github:owner/repo | https://… | /absolute/path

[plugins]
enabled = ["sadr@sadr", "superpowers@anthropic-plugin-directory"]

[codex.marketplaces]             # optional; Codex's own marketplaces
superpowers-dev = "github:obra/superpowers"

[codex.plugins]
enabled = ["superpowers@superpowers-dev", "github@openai-curated"]
```

## Kinds

| Kind | The folder is | colctl manages there | Valid `exclude` entries |
|---|---|---|---|
| `claude` | a Claude Code config folder (`~/.claude`, or any `CLAUDE_CONFIG_DIR`) | `CLAUDE.md`, `skills/*`, `agents/*.md`, plugins from `[plugins]` | `CLAUDE.md`, `skills/<name>`, `agents/<name>.md`, `plugins/<id>` |
| `codex` | a Codex CLI profile (`~/.codex`, or any `CODEX_HOME`) | `AGENTS.md` (a link), `agents/*.toml` (generated), plugins from `[codex.plugins]` | `AGENTS.md`, `agents/<name>.toml`, `plugins/<id>` |
| `agents` | the shared `~/.agents` folder | `skills/*` | `skills/<name>` |

Codex reads skills from `~/.agents/skills` for every profile, so Codex skills come through the `agents` harness, not
the `codex` ones. See [Codex and the shared skills folder](../explanation/codex-and-the-shared-skills-folder.md).

| Key | Rules |
|---|---|
| `harness.name` | `[a-z0-9-]+`, unique |
| `harness.path` | Absolute or starting with `~/`. Two harnesses may not resolve to the same folder. |
| `harness.exclude` | Entries: `CLAUDE.md`, `skills/<name>`, `agents/<name>.md`, `plugins/<name@marketplace>` |
| `marketplaces.<name>` | `[A-Za-z0-9][A-Za-z0-9._-]*`, and must equal the name in the marketplace's own `marketplace.json` |
| `plugins.enabled` | `name@marketplace`; the marketplace must be in `[marketplaces]` unless it is `anthropic-plugin-directory`. No duplicates. |
| `codex.marketplaces.<name>` | As for `[marketplaces]`; `openai-curated` is Codex's built-in marketplace and can't be declared |
| `codex.plugins.enabled` | `name@marketplace`; the marketplace must be in `[codex.marketplaces]` unless it is `openai-curated`. A Claude marketplace doesn't count. |

`colctl add plugin`, `rm plugin`, `adopt` and `init` edit this file in place and keep its comments and formatting.

## Source plugins

A plugin installed from its own repo, pinned to a commit. See [plugins from source](../explanation/plugins-from-source.md).

```toml
[[source]]
name = "superpowers"                                  # [A-Za-z0-9][A-Za-z0-9._-]*, unique; the clone's folder name
repo = "https://github.com/obra/superpowers"           # URL, git@host:path, file://, absolute path, or github:owner/repo
rev = "8ca22dba9a94f28898bbce59f2537ff4d87c747d"       # the pinned commit, all 40 hex characters
plugin = "superpowers"                                # optional: its name in the repo's manifests (default: name)
path = "plugins/superpowers"                          # optional: its folder inside the repo, if not the root
replaces = ["superpowers@anthropic-plugin-directory"] # optional: ids uninstalled wherever they're installed
```

colctl clones each source into `~/.colctl/plugins/<name>` (git-ignored) at `rev`, and installs it in every claude and
codex harness its repo packages for. Change `rev` with [`colctl update`](update.md) rather than by hand. A claude or
codex harness can opt out of one source with `exclude = ["sources/<name>"]`.
