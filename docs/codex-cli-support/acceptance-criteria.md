# Acceptance criteria: Codex CLI support

_Cycle 2 · Refine · 2026-11-12_

IDs are `AC2-NN` so they never clash with cycle 1's; each needs at least one test whose name contains `ac2_NN`.
Formats are in [interfaces.md](interfaces.md), edge cases in [edge-cases.md](edge-cases.md). "Codex harness" means a
harness of kind `codex` (a `CODEX_HOME`); "agents harness" means kind `agents` (the shared `~/.agents` folder).

## Config

**AC2-01 New kinds load**
```gherkin
Given a config.toml with harnesses of kind "claude", "codex" and "agents"
When any command runs
Then the config loads
And an unknown kind is rejected with exit 2, naming the line and listing "claude", "codex" and "agents"
```

**AC2-02 The Codex plugin list**
```gherkin
Given [codex.marketplaces] superpowers-dev = "github:obra/superpowers"
And [codex.plugins] enabled = ["superpowers@superpowers-dev", "x@openai-curated"]
When any command runs
Then both plugins are in the Codex list and neither is in the Claude list
And a Codex plugin whose marketplace is neither in [codex.marketplaces] nor "openai-curated" is rejected with exit 2

Given a config.toml written by colctl 1.0
When any command runs
Then it loads with no Codex plugins and behaves exactly as in 1.0
```

**AC2-03 Excludes are checked per kind**
```gherkin
Given a codex harness may exclude AGENTS.md, agents/<name>.toml and plugins/<id>
And an agents harness may exclude skills/<name>
When an exclude entry isn't valid for its harness's kind (e.g. CLAUDE.md on a codex harness)
Then the config is rejected with exit 2, naming the line
```

## init

**AC2-04 init finds Codex profiles and ~/.agents**
```gherkin
Given ~/.codex, ~/Library/Application Support/kilo-proxy-headless/profiles/codex and ~/.agents exist
When I run colctl init (first run or again)
Then config.toml has harnesses "codex" (kind codex, ~/.codex), "kilo-codex" (kind codex, the kilo path)
  and "agents" (kind agents, ~/.agents)
And on a later run only the ones not yet registered are appended, existing lines unchanged
```

## AGENTS.md in Codex profiles

**AC2-05 AGENTS.md is linked**
```gherkin
Given AGENTS.md in the source and no AGENTS.md in a codex harness
When I run colctl sync
Then <harness>/AGENTS.md is a symlink to <source>/AGENTS.md and status reports it ok

Given AGENTS.md is then removed from the source
When I run colctl sync
Then the link is removed
```

**AC2-06 A real AGENTS.md is foreign and adoptable**
```gherkin
Given <codex harness>/AGENTS.md is a real file
When I run colctl sync
Then it is unchanged and reported foreign with the hint "colctl adopt <harness> AGENTS.md"

When I run colctl adopt <harness> AGENTS.md and the source has no AGENTS.md
Then its content becomes <source>/AGENTS.md and it is replaced by the link
And with a different <source>/AGENTS.md already present, nothing changes and the exit code is 1
```

## Skills through ~/.agents

**AC2-07 Skills are linked into the shared folder**
```gherkin
Given skills foo and bar in the source, and ~/.agents/skills holding twg-jira (a real folder) and .skill-lock.json
When I run colctl sync
Then ~/.agents/skills/foo and ~/.agents/skills/bar are symlinks into <source>/skills
And twg-jira and .skill-lock.json are unchanged byte for byte
And status reports twg-jira as foreign; colctl adopt agents skills/twg-jira adopts it like a Claude skill
```

## Agents generated for Codex

**AC2-08 Agents become Codex TOML**
```gherkin
Given <source>/agents/reviewer.md with frontmatter name "reviewer" and description "Reviews diffs", and a body
When I run colctl sync
Then each codex harness has agents/reviewer.toml with exactly the generated content from interfaces.md
And parsing it gives name "reviewer", description "Reviews diffs" and developer_instructions equal to the body
And its sha256 is recorded under generated."agents/reviewer.toml" for that harness
And Claude harnesses still get agents/reviewer.md as a link, as in 1.0
```

