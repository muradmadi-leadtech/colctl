# Hooks

Put hooks once in `~/.colctl/hooks/hooks.json`, in Claude's plugin hooks format, with any scripts they run beside it:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Bash", "hooks": [{ "type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/guard.sh" }] }
    ]
  }
}
```

Sync builds them into a plugin of colctl's own, `setup@colctl-setup`, in `~/.colctl/plugins/.setup` (not committed),
and installs it in every Claude profile. Codex profiles get the same plugin converted the way
[`colctl convert`](convert.md) converts: the events and matchers Codex runs are kept, the rest are left out. Codex asks
you to trust hooks before they run: open Codex and use `/hooks`. When Codex runs none of them, status says
`unsupported` for Codex.

A script keeps its mode: make it executable in `~/.colctl/hooks`. A change to any file there changes the plugin's
version, and sync reinstalls it. Removing `hooks/` uninstalls it. A `hooks.json` that doesn't parse is `invalid`, and
what's installed stays as it was.

Status lists the plugin as `source colctl-setup`. Bring hooks you already have in a profile's `settings.json` over with
`colctl adopt <profile> hooks` ([adopt](adopt.md)). Why a plugin: [hooks travel as a plugin](../explanation/hooks-travel-as-a-plugin.md).
