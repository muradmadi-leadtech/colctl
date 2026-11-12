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

## Anti-Patterns

<!-- Approaches to avoid -->

### Writing Claude plugin state by hand
*Added: 2026-11-12*

Files under plugins/ hold absolute paths and are owned by Claude; settings.json keys are shared with kilo. Drive plugins through the claude CLI instead.

### Writing tests alongside code in bulk
*Added: 2026-11-12*

Cycle 1 wrote tests with each module rather than one failing test at a time; traceability and coverage were high but TDD discipline was not strict. Next cycle: commit RED tests before GREEN code.

## Decisions

<!-- Key decisions and their rationale -->

### Pin first release with release-as
*Added: 2026-11-12*

release-please's release-as pinned v1.0.0 for cycle 1, then a follow-up PR removed it so later releases bump from commits.

## Tools

<!-- Useful tools and techniques -->

### FORGE skill
*Added: 2026-11-12*

github.com/scottfeltham/forge-skill (commit ffca6bf), installed at ~/.claude/skills/forge.
