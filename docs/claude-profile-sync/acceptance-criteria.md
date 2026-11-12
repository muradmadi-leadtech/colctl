# Acceptance criteria: Claude profile sync

_Cycle 1 · Refine · 2026-11-12_

Every criterion has at least one test whose name contains its ID in snake case (`ac_09_…`). Terms: **source** =
`~/.colctl` (or `$COLCTL_HOME`); **harness** = a registered Claude profile folder; exact formats are in
[interfaces.md](interfaces.md); edge cases are in [edge-cases.md](edge-cases.md).

## Config

**AC-01 Valid config loads**
```gherkin
Given a config.toml with two claude harnesses, one marketplace and two plugins
When any command runs
Then the config loads with "~" expanded to the user's home directory in every path
```

**AC-02 Invalid config is rejected before anything changes**
```gherkin
Given a config.toml with any of: an unknown key, an unknown harness kind, a duplicate harness name,
  or a plugin id not of the form name@marketplace
When any command runs
Then it exits 2
And the message names config.toml, the line and the offending field
And no file, state, git or claude action happens
```

**AC-03 COLCTL_HOME overrides the source location**
```gherkin
Given COLCTL_HOME=/some/dir
When any command runs
Then /some/dir is used as the source instead of ~/.colctl
```

## init

**AC-04 First init creates the source**
```gherkin
Given no source folder exists
When I run colctl init
Then the source contains config.toml, skills/, agents/, .state/ and a .gitignore listing .state/
And it is a git repo with exactly one commit, "colctl: init"
And no AGENTS.md is created
And the status report is printed afterwards
```

**AC-05 init registers the harnesses it finds**
```gherkin
Given ~/.claude exists and ~/Library/Application Support/kilo-proxy-headless/profiles/claude exists
When I run colctl init
Then config.toml has a harness "claude" (kind claude, path ~/.claude)
And a harness "kilo-claude" (kind claude, path ~/Library/Application Support/kilo-proxy-headless/profiles/claude)
And a known path that doesn't exist is not registered
```

**AC-06 Running init again only adds**
```gherkin
Given a source already initialised, with my own edits to config.toml
When a new known harness folder appears and I run colctl init
Then the new harness is appended to config.toml
And every existing line of config.toml, including comments, is unchanged
And git is not re-initialised
```

**AC-07 init --dry-run changes nothing**
```gherkin
Given no source folder exists
When I run colctl init --dry-run
Then it prints what it would create and which harnesses it found
And no file or folder is created
```

## status

**AC-08 status reports every kind of drift**
```gherkin
Given harnesses containing, between them, one item in each state:
  ok, missing, broken, foreign, edited, invalid, plugin-missing, marketplace-missing, harness-missing
And an uncommitted change in the source
When I run colctl status
Then each item is listed with its harness, its state and a one-line fix hint
And the uncommitted change is listed
And the exit code is 1
And with --json the same report is printed in the documented JSON shape
And nothing is written anywhere (no lock, no state, no git)

Given every item in every harness is ok and the source is committed
When I run colctl status
Then the exit code is 0
```

## sync

**AC-09 Skills are linked**
```gherkin
Given skills/foo/SKILL.md in the source and no skills/foo in a harness
When I run colctl sync
Then <harness>/skills/foo is a symlink whose target is the absolute path <source>/skills/foo
And status reports it ok
```

**AC-10 Agents are linked**
```gherkin
Given agents/reviewer.md in the source and no agents/reviewer.md in a harness
When I run colctl sync
Then <harness>/agents/reviewer.md is a symlink to <source>/agents/reviewer.md
```

**AC-11 CLAUDE.md imports AGENTS.md**
```gherkin
Given AGENTS.md in the source and no CLAUDE.md in a harness
When I run colctl sync
Then <harness>/CLAUDE.md has exactly the generated content from interfaces.md
And its sha256 is recorded for that harness in the state
```

**AC-12 Plugins are installed with Claude's CLI**
```gherkin
Given config.toml declares marketplace "sadr" = "github:muradmadi/sadr" and plugin "sadr@sadr"
And a harness where neither is installed
When I run colctl sync
Then colctl runs, in this order, with the harness's environment from interfaces.md:
  claude plugin marketplace add muradmadi/sadr
  claude plugin install sadr@sadr
And both are recorded as installed-by-colctl for that harness in the state

Given plugin "superpowers@anthropic-plugin-directory" and no marketplace entry for it
When I run colctl sync
Then only "claude plugin install superpowers@anthropic-plugin-directory" runs
```

