# Acceptance criteria: Plugins from source

_Cycle 3 · Refine · 2026-11-12_

IDs are `AC3-NN`; each needs a test whose name contains `ac3_NN`. Formats are in [interfaces.md](interfaces.md),
edge cases in [edge-cases.md](edge-cases.md). In tests, "upstream" is a local git repo reached by a `file://` URL.

## Config

**AC3-01 Source plugins load**
```gherkin
Given a config.toml with a [[source]] entry: name "superpowers", repo "https://github.com/obra/superpowers",
  rev "<40 hex characters>", replaces ["superpowers@anthropic-plugin-directory"]
When any command runs
Then the source loads
And an entry with a duplicate name, a rev that isn't 40 hex characters, an invalid repo, an invalid plugin name or
  an invalid replaces id is rejected with exit 2, naming the line
```

**AC3-02 Older configs are unchanged**
```gherkin
Given a config.toml written by colctl 1.0 or 1.1
When any command runs
Then it loads with no sources, and [plugins] / [codex.plugins] behave exactly as in 1.1
```

**AC3-03 Excluding a source from one harness**
```gherkin
Given a claude or codex harness with exclude = ["sources/superpowers"]
When I run colctl sync
Then nothing for that source is installed, updated or reported in that harness
```

## The clone (vault)

**AC3-04 sync clones a declared source at its pin**
```gherkin
Given a [[source]] whose clone doesn't exist yet
When I run colctl sync
Then ~/.colctl/plugins/<name> is a git clone of the repo with HEAD at the pinned rev
And the source's .gitignore lists "plugins/", so clones never enter ~/.colctl's history
```

**AC3-05 The clone is kept at the pin**
```gherkin
Given a clone that was moved to another commit, or whose pinned commit isn't present locally yet
When I run colctl sync
Then the clone is checked out at the pinned rev again, fetching first if the commit is missing
```

**AC3-06 A clone with local changes is never touched**
```gherkin
Given a clone with modified or untracked files
When I run colctl sync
Then the clone is left exactly as it is
And the source is reported invalid ("the clone at <path> has local changes") in status and sync
And nothing for that source is installed or updated in any harness, and the exit code is 1
```

## Packaging

**AC3-07 The repo's own manifests decide the ids**
```gherkin
Given a repo with .claude-plugin/marketplace.json and .agents/plugins/marketplace.json, each listing plugin "superpowers"
  under marketplace "superpowers-dev"
Then the source installs as superpowers@superpowers-dev in claude harnesses and in codex harnesses
And each tool's marketplace is the clone folder itself
```

**AC3-08 A repo without a marketplace manifest gets one from colctl**
```gherkin
Given a repo with only .claude-plugin/plugin.json and .codex-plugin/plugin.json (name "tool")
When I run colctl sync
Then colctl writes a one-plugin manifest for each tool in ~/.colctl/plugins/.manifests/<name>/ (never in the clone),
  under marketplace "colctl-<name>"
And the plugin installs as tool@colctl-<name> in both kinds
```

**AC3-09 Tools the repo doesn't package**
```gherkin
Given a repo with Claude packaging only
When I run colctl sync and colctl status
Then claude harnesses get the plugin
And codex harnesses show the source as "unsupported" (with "the repo has no Codex packaging"), get no codex calls for it,
  and "unsupported" alone doesn't make the exit code 1
```

## Installing and updating

**AC3-10 sync installs from the clone**
```gherkin
Given a source at its pin and harnesses where it isn't installed
When I run colctl sync
Then each claude harness runs, in order: claude plugin marketplace add <marketplace folder>, claude plugin install <id>
And each codex harness runs: codex plugin marketplace add <marketplace folder>, codex plugin add <id>
And the pinned rev is recorded as installed for that source in each harness
And a second sync makes no calls
```

**AC3-11 A moved pin updates every harness**
```gherkin
Given a source installed at rev A in every harness, and the pin changed to rev B
When I run colctl sync
Then the clone is moved to B
And each claude harness runs claude plugin marketplace update <marketplace>, then claude plugin update <id>
And each codex harness runs codex plugin add <id>
And the recorded rev becomes B only where those calls succeeded
```

**AC3-12 replaces removes the aggregator's copy**
```gherkin
Given a source with replaces = ["superpowers@anthropic-plugin-directory"], installed by hand in two claude harnesses
When I run colctl sync
Then claude plugin uninstall superpowers@anthropic-plugin-directory runs in both
And the marketplace it came from is not removed
```

**AC3-13 rm source**
```gherkin
When I run colctl rm source superpowers
Then its [[source]] entry is removed and committed as "colctl: rm source superpowers"
And sync uninstalls its plugin and removes its local marketplace in every harness where colctl installed them
And ~/.colctl/plugins/superpowers (and its colctl-written manifest, if any) is deleted
```

## update

**AC3-14 update --check reports, changes nothing**
```gherkin
Given upstream has commits newer than the pin for one source and none for another
When I run colctl update --check
Then it fetches, prints "<name>: <old short> → <new short> (<n> new commits)" for the first and "up to date" for the second
And config.toml, the clones' checkouts and every harness are unchanged
And the exit code is 1 (updates available); 0 when none are
```

**AC3-15 update moves pins**
```gherkin
When I run colctl update (or colctl update superpowers)
Then each named source (all, by default) is pinned to upstream's default-branch head
And config.toml is committed as "colctl: update source <name> to <short rev>" per changed source
And sync then updates every harness as in AC3-11
```

