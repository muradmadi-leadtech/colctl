# Acceptance criteria: whole setup

Tests are named after these ids (`ac10_01_…`). "The owner fixture" is cycle 6's: 5 profiles, fake `claude` and
`codex` on `PATH`. In this cycle the fakes also handle `mcp add|remove` by writing `.claude.json` / `config.toml` the
way the real tools do (shapes in interfaces.md), and log every call. "The toolport fixture" gives the four
Claude/Codex profiles today's toolport entries, with four different `TOOLPORT_CLIENT_ID`s.

## MCP config (T1)

**AC10-01 An entry with overrides parses**
```gherkin
Given config.toml with the toolport [[mcp]] entry and two [mcp.profile.*] overrides from interfaces.md
When colctl loads it
Then each of the four profiles' effective entry has the shared command and TOOLPORT_PROFILE and its own TOOLPORT_CLIENT_ID
And agents has no MCP servers
```

**AC10-02 References are kept as references**
```gherkin
Given env = { A = { env = "X" }, B = { keychain = "k" }, C = "plain" }
When the config is parsed and written back by add/rm of another entry
Then A and B are still the same references, C the same literal, and comments elsewhere are untouched
```

**AC10-03 Bad entries name the problem**
```gherkin
Given an entry with both command and url / neither / headers on a stdio server / an override for an unknown harness /
  a duplicate name / a name with spaces
Then loading fails with one error naming the entry and the field
```

## MCP in the planner (T2)

**AC10-10 A missing server is added through the tool's CLI**
```gherkin
Given the owner fixture with toolport in config and no servers in any profile
When colctl sync runs
Then the fake claude got `mcp add -s user toolport -e … -- <command>` once per Claude profile, with that profile's CLAUDE_CONFIG_DIR
And the fake codex got `mcp add toolport --env … -- <command>` once per Codex profile, with its CODEX_HOME
And colctl wrote neither .claude.json nor config.toml itself
```

**AC10-11 Every state is reported**
```gherkin
Given servers that are ok, missing, differs (config changed), differs (profile changed), foreign, taken, orphaned and unsupported
When colctl status and status --json run
Then each is reported with the state names from interfaces.md, and the exit code is 1 when anything needs a sync
```

**AC10-12 A change is remove then add**
```gherkin
Given toolport synced, then TOOLPORT_PROFILE changed in config
When colctl sync runs
Then each profile gets `mcp remove` then `mcp add` with the new value, and status is ok afterwards
```

**AC10-13 Foreign servers are left alone**
```gherkin
Given a profile with a server `orrery` that isn't in config
When colctl sync runs, and when colctl rm mcp orrery runs
Then sync doesn't touch it, and rm refuses: "orrery wasn't added by colctl"
```

**AC10-14 A taken name isn't overwritten without asking**
```gherkin
Given toolport in config and a different toolport already in a profile, not recorded
When colctl sync runs
Then it isn't replaced, status says taken, and names `colctl adopt` and `colctl sync --replace-mcp toolport`
When colctl sync --replace-mcp toolport runs
Then it is removed and added from config, and recorded
```

**AC10-15 Orphans are removed**
```gherkin
Given a recorded server removed from config.toml by hand
When colctl sync runs
Then it is removed from every profile it was recorded in, and forgotten
```

## MCP commands (T3)

**AC10-20 add mcp adds everywhere**
```gherkin
Given the owner fixture
When colctl add mcp github --env-from GITHUB_TOKEN=GH_TOKEN -- npx -y @mcp/github
Then config.toml has the entry with GITHUB_TOKEN = { env = "GH_TOKEN" }, it's committed, and every Claude and Codex profile has the server
And Claude profiles got `-e GITHUB_TOKEN=${GH_TOKEN}`; Codex profiles got the value of GH_TOKEN
```

**AC10-21 --only and --url**
```gherkin
When colctl add mcp docs --only claude --url https://docs.example/mcp --bearer-from DOCS_TOKEN
Then only Claude profiles get it, with header Authorization: Bearer ${DOCS_TOKEN}
And with --only codex instead, Codex profiles get --bearer-token-env-var DOCS_TOKEN
```

**AC10-22 rm mcp removes everywhere it was added**
```gherkin
Given github added by colctl
When colctl rm mcp github
Then the entry is gone from config, committed, and every profile got `mcp remove github`
```

**AC10-23 adopt turns four copies into one entry**
```gherkin
Given the toolport fixture and an empty config
When colctl adopt runs and the owner accepts toolport
Then config.toml has one toolport entry: shared command, args and TOOLPORT_PROFILE, and four profile overrides for TOOLPORT_CLIENT_ID
And no mcp add or remove was run, all four are recorded, and colctl status says toolport is ok in each
And a second colctl sync runs no tool call
```

**AC10-24 adopt with a server in only some profiles**
```gherkin
Given orrery only in the two Codex profiles, identical
When adopt accepts orrery
Then its entry has tools = ["codex"] and no overrides
```

**AC10-25 --dry-run and --json**
```gherkin
When add mcp, rm mcp and sync run with --dry-run
Then they print the plan, call no tool, and change no file
And status --json and sync --json carry "mcp" per profile; every existing field is unchanged (contract tests pass)
```

## Secrets (T4)

