# Codex and the shared skills folder

Codex reads the same three things as Claude Code (instructions, skills and agents), but not from the same places or
in the same formats. colctl follows the file layout each tool already uses.

## Why `~/.agents` is a harness of its own

Codex looks for user skills in `$HOME/.agents/skills`, not in its profile folder. Every Codex profile (`~/.codex`,
kilo's `profiles/codex`, any other `CODEX_HOME`) sees the same skills, and other tools that follow the Agent Skills
convention read that folder too. So it's registered once, as kind `agents`, rather than once per Codex profile.

Other tools put things there as well, such as their own skills and lock files. colctl only ever adds symlinks under
`skills/`, and reports everything else as foreign or `linked-elsewhere` without touching it.

## Why Codex instructions are a link but Claude's are an import

Codex reads `AGENTS.md`, the same name as the source file, so its profile's `AGENTS.md` can simply be a symlink to
`~/.colctl/AGENTS.md`. Claude Code reads `CLAUDE.md`, so it gets a two-line file that imports `AGENTS.md`.

## Why Codex agents are generated, not linked

A Claude agent is Markdown with YAML frontmatter; a Codex agent is TOML with `name`, `description` and
`developer_instructions`. The same `~/.colctl/agents/reviewer.md` is linked into Claude profiles as it is, and
**converted** into `agents/reviewer.toml` for each Codex profile:

- `name` comes from the frontmatter (or the file name), `description` from the frontmatter, and
  `developer_instructions` is the Markdown body.
- Claude-only keys such as `model`, `tools` and `color` are dropped. Model names differ between the tools, so a
  generated Codex agent uses Codex's default model.
- Each generated file starts with a comment pointing at its source, and colctl records its hash. If you edit the TOML
  by hand, it becomes `edited` and colctl stops rewriting it. If the `.md` changes, sync rewrites the TOML.

Conversion is one way: a hand-made Codex TOML can't become a `.md` reliably, so `adopt` refuses it.

## Why the plugin lists are separate

Claude and Codex plugins come from different marketplaces with different ids, even when they share a name
(superpowers is `superpowers@anthropic-plugin-directory` for Claude, `superpowers@superpowers-dev` for Codex). Each list
is installed with its own tool's CLI, in the profiles of that kind only.