**AC3-16 update --rev**
```gherkin
When I run colctl update superpowers --rev v7.0.1
Then the tag (or branch, or commit) is resolved after fetching and the full commit is pinned
And --rev with no source name or several is rejected with exit 2, as is a rev that doesn't resolve
```

## add

**AC3-17 add source**
```gherkin
When I run colctl add source https://github.com/obra/superpowers [--rev <commit-ish>] [--name <name>] [--plugin <plugin>]
Then the repo is cloned, pinned to --rev or the default-branch head, and its packaging is checked
And a [[source]] entry is added and committed as "colctl: add source <name>", and sync runs
And a repo with no Claude or Codex packaging for the plugin is rejected with exit 2, leaving no clone behind
```

## adopt

**AC3-18 adopt a plugin-directory plugin moves it to its source**
```gherkin
Given superpowers@anthropic-plugin-directory installed in a claude harness at gitCommitSha C, with
  plugin-directory-bindings.json pointing at github.com/obra/superpowers
When I run colctl adopt <harness> plugin superpowers@anthropic-plugin-directory
Then a [[source]] "superpowers" is added with repo https://github.com/obra/superpowers, rev C,
  replaces ["superpowers@anthropic-plugin-directory"], and committed as "colctl: adopt plugin superpowers@anthropic-plugin-directory from source"
And it is removed from [plugins] if it was listed there
And sync installs superpowers@superpowers-dev at C in every harness the repo packages for, and uninstalls the
  directory copy everywhere (SC-3)
```

**AC3-19 adopt a marketplace plugin moves it to its source**
```gherkin
Given p@m installed from marketplace m
When m's manifest gives p's source as another repo, the source is that repo
When it gives a path inside m's repo, the source is m's repo (with plugin p)
Then adopt creates the [[source]] as in AC3-18, pinned to the installed commit if the tool recorded one, otherwise
  to the commit m's checkout is at
```

**AC3-20 adopt a Codex plugin moves it to its source**
```gherkin
Given p@m installed in a codex harness, with [marketplaces.m] source_type "git"
When I run colctl adopt <harness> plugin p@m
Then the source is found from m's source and manifest as in AC3-19, and p@m is removed from [codex.plugins] if listed
```

**AC3-21 The same plugin from an aggregator in more harnesses**
```gherkin
Given a source already exists for a repo, and another harness has that plugin from an aggregator under another id
When I run colctl adopt (or adopt that item)
Then the other id is added to that source's replaces; no second source or clone is created
```

**AC3-22 No source found: 1.1 behaviour**
```gherkin
Given a plugin whose source can't be determined
When I run colctl adopt <harness> plugin <id>
Then it is adopted into [plugins] / [codex.plugins] exactly as in 1.1
```

## Revise: plugins already in a list

Added after Evaluate on the owner's real profiles: superpowers had been adopted into `[plugins]` in cycle 1, so it
showed as `ok` and `adopt` refused it. A plugin colctl already manages through a list must be able to move to its
source in one step.

**AC3-29 adopt converts a list plugin whose source is known**
```gherkin
Given p@m is in [plugins] (or [codex.plugins]) and installed in a harness, and its source can be found
When I run colctl adopt <harness> plugin p@m (or colctl adopt)
Then a [[source]] is added as in AC3-18 (pinned to the installed commit, replaces [p@m]),
  and p@m leaves the list in the same commit
And one sync, in each harness, uninstalls p@m and installs the source plugin, with no sync in between where the
  harness has neither
And p@m is uninstalled once per harness, whether colctl or the user installed it there
```

**AC3-30 status says a list plugin has a source**
```gherkin
Given a list plugin, installed and ok, whose source can be found
When I run colctl status
Then it stays "ok" (the exit code isn't affected), with the hint
  "source available: <repo>; colctl adopt <harness> plugin <id>"
```

## status

**AC3-23 from-aggregator**
```gherkin
Given a plugin installed in a harness, in no colctl list and no source's replaces, whose source can be determined
When I run colctl status
Then it is reported "from-aggregator" with the hint "source: <repo>; colctl adopt <harness> plugin <id>"
```

**AC3-24 Source items and the sources section**
```gherkin
When I run colctl status (and --json)
Then each claude/codex harness lists each source as an item of kind "source": ok, missing, outdated (installed at another
  commit), unsupported or invalid
And --json has a top-level "sources" list: name, repo, rev, clone state ("ok", "missing", "moved", "dirty", "invalid")
```

**AC3-25 status and sync stay offline**
```gherkin
Given every source cloned and at its pin
When the upstream repos become unreachable and I run colctl status and colctl sync
Then both succeed exactly as before; only update, adopt, add source and a first clone ever contact upstream
```

## Cross-cutting

**AC3-26** Every cycle 1 and cycle 2 test still passes (`tests/cli.rs`, `tests/codex.rs`, `tests/takeover.rs` keep all their tests).

**AC3-27** Docs: reference (`[[source]]` in config.md, add.md, rm.md, update.md, adopt.md, status.md), how-to (move a plugin to its source; update plugins), explanation (plugins from source).

**AC3-28** Every AC3-NN and EC3-NN has a test containing its ID, checked in CI with cycles 1 and 2.

## Constraints

- Same as cycles 1–2. Clones are made and moved only with the `git` CLI; colctl never edits files inside a clone.
- Network access only in `update`, `adopt`, `add source` and the first clone of a source; tests never touch the network.
- Strict TDD: each task branch's first pushed commit holds its tests, failing in CI.
