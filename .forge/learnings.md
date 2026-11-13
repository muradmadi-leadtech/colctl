# FORGE Learnings

Accumulated knowledge from development cycles.

## Patterns

<!-- Successful approaches to reuse -->

### Probe the real harness before designing
*Added: 2026-11-12*

Cycle 1 Focus probes (2026-11-12, Claude Code 2.1.322): symlinked skill folders are discovered in native ~/.claude and kilo profiles/claude; a CLAUDE.md containing only @~/path/AGENTS.md loads the import; enabledPlugins in settings.json does NOT install a plugin, but CLAUDE_CONFIG_DIR=<dir> claude plugin install <id> does (incl. superpowers@anthropic-plugin-directory with no claude.ai login); kilo-proxy-headless merges its keys into settings.json and keeps the rest.

### Approve release-please workflows via API
*Added: 2026-11-12*

GitHub holds workflows on PRs opened by the Actions token; gh api -X POST repos/<o>/<r>/actions/runs/<id>/approve releases them without the web button.

### Codex probes (cycle 2)
*Added: 2026-11-12*

2026-11-12, codex-cli 0.167.2 via kilo-codex: a symlinked $CODEX_HOME/AGENTS.md is read; a symlinked folder in $HOME/.agents/skills is discovered (skills are per-HOME, shared by every Codex profile); a TOML in $CODEX_HOME/agents/ (name, description, developer_instructions) is a usable custom agent. codex plugin marketplace add <src> records [marketplaces.<manifest-name>] in $CODEX_HOME/config.toml (name from the marketplace manifest, e.g. obra/superpowers -> superpowers-dev); codex plugin add <id> records [plugins."<id>"] enabled = true and caches under plugins/cache; remove commands delete both. openai/plugins is reserved (built in). kilo-proxy merges its own keys into profiles/codex/config.toml and keeps the rest.

### Push RED before GREEN
*Added: 2026-11-12*

Cycle 2: each feature PR's first commit (tests + compiling stubs) was pushed so CI recorded the failure, then GREEN was pushed and auto-merge enabled. Squash-merge keeps one commit per task on main; the CI history is the TDD evidence.

### Plugin-from-source probes (cycle 3)
*Added: 2026-11-12*

2026-11-12, claude 2.1.322 + codex-cli 0.167.2, obra/superpowers cloned locally: claude plugin marketplace add <clone> -> marketplace named from .claude-plugin/marketplace.json (superpowers-dev), source {source: directory, path}, installLocation = the clone itself; claude plugin install -> cached at the clone's commit (version + gitCommitSha). Following a new commit: claude plugin marketplace update <m> then claude plugin update <id> (works both directions). codex plugin marketplace add <clone> -> [marketplaces.<m>] source_type local; codex plugin add <id> installs; re-running codex plugin add reinstalls at the clone's current commit; marketplace upgrade is git-only. A colctl-written wrapper (own .claude-plugin/marketplace.json and .agents/plugins/marketplace.json, plugin source ./<name> symlinked to the clone) installs in both. Claude records a directory plugin's origin in plugins/plugin-directory-bindings.json (repositoryKey github.com/obra/superpowers#) and installed_plugins.json gitCommitSha; the directory copy lagged (6.4.2 vs 7.0.2).

### Smoke tests on real data find scope bugs
*Added: 2026-11-12*

Cycle 3's smoke on copies of real profiles found plugins installed in both user and project scope; unit and e2e fixtures never had that. Keep smoke tests on copies of real state before every release.

### Open the PR on the RED commit
*Added: 2026-11-12*

CI only runs on pull requests, so a RED commit pushed before the PR exists never shows as failing. Push RED, open the PR, wait for check to fail, then push GREEN (pushing GREEN early cancels the RED run).

### Smoke-test against real catalogs
*Added: 2026-11-13*

Fixture data that agrees with itself hides mismatches real data has (directory slugs vs plugin names). Run discovery-style features on a copy of the owner's real data before release.

## Anti-Patterns

<!-- Approaches to avoid -->

### Writing Claude plugin state by hand
*Added: 2026-11-12*

Files under plugins/ hold absolute paths and are owned by Claude; settings.json keys are shared with kilo. Drive plugins through the claude CLI instead.

### Writing tests alongside code in bulk
*Added: 2026-11-12*

Cycle 1 wrote tests with each module rather than one failing test at a time; traceability and coverage were high but TDD discipline was not strict. Next cycle: commit RED tests before GREEN code.

### Partial profile copies in smoke tests
*Added: 2026-11-12*

Copying only config.toml of kilo's Codex profile left out models.json (referenced by model_catalog_json), so the real codex failed. Copy every file a config references.

### Text replace after cargo fmt
*Added: 2026-11-12*

A python str.replace aimed at code cargo fmt had already reflowed silently did nothing (fake CLIs). Assert the old text exists before replacing, or use the Edit tool.

### Pushing docs without the gate
*Added: 2026-11-12*

Docs-only pushes still run typos and link checks; the cycle 4 plan PR failed on a crate name. Run just check before every push, docs included.

### Read-modify-write logs in test fakes
*Added: 2026-11-13*

Fakes that append to a log by rewriting it lose lines when colctl calls them in parallel; append in one write.

## Decisions

<!-- Key decisions and their rationale -->

### Pin first release with release-as
*Added: 2026-11-12*

release-please's release-as pinned v1.0.0 for cycle 1, then a follow-up PR removed it so later releases bump from commits.

### Future cycles from cycle 3 Focus
*Added: 2026-11-12*

Cycle 4: quarantine plugins/skills, human approves a new version before it installs. Later: convert a plugin for tools its repo doesn't package (e.g. Codex from a Claude-only plugin).

### in_sync excludes pending reviews
*Added: 2026-11-12*

Pending reviews raise status's exit code but not in_sync, so sync and approve aren't held back by other held versions.

## Tools

<!-- Useful tools and techniques -->

### FORGE skill
*Added: 2026-11-12*

github.com/scottfeltham/forge-skill (commit ffca6bf), installed at ~/.claude/skills/forge.

### Release-please and CI
*Added: 2026-11-12*

Its CHANGELOG holds short hashes that typos flags (exclude it), its PR runs need approval, and a chore commit doesn't refresh its PR: close and reopen it to rerun CI against current main.
