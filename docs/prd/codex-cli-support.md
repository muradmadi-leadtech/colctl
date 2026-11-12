# PRD: Codex CLI support

_Cycle 2 · Focus · 2026-11-12_

## Problem statement

colctl 1.0 keeps every Claude Code profile in step, but the author also runs Codex CLI: native (`~/.codex`) and
through kilo-proxy (`kilo-codex`, whose `CODEX_HOME` is
`~/Library/Application Support/kilo-proxy-headless/profiles/codex`). Codex reads the same kinds of things (a global
`AGENTS.md`, skills, custom agents, plugins), but from different places and partly in different formats, so today
they must be copied and kept in step by hand.

## Target users

The cycle 1 user, running one or more Codex CLI profiles next to their Claude profiles, on macOS (Apple silicon) or
Linux.

## What Codex reads (verified by probes, codex-cli 0.167.2; see `.forge/learnings.md`)

| Item | Location | Format |
|---|---|---|
| Global instructions | `$CODEX_HOME/AGENTS.md`, one per profile | Markdown; a symlink is followed |
| Skills | `$HOME/.agents/skills/<name>/`, **one folder shared by every Codex profile** (and the cross-tool Agent Skills location) | `SKILL.md`, same as Claude; symlinked folders are followed |
| Custom agents | `$CODEX_HOME/agents/<name>.toml` | TOML: `name`, `description`, `developer_instructions` (+ optional `model`) |
| Plugins | `codex plugin marketplace add|remove`, `codex plugin add|remove <name@marketplace>`; recorded in `$CODEX_HOME/config.toml` (`[marketplaces.<name>]`, `[plugins."<id>"]`) | Codex plugins, separate ids and marketplaces from Claude's |

## Solution summary

- **New harness kind `codex`** (path = a `CODEX_HOME`): colctl links `AGENTS.md` straight to `~/.colctl/AGENTS.md`,
  **generates** `agents/<name>.toml` from each `~/.colctl/agents/<name>.md`, and installs Codex plugins with
  `codex plugin …`, each with `CODEX_HOME` pointing at that profile.
- **New harness kind `agents`** (path `~/.agents`): the shared skills folder. colctl links each source skill into
  `~/.agents/skills`, next to skills other tools put there, which stay foreign and untouched.
- **A separate Codex plugin list** in `config.toml`, since Codex plugin ids and marketplaces differ from Claude's.
- `colctl init` also detects `~/.codex`, kilo's `profiles/codex` and `~/.agents`.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| SC-1 | One `AGENTS.md` for every Codex profile. | After `sync`, each codex harness's `AGENTS.md` is a symlink to the source's. |
| SC-2 | Skills reach Codex with one copy on disk. | After `sync`, every source skill is a symlink in `~/.agents/skills`; other entries there are unchanged byte for byte. |
| SC-3 | Agents reach Codex in Codex's format. | After `sync`, every source agent has a generated `.toml` in each codex harness whose `developer_instructions` equal the agent's body; editing the source agent and syncing updates it. |
| SC-4 | Codex plugins match across Codex profiles. | After `sync`, every plugin in the Codex list is `enabled = true` in each codex harness's `config.toml`. |
| SC-5 | Nothing regresses for Claude. | Every cycle 1 test still passes; a cycle 1 `config.toml` loads unchanged. |
| SC-6 | Day-one setup on the author's Mac. | Re-running `colctl init` registers `~/.agents` and the kilo Codex profile (and `~/.codex` if present); after `adopt` of what the user chooses, `status` reports only the skills left foreign on purpose. |
| SC-7 | Test-heavy, as in cycle 1. | Every new AC/EC has a test named after it; coverage ≥ 90%; `just check` passes in CI. Strict RED → GREEN this time: each test is committed failing before its code. |
| SC-8 | Docs. | Reference, how-to and explanation pages cover the two new kinds and the Codex plugin list. |

## Scope

**In:** harness kinds `codex` and `agents`; `AGENTS.md` link; agent `.md` → `.toml` generation; Codex marketplaces
and plugins via `codex plugin`; `init` detection; `status`/`sync`/`add`/`rm`/`adopt` for all of the above; env
scrubbing for `codex` calls (`CODEX_HOME`, `OPENAI_*`); docs.

**Out (won't build this cycle):**
- Codex Desktop, OpenCode or any other harness.
- Writing any `config.toml` key in a Codex profile (only `codex plugin` changes it).
- Converting Codex agents back to `.md`: a foreign `agents/*.toml` is reported, and `adopt` refuses it.
- Mapping Claude model names to Codex models (generated agents carry no `model`).
- Project-level `.agents/skills` or `.codex/` folders; MCP servers; disabling skills via `[[skills.config]]`.
- Managing the other tool that owns the `twg-*` skills (its `.skill-lock.json` is left alone).
