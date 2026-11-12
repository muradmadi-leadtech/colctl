# Links, not copies

colctl could have copied each skill into each profile. It links instead, and lets Claude Code itself install
plugins. Here's why.

## One copy on disk

A skill in `~/.colctl/skills/forge` appears in every profile as a symlink, `skills/forge → ~/.colctl/skills/forge`.
Claude Code follows the link and loads the skill as if it lived there. (This was checked against real profiles before
any code was written: both the native `~/.claude` and a kilo-proxy profile load a linked skill.)

That gives three properties copies can't:

- **Edits are instant.** Change a skill in `~/.colctl`, or ask Claude to improve a skill from inside any profile, and
  every profile sees the same file. Nothing needs syncing.
- **Nothing to drift.** There's no second copy to fall out of date.
- **Ownership is visible.** A link into `~/.colctl` is colctl's; anything else isn't (see [ownership](ownership.md)).

The cost: an edit made from inside a profile changes the source. That's usually what you want, and because
`~/.colctl` is a git repo, `colctl status` shows the change as uncommitted and `git -C ~/.colctl diff` shows exactly
what changed. `colctl sync` commits it.

## Instructions are imported, not linked

Claude Code reads `CLAUDE.md`, while the cross-tool convention is `AGENTS.md`. colctl writes a two-line `CLAUDE.md`:

```
<!-- Managed by colctl. Edit ~/.colctl/AGENTS.md instead; this file is regenerated. -->
@~/.colctl/AGENTS.md
```

Claude Code expands the `@` import when it loads the file, so the one real file is `~/.colctl/AGENTS.md` and future
harnesses that read `AGENTS.md` directly can use it as it is.

## Plugins go through `claude plugin`

A plugin is more than files. Claude Code records installed plugins and known marketplaces in JSON files that contain
absolute paths, keeps a cache per profile, and may change those formats in any release. Writing
`enabledPlugins` into `settings.json` doesn't install anything either (also checked before writing code).

So colctl stores only the intent (`config.toml` lists marketplaces and plugin ids) and runs
`claude plugin marketplace add` and `claude plugin install` for each profile, with `CLAUDE_CONFIG_DIR` pointing at it.
colctl reads Claude's JSON files to report status, but never writes them, and never touches `settings.json`, which a
launcher like kilo-proxy may also be editing.