**AC2-09 Generated agents follow their source**
```gherkin
Given a generated agents/reviewer.toml
When reviewer.md is edited in the source
Then status reports the TOML as broken, and sync rewrites it

When reviewer.md is removed from the source
Then status reports it orphaned, and sync deletes it and its state entry
```

**AC2-10 Hand-edited and foreign agent TOML are left alone**
```gherkin
Given a generated agents/reviewer.toml that was edited by hand (hash differs from the state)
When I run colctl sync
Then it is unchanged and reported edited

Given agents/mine.toml that colctl didn't generate
Then it is reported foreign, never changed
And colctl adopt <harness> agents/mine.toml refuses: "Codex agents can't be converted back to .md; recreate it in <source>/agents"
```

**AC2-11 Agents that can't be converted**
```gherkin
Given <source>/agents/bad.md with no frontmatter, no description or an empty body
When I run colctl status
Then each codex harness reports agent bad as invalid with the reason
And Claude harnesses still link agents/bad.md as in 1.0
```

## Codex plugins

**AC2-12 Plugins are installed with Codex's CLI**
```gherkin
Given [codex.marketplaces] superpowers-dev = "github:obra/superpowers" and plugin "superpowers@superpowers-dev"
And a codex harness where neither is present
When I run colctl sync
Then colctl runs, in order, with that harness's environment:
  codex plugin marketplace add obra/superpowers
  codex plugin add superpowers@superpowers-dev
And both are recorded as installed-by-colctl for that harness
And Claude harnesses get no Codex calls, and codex harnesses no Claude calls
```

**AC2-13 Removal only where colctl installed**
```gherkin
Given a Codex plugin installed by colctl in harness A and by hand in harness B
When it is removed from the Codex list and I run colctl sync
Then "codex plugin remove <id>" runs for A only
```

**AC2-14 Each Codex profile gets exactly its own environment**
```gherkin
Given colctl runs with CODEX_HOME, OPENAI_API_KEY and OPENAI_BASE_URL set
When it calls codex for a harness
Then CODEX_HOME is that harness's path, or unset for the canonical ~/.codex
And no OPENAI_* variable reaches codex
```

**AC2-15 codex missing or failing**
```gherkin
Given no codex on PATH and Codex plugins declared
When I run colctl sync
Then AGENTS.md links, skill links and agent TOML are still synced
And each Codex plugin action is reported "skipped: codex not found" and the exit code is 1

Given codex exits non-zero for one action
Then the message has the full command, exit code and stderr, other actions still run, and the state isn't updated for it
```

**AC2-16 Status reads Codex plugin state without running codex**
```gherkin
Given a codex harness whose config.toml has [marketplaces.m] and [plugins."p@m"]
When I run colctl status with no codex on PATH
Then marketplace m and plugin p@m are reported from config.toml
```

## add, rm and adopt

**AC2-17 add and rm a Codex plugin**
```gherkin
When I run colctl add plugin superpowers@superpowers-dev --codex --marketplace github:obra/superpowers
Then [codex.marketplaces] and [codex.plugins].enabled gain them, comments kept, committed as
  "colctl: add codex plugin superpowers@superpowers-dev", and sync runs
When I run colctl rm plugin superpowers@superpowers-dev --codex
Then it leaves [codex.plugins].enabled, committed as "colctl: rm codex plugin superpowers@superpowers-dev", and sync runs
```

**AC2-18 adopt a Codex plugin**
```gherkin
Given a codex harness's config.toml has [plugins."p@m"] and [marketplaces.m] with source_type "git" and source "https://github.com/o/r.git"
When I run colctl adopt <harness> plugin p@m
Then config.toml gains [codex.marketplaces] m = "https://github.com/o/r.git" (if absent) and p@m in [codex.plugins].enabled
And it is committed as "colctl: adopt codex plugin p@m", and isn't recorded as installed by colctl
```

