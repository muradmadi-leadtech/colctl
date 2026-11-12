# FORGE Learnings

Accumulated knowledge from development cycles.

## Patterns

<!-- Successful approaches to reuse -->

### Probe the real harness before designing
*Added: 2026-11-12*

Cycle 1 Focus probes (2026-11-12, Claude Code 2.1.322): symlinked skill folders are discovered in native ~/.claude and kilo profiles/claude; a CLAUDE.md containing only @~/path/AGENTS.md loads the import; enabledPlugins in settings.json does NOT install a plugin, but CLAUDE_CONFIG_DIR=<dir> claude plugin install <id> does (incl. superpowers@anthropic-plugin-directory with no claude.ai login); kilo-proxy-headless merges its keys into settings.json and keeps the rest.

## Anti-Patterns

<!-- Approaches to avoid -->

### Writing Claude plugin state by hand
*Added: 2026-11-12*

Files under plugins/ hold absolute paths and are owned by Claude; settings.json keys are shared with kilo. Drive plugins through the claude CLI instead.

## Decisions

<!-- Key decisions and their rationale -->

## Tools

<!-- Useful tools and techniques -->

### FORGE skill
*Added: 2026-11-12*

github.com/scottfeltham/forge-skill (commit ffca6bf), installed at ~/.claude/skills/forge.
