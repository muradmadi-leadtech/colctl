# Roadmap: cycles 9–13

_Proposed 2026-11-15. Not agreed yet: the owner confirms, reorders or cuts before cycle 9's Focus._

## Where colctl is

2.5.0 keeps instructions, skills, agents and plugins the same across five profiles (Claude, Codex, kilo), holds
updates for review with a recommendation, converts Claude-only plugins for Codex, and has a TUI over the core loop.

What still makes it a tool you visit, not one you rely on:

1. **It's slow and unfinished in places.** `status` and `review` take about 14 s while big updates are held. Six
   commands have no TUI screen.
2. **Half the setup is still copied by hand.** The `toolport` MCP server is configured separately in `~/.claude.json`
   and `~/.codex/config.toml`. Slash commands, hooks and permissions aren't managed at all.
3. **Everything goes everywhere.** The work-only `twg` skills would reach a personal profile. Nothing says what a
   profile costs in context, or which skills are ever used.
4. **It's one machine.** `~/.colctl` has no remote. The CachyOS laptop starts from nothing.
5. **It only works when you run it.** Updates are found when you remember to look, and each one waits for you even
   when the recommendation is "approve".

Each cycle below removes one of these, in an order where each builds on the last.

| Cycle | Name | Removes | Release |
|---|---|---|---|
| 9 | solid | 1: speed, the TUI's missing screens, carried debt | minor |
| 10 | whole setup | 2: MCP servers, commands, hooks | minor |
| 11 | sets and cost | 3: who gets what, what it costs, what's unused | minor |
| 12 | machines | 4: the same setup on every machine | minor |
| 13 | autopilot | 5: checks, approves and syncs by policy; undo | minor |

Rules that carry over: CLI first (the TUI shows only what a command does); strict TDD with AC/EC-named tests; ≥ 90%
coverage; a smoke test on a copy of the real profiles; Focus, Orchestrate and Refine confirmed by the owner, Generate
autonomous.

---

## Cycle 9: solid

**Focus.** Make what exists fast and complete, so the daily loop never makes you wait or drop to the CLI.

- Success: `status` and `review` answer in under 1 s with large updates held (today about 14 s). Every command has
  a TUI screen or action. Every carried follow-up is closed or dropped on purpose.
- Out: new kinds of things to manage; themes.

**Orchestrate.**

| # | Task |
|---|---|
| T1 | Cache a held version's inspection (commits, files, flags, recommendation) per held commit; invalidate when the hold changes |
| T2 | The TUI opens on cached state and refreshes in the background |
| T3 | TUI screens for adopt, convert (and undo), rm, refresh, marketplaces and connectors, each with CLI parity tests |
| T4 | A command palette (`:` or Ctrl-K) over every action, fuzzy |
| T5 | Debt: `allowed-tools` flag noise grouped, `-v` paths as `~/…`, the `ETXTBSY` flake, connectors' empty fields, `sync`'s "needs attention" wording |
| T6 | Docs, smoke test, release |

**Refine: decisions for the owner.** Whether the palette replaces the Overview menu or sits beside it. Whether `init`
gets a first-run screen.

**Evaluate.** Timed on a copy with the two large updates held. Every screen walked in a real terminal.

---

## Cycle 10: whole setup

**Focus.** One list of MCP servers, slash commands and hooks in `~/.colctl`, in every profile that can use them.

- Success: `toolport` is declared once and present in all four Claude and Codex profiles. Adding a server is
  `colctl add mcp`. A command or hook added once reaches every profile, converted where the formats differ.
  `adopt` finds what each profile already has.
- Constraint kept: colctl doesn't hand-write the tools' settings files. It uses `claude mcp …` and `codex mcp …`.
  Focus must verify both CLIs can do everything needed.
- Secrets: an MCP server's env values are references (an env var name or a keychain item), never stored in
  `~/.colctl` or its git history.
- Out: permissions and other settings (see the decision below); writing MCP servers for tools that have no CLI for it.

**Orchestrate.**

| # | Task |
|---|---|
| T1 | Config and state: `[[mcp]]` (command or URL, args, env references, which tools) |
| T2 | Adapters: read each profile's MCP servers; add and remove through the tools' CLIs |
| T3 | `add mcp`, `rm mcp`, `adopt` for MCP; status, sync, `--json` |
| T4 | Secrets: references resolved at sync, a leak check in the commit path, review flags for a changed command |
| T5 | Slash commands: linked for Claude, converted to prompts for Codex |
| T6 | Hooks outside plugins: one list, reusing cycle 7's hook mapping for Codex |
| T7 | TUI: an MCP section in Items; docs; smoke test; release |

**Refine: decisions for the owner.** Whether permissions (`settings.json`) join in. Claude has no CLI for them, so
managing them means breaking the "never write settings" rule or leaving them out. Whether an MCP server's change is
held for review like a plugin update.

---

