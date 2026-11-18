# Cycle review: whole setup

_Cycle 10 · Evaluate · 2026-11-18_

| | |
|---|---|
| **Cycle** | `20261118-whole-setup` (FORGE, full: Focus → Orchestrate → Refine confirmed by the owner, Generate autonomous) |
| **Asked for** | Roadmap cycle 10: MCP servers, slash commands and hooks from one place in `~/.colctl`, in every Claude and Codex profile |
| **Scope delivered** | AC10-01 – AC10-71 and EC10-01 – EC10-30, each with a named test |
| **Released** | v2.7.0 (release-please, from the `feat` PRs below) |
| **PRs** | #141 Focus, #142 Orchestrate, #143 Refine, #144 T1, #146 T2, #147 T3, #148 T4, #149 T5, #150 T6, #151 T7, T8 (this) |
| **Disposition** | _Yours_ |

## What you get

- **MCP servers, declared once.** `[[mcp]]` in `config.toml`, with `[mcp.profile.<name>]` overrides for what differs.
  `colctl adopt mcp toolport` turns today's four hand-made copies into one entry with four `TOOLPORT_CLIENT_ID`
  overrides, without touching a profile. `colctl add mcp` / `rm mcp`; `status` shows `ok`, `missing`, `differs`,
  `taken`, `orphaned`, `foreign`, `unsupported`, `unreadable`; `sync --replace-mcp` takes over a name a profile already
  uses. colctl reads `.claude.json` and `config.toml` and writes only through `claude mcp` and `codex mcp`.
- **Secrets stay references.** `{ env = "NAME" }` stays `${NAME}` in Claude (it expands it at start). Codex can't keep
  a reference for a command server's env, so colctl passes the value at sync. `{ keychain = "item" }` is read with
  `security` once per sync. Resolved values are hidden in every output, and a literal that looks like a secret is
  never committed to `~/.colctl`.
- **Commands.** `~/.colctl/commands/<name>.md` is `/<name>` in both Claude profiles (a link) and a skill in
  `~/.agents` for Codex.
- **Hooks.** `~/.colctl/hooks/hooks.json` and its scripts become `setup@colctl-setup`, a plugin colctl builds and
  installs in every Claude profile, and in Codex through cycle 7's conversion. colctl still never writes
  `settings.json`; `colctl adopt <profile> hooks` copies yours out of it.
- **TUI.** Items lists MCP servers and commands (hooks as the `colctl-setup` source); `x` removes, `n` (or the
  palette's "add") opens a `colctl add …` line; Profiles' adopt offers foreign MCP servers.

## Smoke test (a copy of your profiles and `~/.colctl`, release build, real `claude` 2.1.327 and `codex` 0.169.0)

The copy left out session logs, caches and databases; links and the tools' plugin records were re-pointed into it.

1. `colctl status`: toolport `foreign` in all four profiles, not counted as needing you.
2. `colctl adopt mcp toolport` (13 s, most of it the sync after): one entry, shared `command` and
   `TOOLPORT_PROFILE`, four `TOOLPORT_CLIENT_ID` overrides; no `mcp add` or `remove` ran; toolport `ok` in all four.
   Each profile's toolport entry was compared before and after: **unchanged in all four**.
3. A second `colctl sync`: **0 changes**.
4. `colctl add command` (a sample) and a sample PreToolUse hook, then `colctl sync` (4.6 s): the command linked in
   both Claude profiles and written as a skill in `~/.agents`; `setup@colctl-setup` installed in all four profiles
   (`claude plugin list` shows it, version `0.0.0-<hash>`). Another sync: 0 changes, everything `ok`.
5. `config.toml` held no secret. The copy was deleted.

The sync after adopt also did 2.6.1's move of qdrant and superpowers to their `colctl-` marketplaces, which hasn't run
on your real profiles yet. It worked on the copy.

## Changed from the spec during Generate

- **Codex commands go to `~/.agents/skills`**, not into the `colctl-setup` plugin (T5): the folder Codex already reads
  every skill from, and the generated-file path colctl already has. A command named like a skill is `invalid` there
  (EC10-19). Recorded in interfaces.md.
- **Hooks show as `source colctl-setup`**, not a separate `hooks` kind (T6), because they reuse source plugins'
  install, update, reinstall and removal as they are. Recorded in interfaces.md.
- **`colctl adopt mcp <name>`** was added during the smoke test: adopting one server from every profile without
  adopting every other foreign item.

## Not done, or done differently

- **EC10-07:** a literal with `${…}` is passed as is (tested), but status doesn't add the note "Claude expands ${…}".
- **EC10-21:** a script keeps its mode (tested), but status doesn't flag a script that isn't executable.
- **AC10-50:** status doesn't mark hooks "runs code" when first added.
- **EC10-16:** documented ("start a new session to pick it up"); there's nothing to test in colctl.
- **AC10-61:** adding from the TUI is a typed `colctl add …` line, not a form with a field per flag.

## Strict TDD: the evidence

T1, T2, T3, T4, T5, T6 and T7 each opened on a RED commit whose new tests failed (T7's RED left out the key binding
and the new kinds so the tests compiled and failed), then GREEN. Test fixes made in GREEN commits, each a wrong
expectation rather than a code change: T1 (an override for a tool the entry excludes is an error by spec), T2 (an
unsupported hint shows with `status --all`), T3 (a server left `taken` makes `adopt` exit 1), T4 (a long error is
cut short on screen).

## Test results

`just check` passes: fmt, clippy `-D warnings`, the full suite including ignored tests, coverage **93.1% lines**,
cargo-deny, typos, shellcheck, gitleaks.

## Security review

- **Secrets.** colctl reads a secret only to pass it to `claude mcp add` or `codex mcp add`, and only at sync. It's
  never written by colctl, never in `.state` (state hashes the entry with references, not values), never in
  `--json`, `-v` or an error (each resolved value is replaced with `•••`, tested with a failing tool that echoes it).
  `Ctx` and the keychain cache print names only in `Debug`. Once passed, a value lives in the tool's own file, as
  when added by hand: that's documented.
- **Leak check.** Every commit colctl makes of `~/.colctl` checks `config.toml` for a literal that looks like a secret
  and refuses, naming the key, never the value. It runs before the commit, so nothing reaches history.
- **Hooks run code.** They're your own files, built into a plugin under `~/.colctl/plugins` (git-ignored). Codex
  still asks you to trust them in `/hooks`.
- **Reading profiles.** `.claude.json` and `config.toml` are parsed, never written. A file that doesn't parse stops
  only that profile's MCP items.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.7.0`, then `colctl --version` says 2.7.0
- [ ] `colctl sync` once (it also does 2.6.1's marketplace move)
- [ ] `colctl adopt mcp toolport`, then `colctl status`: toolport ok in all four profiles, and the four
      `TOOLPORT_CLIENT_ID`s are in `config.toml` as overrides
- [ ] Start a Claude and a Codex session: toolport's tools are there as before
- [ ] Optional: `colctl add command ./x.md` and check `/x` in Claude