**AC10-30 An env reference in Claude stays a reference**
```gherkin
Given GITHUB_TOKEN = { env = "GH_TOKEN" } and GH_TOKEN unset in colctl's environment
When sync runs
Then Claude profiles get `${GH_TOKEN}` (it's Claude's environment that counts), and Codex profiles skip github:
  "codex: set GH_TOKEN, then colctl sync"
And the rest of the sync completes, exit code 1
```

**AC10-31 Keychain items are resolved at sync**
```gherkin
Given a fake `security` on PATH that returns "s3cret" for item "gh"
And GITHUB_TOKEN = { keychain = "gh" }
When sync runs
Then both tools got the value, and "s3cret" appears in no output, no log line, no file colctl wrote and no state hash input
```

**AC10-32 Secrets never reach ~/.colctl**
```gherkin
When colctl add mcp x --env API_KEY=ghp_abcdefghijklmnopqrstuvwxyz0123456789 -- cmd
Then it refuses with "That looks like a secret. Use --env-from API_KEY=NAME or --keychain API_KEY=ITEM." and config.toml is unchanged
Given the same literal written into config.toml by hand
When any colctl command commits (add, rm, adopt, approve)
Then the commit is refused, naming config.toml and mcp.x.env.API_KEY but not the value
```

**AC10-33 adopt keeps secrets out**
```gherkin
Given a profile server with env GITHUB_TOKEN = "ghp_…"
When adopt accepts it
Then config gets GITHUB_TOKEN = { env = "GITHUB_TOKEN" }, and adopt says "Set GITHUB_TOKEN in your shell, or
  store it with security add-generic-password -s colctl-github -w and switch to { keychain = "colctl-github" }"
```

**AC10-34 Redaction**
```gherkin
Given a resolved keychain value and a failing `codex mcp add` whose stderr echoes it
When sync runs with -v and --json
Then every occurrence is shown as •••
```

## Commands (T5)

**AC10-40 A command reaches every profile**
```gherkin
Given ~/.colctl/commands/standup.md with description frontmatter
When sync runs
Then each Claude profile has commands/standup.md linked to it
And each Codex profile's colctl-setup plugin has skills/standup/SKILL.md with name standup and that description
```

**AC10-41 add command, rm command**
```gherkin
When colctl add command ./standup.md, then colctl rm command standup
Then the file is copied, committed and synced; then removed, committed, and the links and the Codex skill are gone
```

**AC10-42 adopt a command**
```gherkin
Given ~/.claude/commands/notes.md as a plain file
When adopt accepts it
Then it moves to ~/.colctl/commands/notes.md and ~/.claude/commands/notes.md becomes a link to it
```

## Hooks (T6)

**AC10-50 Hooks reach Claude through colctl's plugin**
```gherkin
Given ~/.colctl/hooks/hooks.json with a PreToolUse Bash hook calling ${CLAUDE_PLUGIN_ROOT}/guard.sh
When sync runs
Then each Claude profile got `plugin marketplace add <colctl-setup folder>` and `plugin install setup@colctl-setup`
And the plugin folder has hooks/hooks.json and guard.sh, and status shows hooks ok with "runs code"
```

**AC10-51 Codex gets the mapped hooks**
```gherkin
Given hooks.json with PreToolUse Bash, Notification, and a prompt-type Stop hook
When sync runs
Then each Codex profile's colctl-setup has the PreToolUse group only, the report lists Notification and the prompt hook by name,
  and colctl says to trust them in Codex's /hooks
```

**AC10-52 A change reinstalls**
```gherkin
Given colctl-setup installed
When guard.sh changes and sync runs
Then the plugin's version changes and each profile gets the update/reinstall calls (as for source updates)
```

**AC10-53 adopt hooks from settings.json**
```gherkin
Given a profile's settings.json with hooks
When adopt accepts them
Then they're merged into ~/.colctl/hooks/hooks.json, settings.json is byte for byte unchanged,
  and adopt prints the "remove these yourself; until you do, they run twice" line naming the file
```

**AC10-54 Nothing to carry, no plugin**
```gherkin
Given no commands and no hooks
When sync runs
Then no colctl-setup marketplace or plugin is added, and a previously installed one is uninstalled and removed
```

## TUI (T7)

**AC10-60 Items shows the new kinds**
```gherkin
Given the toolport fixture adopted, one command, one hook
When the TUI opens on Items
Then MCP servers, Commands and Hooks sections list them with per-profile state, using cycle 8's tokens
```

**AC10-61 Actions match the CLI**
```gherkin
For add mcp (prompted name, command or URL, env as references), rm mcp, add command, rm command, adopt
When each runs from Items or the palette
Then the job calls the same core function as the CLI (parity test), asks first, shows the plan, ends in the result overlay
```

## Smoke and docs (T8)

**AC10-70 The real setup, copied**
```gherkin
Given a copy of the real four profiles and ~/.colctl, release build, real claude and codex
When colctl adopt (toolport), colctl status, colctl sync
Then status is in sync, the second sync runs no tool call, and each profile's toolport is byte for byte what it was
When a sample command and hook are added and sync runs
Then /<command> exists in both Claude profiles and the skill in both Codex profiles, and the hook is listed in Claude's /hooks
```

**AC10-71 Reference pages exist** for `add mcp`, `rm mcp`, `add command`, `rm command`, `[[mcp]]`, the hooks folder,
and the `adopt` changes, plus the how-to and explanation named in the tasks.