**AC-13 Broken owned links are repaired**
```gherkin
Given <harness>/skills/foo is a symlink into the source but pointing at the wrong path or at nothing
And skills/foo exists in the source
When I run colctl sync
Then the link is replaced so it points at <source>/skills/foo
```

**AC-14 Removed source items are removed from harnesses**
```gherkin
Given <harness>/skills/foo is a symlink into the source and skills/foo no longer exists in the source
When I run colctl sync
Then the link is removed

Given a generated CLAUDE.md whose hash matches the state, and AGENTS.md no longer exists in the source
When I run colctl sync
Then CLAUDE.md is removed and its state entry is deleted
```

**AC-15 Foreign items are never touched**
```gherkin
Given <harness>/skills/foo is a real folder and skills/foo exists in the source
When I run colctl sync
Then <harness>/skills/foo is unchanged, byte for byte
And it is reported as foreign with the hint "colctl adopt <harness> skills/foo"
And the exit code is 1
```

**AC-16 Plugins removed from config are uninstalled only where colctl installed them**
```gherkin
Given plugin p was installed by colctl in harness A, and installed by hand in harness B
When p is removed from config.toml and I run colctl sync
Then "claude plugin uninstall p" runs for A only
And A's state entry for p is deleted
```

**AC-17 sync commits pending source changes first**
```gherkin
Given an uncommitted change in the source (for example a skill edited from inside a harness)
When I run colctl sync
Then before any harness is touched, the source is committed with the message "colctl: sync"

Given a clean source
When I run colctl sync
Then no commit is made
```

**AC-18 sync can target one harness**
```gherkin
Given harnesses "claude" and "kilo-claude"
When I run colctl sync --harness kilo-claude
Then only kilo-claude is changed

When I run colctl sync --harness nope
Then it exits 2 and lists the known harness names
```

**AC-19 --dry-run never changes anything**
```gherkin
Given any state in which a command would change something
When I run init, sync, add, rm or adopt with --dry-run
Then the planned actions are printed in order
And no file, link, state entry, lock, git commit or claude call is made
```

## add and rm

**AC-20 add skill**
```gherkin
Given a folder ./foo containing SKILL.md
When I run colctl add skill ./foo
Then <source>/skills/foo is a copy of ./foo
And the source is committed with "colctl: add skill foo"
And sync runs

Given a git URL and --subdir path/to/skill
When I run colctl add skill <url> --subdir path/to/skill
Then the repo is shallow-cloned to a temp folder, path/to/skill is copied without any .git,
  and the result is the same as adding a local folder
```

**AC-21 add agent**
```gherkin
Given a file ./reviewer.md
When I run colctl add agent ./reviewer.md
Then <source>/agents/reviewer.md is a copy, committed with "colctl: add agent reviewer", and sync runs
```

**AC-22 add plugin**
```gherkin
Given config.toml without plugin sadr@sadr
When I run colctl add plugin sadr@sadr --marketplace github:muradmadi/sadr
Then config.toml gains the marketplace and the plugin, with every existing line and comment unchanged
And it is committed with "colctl: add plugin sadr@sadr" and sync runs

When I run colctl add plugin foo@unknown without --marketplace
Then it exits 2 and nothing changes
```

**AC-23 rm**
```gherkin
Given skill foo, agent reviewer and plugin sadr@sadr are managed
When I run colctl rm skill foo (or rm agent reviewer, or rm plugin sadr@sadr)
Then the item is removed from the source or config.toml, committed with "colctl: rm <kind> <name>"
And sync removes it from every harness as in AC-14 and AC-16

When I run colctl rm skill nope
Then it exits 2 and nothing changes
```

## adopt

**AC-24 adopt a skill or agent**
```gherkin
Given <harness>/skills/foo is a real folder and the source has no skills/foo
When I run colctl adopt <harness> skills/foo
Then the folder is copied into <source>/skills/foo
And the copy's tree hash equals the original's
And only then is <harness>/skills/foo replaced by a link to the copy
And the source is committed with "colctl: adopt skill foo"
```

