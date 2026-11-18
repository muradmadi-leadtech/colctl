# PRD: whole setup

_Cycle 10 · Focus · 2026-11-18_

## Problem statement

colctl keeps instructions, skills, agents and plugins the same in every profile. The rest of a setup is still
copied by hand:

1. **MCP servers.** `toolport` is configured four times: in `~/.claude.json`, the kilo Claude profile's
   `.claude.json`, `~/.codex/config.toml` and the kilo Codex profile's `config.toml`. Each copy has the same command
   and `TOOLPORT_PROFILE`, and its own `TOOLPORT_CLIENT_ID` (`claude-code`, `claude-code@f32399132c8e1a62`, `codex`,
   `codex@710cc12e3086d30b`). Adding a second server means editing four files in two formats.
2. **Slash commands and hooks outside plugins.** There's nowhere to put a command or hook once and have it reach every
   profile. None exist on this machine today, partly because there's no good place for them.

## Target users

- **The owner**, on the work Mac, keeping 5 profiles in sync (4 of them take MCP servers, commands and hooks:
  `claude`, `kilo-claude`, `codex`, `kilo-codex`; `agents` takes none of them).
- Scripts and CI using the CLI and `--json`. Existing output must not change shape; new fields are additions.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| S1 | `toolport` is declared once in `~/.colctl` and present in all four Claude and Codex profiles, each with its own `TOOLPORT_CLIENT_ID` | Smoke test on a copy of the real profiles: `adopt` then `status` says in sync, and `sync` changes nothing |
| S2 | `adopt` finds the MCP servers each profile already has and writes one entry per server, with per-profile overrides for the values that differ | A test with four fixture profiles shaped like today's toolport |
| S3 | `colctl add mcp <name> -- <command> [args]` (or `--url`) adds a server to every profile that can use it; `colctl rm mcp <name>` removes it; `status` and `sync` cover MCP drift; `--json` includes it | Tests per command and drift case |
| S4 | An MCP server's secret values are references (an environment variable or a macOS keychain item), resolved at sync. They are never written to `~/.colctl` or its git history | Tests: config and history hold only the reference; a commit with a literal secret is refused |
| S5 | A command file put once in `~/.colctl` is a slash command in every Claude profile and the closest equivalent in every Codex profile | Tests per tool; smoke test with a sample command |
| S6 | A hook put once in `~/.colctl` runs in every Claude profile and, mapped like cycle 7's conversion, in every Codex profile, with what Codex can't run listed by name | Tests per event and matcher |
| S7 | colctl still never writes a tool's settings files itself: MCP servers go through `claude mcp` and `codex mcp`, and commands and hooks through a plugin colctl generates and installs | Tests: the fake tools' config files change only through the fake CLIs |
| S8 | colctl never removes an MCP server, command or hook it can't prove it added | Tests with foreign entries of each kind |
| S9 | The TUI shows MCP servers, commands and hooks in Items, with their actions in the palette | A parity test per action |

## How it fits colctl's rules

- **MCP servers:** read the profile's config file directly, read-only (`claude mcp list` and `get` start every server
  to health-check it, which is slow and runs code). Write only through `claude mcp add|remove -s user` and
  `codex mcp add|remove`, pointed at the right profile the way plugins already are.
- **Commands and hooks:** Claude keeps loose hooks in `settings.json`, which colctl doesn't write. So colctl packages
  `~/.colctl`'s commands and hooks as a plugin of its own, in its own marketplace, and installs it like any other.
  For Codex, the same plugin goes through cycle 7's conversion (hooks mapped; commands become whatever Codex supports,
  verified in Refine).
- **Ownership:** colctl records what it added, in `.state`, and touches only that, as for plugins.

## Boundaries

**In:** MCP servers (stdio and HTTP) in user scope, with per-profile overrides and secret references; slash commands;
hooks outside plugins; `add`/`rm`/`adopt`/`status`/`sync`/`--json` for all three; Items and palette in the TUI; docs.

**Out:**
- Permissions and other `settings.json` settings: colctl keeps its rule of never writing a tool's settings.
- Holding an MCP change for review: every entry is the owner's own edit. Changes pulled from another machine are
  held in cycle 12.
- Project-scoped servers (`.mcp.json` in a repo), and OAuth logins (`claude mcp login` stays the owner's).
- Carrying MCP servers or commands from third-party plugins into Codex (cycle 7's conversion still lists them as not
  carried over).
- The `agents` profile kind: `~/.agents` has no MCP servers, commands or hooks.
- Tools without an MCP CLI.

## Constraints

- CLI first: the TUI shows only what a command does, through `src/core`.
- Tests never touch the real profiles; fake `claude` and `codex` record what they're asked to do.
- Strict TDD, AC/EC-named tests, ≥ 90% coverage, a smoke test on a copy of the real profiles.

## To verify in Refine

- Whether Claude expands `${VAR}` in a user-scope server's env. If it does, an environment-variable secret can stay
  a reference inside `~/.claude.json` too. If not, the resolved value lands there, as it does today when added by hand.
- What Codex 0.169 has for user commands. It doesn't seem to have `~/.codex/prompts` any more, so a command would
  probably become a skill.
- That a hook delivered in a plugin behaves like one in `settings.json` in Claude, and in Codex after conversion.
- How `codex mcp` reaches the kilo Codex profile (`CODEX_HOME`), and `claude mcp` the kilo Claude profile
  (`CLAUDE_CONFIG_DIR`).
