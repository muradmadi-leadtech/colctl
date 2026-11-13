# Acceptance criteria: convert

Tests are named after these ids (`ac7_01_…`). "The qdrant fixture" is a source repo with only
`.claude-plugin/plugin.json` and `marketplace.json` (plugin source `./`), and `skills/<a>/SKILL.md`, `skills/<b>/SKILL.md`.
"The hooks fixture" adds `hooks/hooks.json`, `hooks/run.py`, `commands/setup.md`, `agents/helper.md` and `.mcp.json`.

## The package (T1, T2)

**AC7-01 A Codex manifest from Claude's**
```gherkin
Given the qdrant fixture whose plugin.json has name, version, description, author, homepage, repository, license and keywords
When colctl builds its Codex package
Then .codex-plugin/plugin.json has the same fields with the same values, "skills": "./skills/",
  and "interface": {"displayName": <name>}
And fields Claude's manifest doesn't have are left out, never invented
```

**AC7-02 What Codex reads is carried over, where it was**
```gherkin
When colctl builds a package that keeps a hook
Then every file of the plugin folder is in it at the same path, except .git, .claude-plugin and hooks/hooks.json
  (which is rewritten, AC7-04), so skills and the files hooks run are where they were
And when it keeps no hook, only the skills folder and the top-level README, LICENSE and NOTICE files are carried
  (smoke test: qdrant's plugin folder is its whole repo, CI and eval scripts included, which Codex never reads)
And the clone is unchanged (git status is clean, no file's mtime changes)
```

**AC7-03 The same commit gives the same package**
```gherkin
When the package for one commit is built twice, in different folders and at different times
Then both are byte-identical, file modes included, and have the same digest
```

**AC7-04 Hooks Codex runs are kept unchanged**
```gherkin
Given hooks.json with UserPromptSubmit, SessionStart (matcher "startup|clear"), PreToolUse (matcher "Bash"),
  PostToolUse (matcher "Edit|Write") and PreToolUse (matcher "mcp__github__.*"), each with a command hook
When the package is built
Then its hooks/hooks.json has those groups with the same matchers and commands, byte for byte in meaning
  (commands that use ${CLAUDE_PLUGIN_ROOT} are kept: Codex sets it)
```

**AC7-05 Hooks Codex never runs are reported, not kept**
```gherkin
Given hooks.json with a Notification hook, a PreToolUse group matching "Read|Grep", a PostToolUse group matching
  "Bash|WebFetch", and a hook of type "prompt"
When the package is built
Then Notification, the "Read|Grep" group and the prompt hook are left out
And the "Bash|WebFetch" group is kept with matcher "Bash"
And the report lists each: "Notification hook (Codex has no such event)", "PreToolUse on Read|Grep (Codex has no
  such tools)", "WebFetch in PostToolUse's matcher", "prompt hook in Stop (Codex runs command hooks only)"
```

**AC7-06 What doesn't carry over is named**
```gherkin
Given the hooks fixture
When the package is built
Then the report lists "command setup", "agent helper" and each MCP server in .mcp.json by name, under
  "Not carried over", and likewise LSP servers and output styles when the manifest declares them
```

## The command (T3, T4)

**AC7-07 convert holds a package for review**
```gherkin
Given the qdrant fixture added as a source and Codex profiles registered
When I run colctl convert qdrant
Then it prints "! Held qdrant's Codex package for review", the skills and hooks it carries, what isn't carried over,
  and "→ colctl review qdrant"
And it exits 0, installs nothing and leaves config.toml unchanged
And colctl status lists it under Needs attention as held, with "→ colctl review qdrant"
```

**AC7-08 review shows a held conversion**
```gherkin
When I run colctl review qdrant on a held conversion
Then it shows the commit it was built from, the Codex manifest, the skills, each kept hook with its event, matcher
  and command, risk flags (hooks run code), what Codex never runs, and what isn't carried over
And --json gives the same as one document with "kind": "conversion"
```

**AC7-09 approve installs it**
```gherkin
When I run colctl approve qdrant
Then config.toml's [[source]] for qdrant gets convert = ["codex"] and the change is committed in ~/.colctl
And plugins/.converted/qdrant holds the package and Codex profiles install qdrant@colctl-qdrant from it
And colctl status shows each Codex profile with "1 plugin" and no "not packaged"; --all and --json mark it converted
And when the package has hooks, the output says Codex asks to trust them in /hooks before they run
```

**AC7-10 reject drops it**
```gherkin
When I run colctl reject qdrant on a held conversion
Then nothing is installed, config.toml is unchanged and status still says "1 not packaged"
And colctl convert qdrant later holds it again
```

**AC7-11 undo**
```gherkin
Given qdrant is converted
When I run colctl convert --undo qdrant
Then Codex profiles uninstall it and remove the colctl-qdrant marketplace, plugins/.converted/qdrant is removed,
  convert is removed from config.toml (committed), and status says "1 not packaged" again
```

**AC7-12 dry runs**
```gherkin
When I run colctl convert qdrant --dry-run, or colctl convert --undo qdrant --dry-run
Then it prints what it would hold or remove and writes nothing (config, state, folders, tool calls)
```

## Sync, status and updates (T5, T6)

**AC7-13 Status points at convert**
```gherkin
Given a source with Claude packaging and none for Codex, not converted
When I run colctl status --all
Then each Codex item is unsupported with the fix "the repo has no Codex packaging; colctl convert <name> packages
  it"; it stays informational (exit 0)
```

**AC7-14 Sync keeps the package as approved**
```gherkin
Given qdrant is converted
When plugins/.converted/qdrant is deleted, or a file in it is edited
Then colctl status shows qdrant outdated in each Codex profile, and colctl sync rebuilds the package and reinstalls it
And when nothing changed, sync makes no tool calls
```

**AC7-15 Native packaging wins**
```gherkin
Given qdrant is converted
When an approved update adds .codex-plugin/plugin.json to the repo
Then sync installs the repo's own Codex packaging, removes the conversion and its convert field (committed),
  and prints "qdrant now packages for Codex itself; colctl's conversion is no longer used"
```

**AC7-16 Updates carry the conversion**
```gherkin
Given qdrant is converted
When colctl update holds a new commit
Then colctl review qdrant shows the source's changes and, under "Codex package", what changes in the conversion
  (files, hooks with risk flags, report)
And colctl approve qdrant installs the new commit in Claude profiles and its conversion in Codex profiles
And colctl reject qdrant keeps both at the old commit
```

**AC7-17 Machine output stays compatible**
```gherkin
When nothing is converted
Then every --json document is byte-identical to 2.2's (the AC6-01 goldens)
And when something is, the only additions are "convert" on sources, "converted": true on items, and the
  "conversion" candidate kind
```