## Cycle 11: sets and cost

**Focus.** Decide what each profile gets, and see what it costs.

- Success: items carry tags (`work`, `personal`, or any name) and each profile lists the tags it takes, so `twg`
  reaches work profiles only. `colctl cost` shows, per profile, what loads at session start (instructions, skill and
  agent descriptions, plugin contents, MCP tools) in tokens, largest first. `colctl usage` reads the tools' own
  session logs on this machine and shows which skills, agents and MCP tools were used, and which never were.
- Out: per-project sets inside repos (a later cycle); sending usage anywhere.

**Orchestrate.**

| # | Task |
|---|---|
| T1 | Tags on items and `takes = […]` on profiles in config; the planner filters; an untagged item goes everywhere, as today |
| T2 | `colctl tag`, `colctl untag`; status and Items show tags; sync removes what a profile no longer takes (owned items only) |
| T3 | `colctl cost [--harness]`: token estimates per item and a total per profile; `--json` |
| T4 | `colctl usage [--since]`: reads Claude's and Codex's local transcripts, read-only; counts per item |
| T5 | Suggestions: unused and heavy items listed with the command that removes or untags them |
| T6 | TUI: a Cost view, tags in Items; docs; smoke test; release |

**Refine: decisions for the owner.** Token counts are estimates (characters ÷ 4) unless a tokenizer is added. How far
back `usage` looks by default. Whether transcripts on this machine may be read at all.

---

## Cycle 12: machines

**Focus.** The same setup on the work Mac and the CachyOS laptop, each keeping what's its own.

- Success: `colctl remote add <private repo>` then `colctl push`; on the other machine `colctl clone <repo>` then
  `colctl sync` gives the same skills, agents, plugins and MCP servers, filtered by that machine's tags. Profile
  paths and machine-only items live in a per-machine file that never leaves the machine. `status` says when the
  other machine is ahead.
- Depends on: cycle 10 (secrets are references, so nothing sensitive is pushed) and cycle 11 (tags decide what a
  machine takes).
- Out: live sync, a hosted service, merging conflicting edits automatically.

**Orchestrate.**

| # | Task |
|---|---|
| T1 | Split config into shared (`config.toml`) and machine (`machine.toml`, git-ignored): profiles, paths, tags taken |
| T2 | `remote add`, `push`, `pull`, `clone`: git underneath, fast-forward only, a clear stop on divergence |
| T3 | A pull is held like an update when it changes what runs code; plain edits apply |
| T4 | Linux paths for every tool; the two GitHub accounts (work and personal) documented and tested |
| T5 | A leak guard before every push: no secrets, no machine file, no credentials |
| T6 | TUI: a Machines line on Overview; docs (a second-machine tutorial); smoke test on a second home folder; release |

**Refine: decisions for the owner.** Whether work items may live in a repo the personal account can read, or the two
machines use two repos with a shared subset. Whether your own pushed change is held for review on the other machine.

---

## Cycle 13: autopilot

**Focus.** colctl looks after itself: it checks, approves what's safe, syncs, and tells you only when it needs you.

- Success: a scheduled check (launchd on macOS, a systemd timer on Linux) runs `update` and `pull`. A policy in
  config decides what happens next: `approve = "no-malware"` installs anything whose recommendation is approve,
  `approve = "never"` holds everything, per source. You're notified only for "look before approving" and for
  failures. `colctl log` shows what changed and when, and `colctl undo` takes back the last change everywhere.
- Depends on: cycle 9 (fast inspection) and the recommendation from 2.5.
- Out: a long-running daemon; auto-approving anything with a suspicious flag, ever.

**Orchestrate.**

| # | Task |
|---|---|
| T1 | Policy in config, global and per source; `approve --policy` applies it; the reason is recorded |
| T2 | `colctl schedule on|off|status`: installs and removes the timer; a run is one `colctl auto` |
| T3 | `colctl auto`: update, pull, apply the policy, sync; exits quietly when nothing needs you |
| T4 | Notifications: a system notification and a line on the next `colctl` run; nothing when all is well |
| T5 | `colctl log` and `colctl undo`: built on `~/.colctl`'s git history, re-syncing profiles to the earlier state |
| T6 | `colctl doctor`: tools found, versions, profiles readable, timer healthy, each problem with its fix |
| T7 | TUI: an Activity tab; docs; a week-long soak on the real machine; release |

**Refine: decisions for the owner.** The default policy for new installs (hold everything, as today). How widely the
malware patterns must be tested before auto-approve is offered. Whether `undo` may reinstall an older plugin commit
without a review.

---

## Not in these five

- **More tools** (OpenCode, Cursor, Gemini CLI, Copilot). Worth it once one of them is used daily. The adapter
  boundary already exists.
- **Per-project sets** inside repositories.
- **Codex → Claude conversion**, until a Codex-only plugin shows up.
- **Themes and a TUI config file.**
