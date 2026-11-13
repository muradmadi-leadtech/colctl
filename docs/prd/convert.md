# Cycle 7: convert

## Problem

A source plugin whose repo only packages for Claude never reaches Codex. On the owner's machine `qdrant-skills` shows
`1 not packaged` on both Codex profiles: its repo has `.claude-plugin/` and a `skills/` folder, and nothing for Codex,
even though Codex reads the same `SKILL.md` files. Today the only way to get it into Codex is to repackage it by hand
and keep that copy up to date with every release.

Codex has become close to Claude's plugin format: the same skill format, a `hooks/hooks.json` with the same event
names, `CLAUDE_PLUGIN_ROOT` set for compatibility, and `Bash`, `Edit`/`Write` and `mcp__…` matchers that work as they
do in Claude ([Codex hooks](https://learn.chatgpt.com/docs/hooks.md)). The databricks repo shows the difference in
practice: its Codex package is its Claude package with a `.codex-plugin/plugin.json` and the hook matcher widened.

## Users

The owner, who runs Claude and Codex profiles side by side and wants a Claude-only plugin in Codex too, without
maintaining a fork.

## Decisions (owner, 2026-11-13)

- **Direction:** Claude → Codex. Codex → Claude waits until a Codex-only plugin shows up.
- **Opt in per source:** `colctl convert <source>` asks for it; nothing converts on its own. Status keeps saying
  `not packaged`, with that command as the fix.
- **What carries over:** skills and hooks. Everything else in the plugin (commands, agents, MCP servers, LSP servers,
  output styles) is listed as not carried over, never dropped silently.
- **Review:** a conversion is held like an update: `colctl review` shows the generated package and its risk flags
  (hooks run code) and `colctl approve` installs it. A new version of a converted source is held with its conversion.

## Success criteria

1. `colctl convert qdrant-skills`, then `colctl approve qdrant-skills`, installs `qdrant` in both Codex profiles with
   its 10 skills; `colctl status` shows both Codex lines without `not packaged`, and Codex can load the plugin.
2. A plugin with hooks is converted with every hook Codex supports, unchanged; hooks or matchers Codex never fires
   (e.g. `Notification`, a `Read` matcher) are listed in the convert output and in review.
3. Commands, agents, MCP servers and other parts of the plugin are listed as not carried over, by name.
4. `colctl update` on a converted source holds the new version and its new conversion together; review shows what
   changed in both; approve installs both.
5. The conversion can be undone (`colctl convert --undo <source>`): Codex profiles lose the plugin, status goes back
   to `not packaged`.
6. If the repo later ships its own Codex packaging, colctl installs that, drops its conversion, and says so.
7. Converting never writes into the clone, and the same commit always produces the same package, byte for byte.
8. `--json` and exit codes follow semver as before; `--dry-run` works for `convert` and `--undo`.

## Out of scope

- Codex → Claude, and other tools (Cursor, Copilot, Gemini).
- Turning commands into skills, agents into Codex agents, `.mcp.json` into Codex MCP config.
- Editing hook scripts, or rewriting `${CLAUDE_PLUGIN_ROOT}` (Codex sets it).
- Trusting hooks in Codex for the owner: Codex's own `/hooks` review stays the owner's step, and colctl says so.
- Plugins installed from a marketplace listing (`[plugins]`), not as a source: they have no clone to convert from.
- Loose skills and agents: colctl already links skills and converts agents (cycle 2).