**AC2-19 adopt everything includes the new kinds**
```gherkin
Given foreign items in claude, codex and agents harnesses
When I run colctl adopt
Then Codex AGENTS.md files, ~/.agents skills and Codex plugins are adopted as above
And foreign agent TOML files are refused with the AC2-10 message
```

## Taking over skills other tools link in

Added after the owner found that the twg CLI installs its skills into `~/.config/twg/skills` and symlinks them into
`~/.agents/skills` on its own. The source of truth must be `~/.colctl`, not whichever folder a tool picked. Applies to
skills and agents in every kind of harness.

**AC2-24 Links into other folders are visible**
```gherkin
Given <harness>/skills/twg-jira is a symlink to ~/.config/twg/skills/twg-jira (outside the source)
When I run colctl status
Then it is reported "linked-elsewhere" with the target and the hint "colctl adopt <harness> skills/twg-jira"
And sync never changes it
```

**AC2-25 adopt takes the target over**
```gherkin
Given that link and no skills/twg-jira in the source
When I run colctl adopt <harness> skills/twg-jira (or colctl adopt)
Then the target folder is copied into <source>/skills/twg-jira and the copy's tree hash equals the target's
And only then is the harness link replaced by one to <source>/skills/twg-jira
And ~/.config/twg/skills/twg-jira is left exactly as it was
And the origin path and the adopted tree hash are recorded in the state
And it is committed as "colctl: adopt skill twg-jira" and sync spreads it to the other harnesses
```

**AC2-26 The origin changes later**
```gherkin
Given an adopted skill whose origin folder now differs from what was adopted, and an unedited source copy
When I run colctl status
Then the skill is reported "origin-changed" with the hint "colctl refresh skills/<name>"

When I run colctl refresh skills/<name> (or colctl refresh for all)
Then the origin is copied over the source copy, checked, committed as "colctl: refresh skill <name>"
And the recorded hash is updated, so status reports ok

Given the source copy was also edited since adoption
Then refresh refuses ("changed in both places"), nothing changes, and the exit code is 1
```

**AC2-27 The tool re-links over colctl**
```gherkin
Given an adopted skill whose harness link was replaced again by the tool's own link to the recorded origin
When I run colctl adopt <harness> skills/<name>
Then the link is pointed back at the source copy (no copy is made)
And if the origin's content differs from what was adopted, it says "origin has changes; run colctl refresh skills/<name>"
```

**AC2-28 What adopt still refuses**
```gherkin
Given a symlink in a harness that is dangling, points at something of the wrong type (a file for a skill),
  or is CLAUDE.md / AGENTS.md
When I run colctl adopt on it
Then it is refused with the reason, and nothing changes
And for a skill whose source copy differs from the target and whose origin isn't that target, both paths are shown
```

## Cross-cutting

**AC2-20 status --json shows the kinds**
```gherkin
When I run colctl status --json
Then each harness has "kind": "claude" | "codex" | "agents"
And a codex harness's instructions item is named "AGENTS.md" and its agents are items of kind "agent"
```

**AC2-21 Claude behaviour is unchanged**
```gherkin
Given the cycle 1 test suite
Then every cycle 1 test passes without being modified, except for renamed module paths
And except cycle 1's EC-14 test, whose "adopt refuses foreign links" expectation AC2-25 deliberately replaces
```

**AC2-22 Docs**
```gherkin
Then docs cover: the codex and agents kinds and the [codex] table (reference/config.md), --codex (reference/add.md, rm.md),
  a how-to "Add Codex", and an explanation of the shared skills folder and generated agents
```

**AC2-23 Traceability across cycles**
```gherkin
Then every AC-NN, EC-NN, AC2-NN and EC2-NN has a test containing its ID, checked in CI
```

## Constraints (how, not what)

- Same as cycle 1, plus: a maintained YAML parser for agent frontmatter, chosen in T4 with its reason in the PR.
- No writes to a Codex profile's `config.toml`; only `codex plugin …` changes it.
- Generated TOML is produced with `toml_edit`, never by string concatenation of values.
- Strict TDD: each task branch's first pushed commit holds its new tests, failing in CI.