**AC-25 adopt with an existing source item**
```gherkin
Given the source already has skills/foo
And <harness>/skills/foo is a real folder with the same tree hash
When I run colctl adopt <harness> skills/foo
Then the folder is replaced by a link and nothing is copied

Given the folder's tree hash differs
Then nothing changes for that item, both paths are printed, and the exit code is 1
```

**AC-26 adopt CLAUDE.md**
```gherkin
Given <harness>/CLAUDE.md is a real file colctl didn't generate and the source has no AGENTS.md
When I run colctl adopt <harness> CLAUDE.md
Then its content becomes <source>/AGENTS.md, byte for byte
And <harness>/CLAUDE.md is replaced by the generated import

Given the source already has an AGENTS.md with different content
Then nothing changes, both paths are printed, and the exit code is 1
```

**AC-27 adopt plugins**
```gherkin
Given plugin p@m is installed in a harness and not in config.toml
And m's source in that harness's known_marketplaces.json is GitHub repo owner/repo
When I run colctl adopt <harness> plugin p@m
Then config.toml gains marketplace m = "github:owner/repo" (if absent) and plugin p@m
And the source is committed with "colctl: adopt plugin p@m"
And p@m is not recorded as installed-by-colctl in that harness
```

**AC-28 adopt everything**
```gherkin
Given foreign skills, agents, a CLAUDE.md and plugins across several harnesses
When I run colctl adopt with no arguments
Then every foreign item is adopted as in AC-24 to AC-27, one item at a time
And a summary lists each item as adopted, linked (already identical) or refused
And the exit code is 1 if any item was refused, otherwise 0
```

## Safety and environment

**AC-29 One colctl at a time**
```gherkin
Given a colctl command holding the lock
When a second command that changes something starts
Then it exits 3 and says which pid holds the lock
```

**AC-30 claude failures are reported in full**
```gherkin
Given the claude CLI exits non-zero for one plugin action in harness A
When I run colctl sync
Then the message includes the full command, its exit code and its stderr
And all other actions, in A and in other harnesses, still run
And the failed action is not recorded in the state
And the exit code is 1
```

**AC-31 claude missing**
```gherkin
Given no claude on PATH and plugins declared in config.toml
When I run colctl sync
Then all links and CLAUDE.md files are still synced
And each plugin action is reported as "skipped: claude not found"
And the exit code is 1
```

**AC-32 git missing**
```gherkin
Given no git on PATH
When any command except status runs
Then it exits 2 with "colctl needs git" before changing anything
```

## Release, docs and quality

**AC-33 Installable release**
```gherkin
Given a release made by release-please
When publish.yml runs
Then archives for aarch64-apple-darwin and x86_64-unknown-linux-gnu are attached to the release, each with a build attestation
And "mise use -g github:muradmadi-leadtech/colctl" installs a binary that prints its version
```

**AC-34 Docs exist for everything shipped**
```gherkin
Given cycle 1 is complete
Then docs/reference has a page per command, one for config.toml, one for exit codes and one for the state folder
And docs/tutorials has a first-run tutorial (init, adopt, sync)
And docs/how-to covers: add a Claude profile, adopt what you already have, add a plugin
And docs/explanation covers: links not copies, and what "owned" means
```

**AC-35 Test-heavy**
```gherkin
Given the Generate phase is complete
Then every AC-NN and EC-NN in this cycle has at least one test containing its ID
And line coverage is at least 90%, enforced by just check
And just check passes in CI
```

## Constraints (how to build, not what)

- Rust (edition 2024, toolchain pinned by sadr's `mise.toml`), single crate, `unsafe` forbidden.
- Crates: `clap`, `serde`, `serde_json`, `toml_edit` (keeps comments when editing `config.toml`), `anyhow`, `sha2`,
  `dirs`; tests: `assert_cmd`, `tempfile`, `predicates`. New dependencies need a reason in the PR.
- No libgit2, no HTTP client: shell out to `git` and `claude`.
- Plan/apply split; `plan` is pure and has no I/O.
- Never write `settings.json` or anything under a harness's `plugins/`.
- Tests never touch the real `HOME`, `~/.claude` or the network; the fake `claude` stands in for the real one.
- macOS arm64 and Linux x86_64. Unix symlinks only.
