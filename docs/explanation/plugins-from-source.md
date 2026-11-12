# Plugins from source

A plugin is usually written once, in its author's git repo, and then reaches each tool its own way: Claude's
plugin directory, a marketplace that lists other people's plugins, a separate Codex marketplace. Those copies lag
and drift. On the author's Mac the plugin directory's superpowers was 6.4.2 while the repo was at 7.0.2.

A **source plugin** cuts out the middle: colctl clones the author's repo once into `~/.colctl/plugins/<name>` and has
every Claude and Codex profile install the plugin from that clone.

## Why the repo's own manifests

Many plugin repos ship packaging for several tools side by side (`.claude-plugin/`, `.codex-plugin/`,
`.agents/plugins/marketplace.json`). colctl adds the clone itself as a local marketplace, so each tool installs exactly
what the author published for it, under the author's id (`superpowers@superpowers-dev`). If a repo has only plugin
manifests, colctl writes a one-plugin marketplace manifest next to the clone (in `plugins/.manifests/`, never in the
clone). A tool the repo doesn't package for gets nothing; converting packaging between tools is a possible later
cycle.

## Why pins

`config.toml` records an exact commit for each source, and nothing moves it except `colctl update`. That means:

- `status` and `sync` never touch the network and never change what code runs;
- a new version arrives only when you ask, as a commit you can read and revert;
- adopting a plugin pins it to the commit already installed, so switching to the source and taking new code are
  separate steps.

This is also what a later "quarantine" cycle builds on: review a new version before `update` installs it.

## What stays the tools' business

Each tool still keeps its own cache of an installed plugin; colctl can't share that. And colctl never edits a clone:
one with local changes is reported and left alone until you clean it up.
