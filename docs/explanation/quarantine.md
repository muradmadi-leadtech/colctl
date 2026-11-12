# Quarantine

colctl installs other people's code into every agent harness on your machine: plugins cloned from their author's
repo, and skills that tools like twg keep in their own folders. You chose each of those once, deliberately. Quarantine
is about what happens next: **their next version**.

## Why hold new versions

A plugin can bring hooks that run on every tool call, MCP servers, and scripts. A skill can grant itself Bash, or run a
shell command when it loads. None of that is unusual, and a new version can add any of it. Before 2.0, `colctl update`
fetched a new commit and installed it in every Claude and Codex profile in one step: the first time you saw a change
was when every agent was already running it.

Holding new versions puts one step in between: you see what changed, then decide.

## What is held, and what isn't

| Comes from | Held? |
|---|---|
| A source plugin's new commit (`colctl update`) | Yes |
| A taken-over skill or agent's changed origin (`colctl refresh`) | Yes |
| A first-time `add source` or `adopt` | No: choosing it is already the explicit step |
| A plugin from a marketplace list (`[plugins]`, `[codex.plugins]`) | No: shown as `unreviewed` |

colctl can only hold back code whose source it owns. Marketplace plugins are installed and auto-updated by Claude
and Codex themselves, and colctl doesn't change their settings. `status` marks them `unreviewed` so you know. When a
plugin's source repo is known, `colctl adopt` turns it into a source plugin, and from then on its updates are held
too.

## What you approve is what you reviewed

A held source version is a commit, and commits can't change. colctl keeps a ref to it in the clone, so even an
upstream force-push can't take it away before you decide. A held skill is an exact copy in `.state/quarantine`, with
its hash recorded; if that copy changes, `review` and `approve` refuse it. If upstream or the origin moves on while
you're reviewing, `approve` still installs the version you looked at.

## What flags are, and aren't

Risk flags are pattern matches: this file is a hook, that one is executable, this frontmatter grants Bash. They tell
you where to look first. They don't scan for malware, check signatures or judge intent, and a version with no flags
can still do something you don't want (a skill's plain-text instructions are code for an agent). Quarantine gives
you the moment to look; the judgement is yours.

## Remembering "no"

`reject` remembers the version, so `update` and `refresh` don't keep offering it. When the author publishes something
newer, that's held as usual: rejecting one release doesn't block the plugin.
