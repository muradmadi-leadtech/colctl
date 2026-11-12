# PRD: Claude profile sync

_Cycle 1 · Focus · 2026-11-12_

## Problem statement

People who run more than one AI coding harness, or more than one profile of the same harness, keep a separate copy of
their instructions, skills, agents and plugins in each one. On the author's machine that's native Claude Code
(`~/.claude`) and a kilo-proxy Claude profile (`~/Library/Application Support/kilo-proxy-headless/profiles/claude`).
Keeping them in step is manual, so they drift: on 2026-11-12 the native profile had the superpowers plugin and the kilo
profile didn't. Symlinking folders by hand doesn't scale past two profiles and gives no way to see what's out of step.

## Target users

1. **Primary:** a developer on macOS (Apple silicon) or Linux who runs two or more Claude Code profiles (native plus
   one or more gateway/proxy profiles such as kilo-proxy) and edits skills or instructions regularly.
2. **Secondary (later cycles):** the same developer adding Codex CLI or OpenCode, which read the same skill format.

## Solution summary

`colctl` keeps one source of truth in `~/.colctl` (a local git repo) and makes each registered harness point at it:

- `AGENTS.md` → each Claude profile gets a generated `CLAUDE.md` that imports it.
- `skills/<name>/` and `agents/<name>.md` → one symlink per item in each profile, so one copy on disk.
- Marketplaces and plugins → declared in `config.toml` and installed into each profile with Claude's own CLI.

`colctl status` shows what's in step and what isn't; `colctl sync` fixes it; `colctl adopt` brings what a profile
already has into the source.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | One copy of each skill and agent on disk. | After `sync`, every managed skill/agent in every harness is a symlink resolving into `~/.colctl`; no file content is duplicated. |
| SC-2 | An edit to a skill or `AGENTS.md` reaches every Claude profile with no command. | Edit in `~/.colctl`, start each profile: the change is visible (manual smoke) — and the link/import structure is asserted in tests. |
| SC-3 | Plugins match across profiles. | After `sync`, every plugin in `config.toml` is listed in each claude-kind harness's `installed_plugins.json`. |
| SC-4 | Drift is visible. | `colctl status` exits non-zero and names the item and harness for each of: missing, broken, foreign, edited, plugin missing, uncommitted source change. |
| SC-5 | colctl never destroys data it doesn't own. | No test or smoke run ever removes or overwrites a file colctl can't prove it created; enforced by edge-case tests. |
| SC-6 | Day-one migration takes one command. | On the author's Mac, `colctl init` then `colctl adopt` brings all existing skills, plugins and the `CLAUDE.md` under colctl; a following `status` exits 0. |
| SC-7 | Install is one command. | `mise use -g github:muradmadi-leadtech/colctl` installs a working binary on macOS arm64 and Linux x86_64. |
| SC-8 | Test-heavy. | Every acceptance criterion has a test named after it; line coverage ≥ 90%; `just check` passes in CI. |

## Scope

**In:**
- `init`, `status` (`--json`), `sync`, `add`, `adopt`, `rm`; `--dry-run` on everything that changes something.
- The `claude` harness kind, for any Claude Code config folder (native or `CLAUDE_CONFIG_DIR`-style profile).
- Items: `AGENTS.md`, skills, agents, marketplaces, plugins (including Anthropic's plugin directory).
- Local git history of `~/.colctl`, auto-committed.
- Release binaries for macOS arm64 and Linux x86_64 with build attestations.
- Diataxis docs for everything shipped.

**Out (won't build this cycle):**
- Codex, OpenCode or any other harness kind.
- `commands/` (custom slash commands), MCP server config, hooks, `settings.json` keys of any kind.
- Plugin updates (`claude plugin update` exists).
- A git remote, push, or syncing `~/.colctl` between machines.
- Project-level `AGENTS.md`/`CLAUDE.md` files (only user-level).
- `skills/synced/` (claude.ai account skills).
- Windows. A GUI. A daemon or file watcher.
- A `harness add` command (edit `config.toml`; `init` re-detects).
