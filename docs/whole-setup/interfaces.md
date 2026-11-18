# Interfaces: whole setup

## What the tools do (verified 2026-11-18, Claude Code 2.1.327, codex-cli 0.169.0)

Checked against throwaway `CLAUDE_CONFIG_DIR`, `CODEX_HOME` and `HOME` folders with a fake server that logs its env.

| Question (PRD "To verify") | Answer |
|---|---|
| Where `claude mcp add -s user` writes | `$CLAUDE_CONFIG_DIR/.claude.json` when set (it also writes `backups/`), else `~/.claude.json`. Shape: `mcpServers.<name> = {type, command, args, env}` or `{type: "http", url, headers}` |
| Does Claude expand `${VAR}` in a user-scope server? | **Yes.** `-e 'K=${FOO}'` is stored literally and the server starts with `K` set to `$FOO`'s value. The same goes for `-H 'Authorization: Bearer ${FOO}'`. So an env reference never has to be resolved for Claude |
| Where `codex mcp add` writes | `$CODEX_HOME/config.toml`: `[mcp_servers.<name>]` with `command`, `args`, `[…env]`, or `url` and `bearer_token_env_var` |
| Can Codex keep an env reference? | For HTTP auth, yes: `--bearer-token-env-var NAME`. For a stdio server's env, **no**: Codex has `env_vars = [names]` but `codex mcp add` can't set it (`-c` doesn't persist). colctl resolves the value and passes it with `--env`, so it lands in `config.toml`, as it does today when added by hand |
| Codex HTTP headers other than a bearer token | Not settable through `codex mcp add`. Such a server is `unsupported` in Codex profiles, with the reason |
| Reading a profile's servers | Claude: parse `.claude.json` (`claude mcp list/get` start every server). Codex: parse `config.toml`; `codex mcp get --json` is the same data, one process per server |
| User commands in Codex | No `~/.codex/prompts` folder in 0.169, and nothing reads one. A command becomes a **skill** in Codex |
| Hooks in a plugin | Claude runs a plugin's `hooks/hooks.json` like hooks in `settings.json`. Codex runs converted plugin hooks after the owner trusts them in `/hooks` (cycle 7) |

## `config.toml`

```toml
[[mcp]]
name = "toolport"                                   # [a-z0-9][a-z0-9_-]*, unique
command = "/Applications/Toolport.app/…/toolport-gateway"   # either command (+ args) …
args = []
# url = "https://mcp.example.com/mcp"               # … or url (+ headers, bearer)
# bearer = { env = "EXAMPLE_TOKEN" }                # HTTP only: Authorization: Bearer <value>
# headers = { "X-Team" = "core" }                   # HTTP only; Claude profiles only
tools = ["claude", "codex"]                         # optional; default both
env = { TOOLPORT_PROFILE = "trimmed" }

[mcp.profile.claude]                                # optional overrides, by harness name
env = { TOOLPORT_CLIENT_ID = "claude-code" }
[mcp.profile.kilo-claude]
env = { TOOLPORT_CLIENT_ID = "claude-code@f32399132c8e1a62" }
```

- A value in `env`, `headers` or `bearer` is a literal string, `{ env = "NAME" }` or `{ keychain = "item" }`.
- Overrides may set `env`, `args` and `headers`. `env` and `headers` merge key by key over the shared ones, and
  `args` replaces them. An override for a harness that doesn't exist or doesn't take MCP servers is an error naming it.

### What each reference becomes

| Reference | Claude profile | Codex profile |
|---|---|---|
| literal | passed as is | passed as is |
| `{ env = "N" }` in `env` | `${N}`, expanded by Claude at launch | value of `$N` in colctl's environment at sync, passed with `--env` |
| `{ env = "N" }` as `bearer` | header `Authorization: Bearer ${N}` | `--bearer-token-env-var N` |
| `{ keychain = "i" }` | resolved at sync, passed as the value | resolved at sync, passed as the value |

Keychain: `security find-generic-password -s <item> -w` (macOS). On other systems a keychain reference is an error
naming the item.

## `.state`

`HarnessState.mcp: BTreeMap<String, McpRecord>` (optional field; older state files still parse).

```text
McpRecord { config: "sha256:…",   // the resolved-free entry for this harness (references, not values)
            written: "sha256:…" } // what the profile's file held right after colctl added it
```

Status never resolves a secret. A server is:

| State | When |
|---|---|
| `ok` | recorded, `config` matches the entry now, and the profile's entry hashes to `written` |
| `missing` | in config, not in the profile |
| `differs` | recorded, but the entry changed in config, or someone changed it in the profile |
| `foreign` | in the profile, not recorded, and the name isn't in config (left alone) |
| `taken` | in config and in the profile under the same name, not recorded: `adopt` or `sync --replace-mcp <name>` takes it over |
| `orphaned` | recorded, gone from config: sync removes it |
| `unsupported` | this tool can't express it (Codex with custom headers) |

## Commands

```text
colctl add mcp <name> [--only claude|codex] [--env K=V]... [--env-from K=NAME]... [--keychain K=ITEM]...
               (-- <command> [args]... | --url <url> [--header K=V]... [--bearer-from NAME | --bearer-keychain ITEM])
               [--dry-run]
colctl rm mcp <name> [--dry-run]
colctl add command <file.md> [--dry-run]           # copies to ~/.colctl/commands/<name>.md
colctl rm command <name> [--dry-run]
colctl sync [--replace-mcp <name>]...
colctl adopt [<profile>]                           # now also offers MCP servers, commands and hooks
```

`add`/`rm` edit `config.toml` (or the folder), commit, then sync, as `add skill` does. `--env K=V` with a value that
looks like a secret is refused: "That looks like a secret. Use --env-from K=NAME or --keychain K=ITEM."

### Output (sync)

```text
✓ toolport            mcp       claude, kilo-claude, codex, kilo-codex
+ github              mcp       added to claude, kilo-claude · codex: set GITHUB_TOKEN first
! search              mcp       kilo-claude: changed outside colctl · colctl sync --replace-mcp search
```

`--json` gains, per profile, `"mcp": [{"name","state","detail"}]`, `"commands": […]` and `"hooks": […]` in the
existing item shape. Nothing existing changes.

## `secret` (new, `src/secret.rs`)

```text
resolve(&Value, env: &dyn Fn(&str)->Option<String>) -> Result<String, SecretError>  // Missing{name} | Keychain{item, why}
looks_secret(key: &str, value: &str) -> bool   // key matches TOKEN|SECRET|KEY|PASSWORD|AUTH|CREDENTIAL (case-insensitive),
                                               // or value matches a known token shape (ghp_, github_pat_, sk-, xox?-,
                                               // AKIA, glpat-, eyJ…JWT) or is ≥ 32 chars of base64/hex
redact(text) -> String                         // every resolved value in this run → "•••"
leak_check(config_text) -> Vec<Finding>        // literal values for which looks_secret is true
```

- The leak check runs inside `Git::commit_all` for `~/.colctl`. A finding refuses the commit and names the key and
  file, but never the value.
- `TOOLPORT_CLIENT_ID = "claude-code@f32399132c8e1a62"` must not count as a secret: the key doesn't match, and the
  value is short and contains `@`. That's a test.

## Commands (T5)

- Source: `~/.colctl/commands/<name>.md`; `name` is `[a-z0-9][a-z0-9_-]*`.
- Claude profile: a link `commands/<name>.md` → the source file (ownership by link target, as for skills).
- Codex profiles: inside the generated `colctl-setup` plugin as `skills/<name>/SKILL.md`. Frontmatter
  `name: <name>`, and `description` from the command's `description:` or its first line. The body is the command's
  text; `$ARGUMENTS` becomes "the user's request".
- Adopt: a file in a Claude profile's `commands/` that isn't a colctl link.

## Hooks (T6) and the `colctl-setup` plugin

- Source: `~/.colctl/hooks/hooks.json` (Claude's plugin hooks format) and any scripts beside it. Commands use
  `${CLAUDE_PLUGIN_ROOT}/<script>`.
- colctl builds `colctl-setup` in its marketplace folder on every sync when there's anything to carry. The marketplace
  is `colctl-setup` and the plugin id is `setup@colctl-setup`:
  - Claude: `hooks/hooks.json` and the scripts.
  - Codex: the same hooks passed through `repackage::hooks` (cycle 7), plus `skills/` from commands.
- The plugin's version is the content hash, so a change reinstalls it (the 2.5 lesson: Claude ignores an update whose
  version didn't change).
- Adopt: hooks in a profile's `settings.json` (`hooks` key) are copied into `hooks/hooks.json` and merged by event and
  matcher. colctl then says: "Remove these from <settings.json> yourself; colctl doesn't edit it. Until you do, they run
  twice."
- Hooks are held for review like a source update? **No**, they're the owner's own files. They're flagged in `status`
  with `runs code` the first time they're added, as plugins are.

## Jobs (`core::run::Job`)

`AddMcp{…}`, `RmMcp{name}`, `AddCommand{path}`, `RmCommand{name}`, and `Adopt` extended. Each calls the same function
as its command.
