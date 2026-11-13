# Converting plugins

Claude Code and Codex read nearly the same plugins. Skills are the same `SKILL.md` folders. Hooks use the same
`hooks/hooks.json` shape and most of the same events, and Codex sets `CLAUDE_PLUGIN_ROOT` for hook commands written
for Claude. A plugin repo that ships a `.codex-plugin/plugin.json` next to its `.claude-plugin/plugin.json` works in
both; many only ship the Claude one.

`colctl convert` writes the missing part for you, and nothing else:

- **Codex's manifest** is made from Claude's: the same name, version, description, author, links, license and
  keywords, plus where the skills are. Nothing is invented.
- **Hooks are copied, never rewritten.** A hook Codex runs works as written; one it can't run is left out and named,
  not translated into something that might behave differently. Changing commands would mean colctl deciding what
  someone else's code should do.
- **What has no Codex equivalent is named, not dropped silently.** Commands, agents, MCP servers, LSP servers and
  output styles stay Claude-only.

## Why it's held for review

A conversion installs someone else's code into a tool it wasn't packaged for. You review it as you review an update
([Quarantine](quarantine.md)): the package is built, held, shown with its risk flags, and installed only when you
approve. Codex then asks again before running any hook.

## Why it's a function of the commit

The package is built from the pinned commit alone: the same commit gives the same bytes. So approving a commit
approves its conversion, and `config.toml` only needs `convert = ["codex"]` next to the pin. When an update is held,
its package is built too and shown in the same review; when the repo starts packaging for Codex itself, its own
packaging wins.

The package lives in `~/.colctl/plugins/.converted/<source>`, next to the clone it's built from, and is rebuilt by
sync whenever it's missing or doesn't match the approved commit.
