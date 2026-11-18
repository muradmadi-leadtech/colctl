# Hooks travel as a plugin

Claude keeps hooks you write yourself in `settings.json`, and there's no `claude` command that adds one. colctl's rule
is that it never writes a tool's settings files: it links what's identical, generates what must be converted, and lets
each tool's own CLI change its configuration. Writing hooks into `settings.json` would break that rule for every
profile at once, in a file you also edit by hand.

A plugin can carry hooks, and plugins already have a CLI. So colctl builds one, `setup@colctl-setup`, from
`~/.colctl/hooks`, and installs it like any source plugin: through `claude plugin marketplace add` and
`claude plugin install`. Its version is a hash of its content, so a change reinstalls it.

The same package suits Codex, which runs plugin hooks after you trust them, once it goes through the conversion that
`colctl convert` uses for Claude-only plugins.

Commands didn't need this: Claude reads `commands/` like `skills/`, so colctl links them. Codex has no commands, so
each becomes a skill in `~/.agents`, the folder Codex already reads skills from. MCP servers have CLIs in both tools
(`claude mcp`, `codex mcp`), so colctl uses those.
