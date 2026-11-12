# Acceptance criteria: Quarantine

_Cycle 4 · Refine · 2026-11-12_

IDs are `AC4-NN`; each needs a test whose name contains `ac4_NN`. Formats are in [interfaces.md](interfaces.md),
edge cases in [edge-cases.md](edge-cases.md). In tests, "upstream" is a local git repo reached by a `file://` URL,
and "origin" is a folder outside the source, as in cycle 2.

A **candidate** is a held new version: a commit for a source plugin, a snapshot for a taken-over skill or agent.
Its **key** is `sources/<name>`, `skills/<name>` or `agents/<name>.md`. On the command line a source plugin is
named by its source name (`superpowers`), and skills and agents by their key, as `refresh` already does.

## Storage (T1)

**AC4-01 Older state files load**
```gherkin
Given a .state/state.json written by colctl 1.x
When any command runs
Then it loads with nothing held and nothing rejected
And state.json gains "quarantine" and "rejected" only once something is held or rejected
```

**AC4-02 Held content is never committed**
```gherkin
Given a taken-over skill whose origin changed
When I run colctl refresh
Then the snapshot is at .state/quarantine/skills/<name>
And git status in ~/.colctl is clean, and no commit was made
```

**AC4-03 A held commit survives until it's decided**
```gherkin
Given colctl update held commit C for source superpowers
When upstream force-pushes its branch so C is no longer on it, and git gc --prune=now runs in the clone
Then refs/colctl/candidate in the clone still points at C, and colctl review superpowers still shows it
```

## Risk flags (T2)

Flags apply to added, modified, renamed and mode-changed files. Deleted files are never flagged.

**AC4-04 Hooks**
```gherkin
Given a candidate that adds or changes hooks/hooks.json, any file named hooks.json, or a "hooks" key in a plugin.json
When it's reviewed
Then each file gets a hook flag naming the file
```

**AC4-05 MCP and LSP servers**
```gherkin
Given a candidate that adds or changes .mcp.json, .lsp.json, or an "mcpServers" / "lspServers" key in a plugin.json
When it's reviewed
Then each file gets an mcp or lsp flag naming the file
```

**AC4-06 Code that can run**
```gherkin
Given a candidate with a file that becomes executable, a new or changed script (by extension or a #! first line),
  and a new or changed file that isn't valid UTF-8
When it's reviewed
Then they get executable, script and binary flags respectively, naming each file
```

**AC4-07 Symlinks**
```gherkin
Given a candidate that adds a symlink or changes where one points
When it's reviewed
Then it gets a symlink flag with its target
And the reason says "points outside" when the target resolves outside the plugin repo or the skill
```

**AC4-08 Permissions and inline shell**
```gherkin
Given a candidate whose Markdown frontmatter adds or changes allowed-tools (or tools, in an agent),
  or whose Markdown adds a !`command` line
When it's reviewed
Then it gets an allowed-tools flag (the reason names Bash when Bash is granted) or an inline-shell flag
```

**AC4-09 Manifests**
```gherkin
Given a candidate that changes a file under .claude-plugin/, .codex-plugin/ or .agents/plugins/
When it's reviewed
Then it gets a manifest flag naming the file
```

**AC4-10 Nothing risky, no flags**
```gherkin
Given a candidate that only changes README.md and a skill's body text, and deletes a script
When it's reviewed
Then it has no flags
```

## Update holds (T3)

**AC4-11 update holds instead of installing**
```gherkin
Given a source superpowers pinned at A whose upstream head is B
When I run colctl update
Then it prints "superpowers: A → B (n new commits), held for review; colctl review superpowers"
And config.toml, the source repo's history, the clone's HEAD and every harness are unchanged
And no claude or codex command ran
And the state holds sources/superpowers from A to B
And the exit code is 0
```

**AC4-12 Nothing newer**
```gherkin
Given a source whose upstream head is its pin
When I run colctl update
Then it prints "<name>: up to date", nothing is held, exit 0
```

**AC4-13 A newer commit replaces the held one**
```gherkin
Given superpowers holds B
When upstream moves to C and I run colctl update
Then it holds C instead of B, and says it replaced B
And running colctl update again with upstream still at C prints "superpowers: C already held for review"
```

**AC4-14 --rev holds the named commit**
```gherkin
Given a source pinned at A
When I run colctl update superpowers --rev v7.0.2
Then the commit v7.0.2 resolves to is held, whether it's newer or older than A
```

**AC4-15 --check is unchanged, and says what's held**
```gherkin
Given sources with a newer upstream, one of them already held and one whose newer commit was rejected
When I run colctl update --check
Then each prints "<name>: A → B (n new commits)", with "(held for review)" or "(rejected)" after it
And nothing is held or changed
And the exit code is 1 when any source has a newer commit that isn't rejected, else 0
```

**AC4-16 sync never installs a candidate**
```gherkin
Given superpowers holds B and is pinned at A
When I run colctl sync
Then every harness stays at A, and the candidate is still held
```

## Refresh holds (T4)

**AC4-17 refresh holds instead of copying**
```gherkin
Given a taken-over skill skills/twg-jira whose origin changed while the source copy is as adopted
When I run colctl refresh
Then it prints "held skills/twg-jira for review; colctl review skills/twg-jira"
And the source copy, its recorded origin hash and the source repo's history are unchanged, and sync doesn't run
And the state holds the snapshot's hash, the origin path and the base hash (what's in the source now)
And the same is true for a taken-over agent
```

**AC4-18 refresh's refusals stay, and it knows what's held**
```gherkin
Given taken-over items where one changed in both places, one's origin is gone, and one is already held with the
  origin unchanged since
When I run colctl refresh
Then the first two are refused as in 1.x (exit 1), and the third prints "<key> already held for review"
And an item whose origin changed again since it was held gets a new snapshot that replaces the old one
```

## Review (T5)

**AC4-19 review lists what's held**
```gherkin
Given one held source plugin and one held skill
When I run colctl review
Then one line per candidate: key, what it changes from and to, number of commits or changed files, number of flags
And the exit code is 1; with nothing held it prints "nothing held for review" and exits 0
```

**AC4-20 Reviewing a source plugin**
```gherkin
Given superpowers holds B over pin A
When I run colctl review superpowers
Then it prints the commits from A to B (short hash, subject), the changed files with their status,
  the risk flags (kind, file, reason) and the unified diff from A to B
And it doesn't fetch, write or take the lock
```

**AC4-21 Reviewing a skill or agent**
```gherkin
Given skills/twg-jira is held
When I run colctl review skills/twg-jira
Then it prints the changed files, the risk flags and the unified diff from the source copy to the snapshot
```

**AC4-22 review --json**
```gherkin
When I run colctl review --json, or colctl review <name> --json
Then the output is the JSON in interfaces.md: every candidate with its files and flags, and the diff text only for
  a named candidate
```

## Approve (T6)

**AC4-23 Approving a source plugin**
```gherkin
Given superpowers holds B over pin A
When I run colctl approve superpowers
Then config.toml pins B, committed as "colctl: approve source superpowers at <short B>"
And the candidate ref and the state entry are gone
And sync runs, so every harness the repo packages for has superpowers at B
```

**AC4-24 Approving a skill or agent**
```gherkin
Given skills/twg-jira is held
When I run colctl approve skills/twg-jira
Then the snapshot replaces the source copy, its recorded origin hash becomes the snapshot's hash,
  the change is committed as "colctl: approve skill twg-jira", the snapshot is deleted, and sync runs
And an agent is committed as "colctl: approve agent <name>"
```

**AC4-25 What's approved is what was reviewed**
```gherkin
Given superpowers holds B and skills/twg-jira is held
When upstream moves to C and the skill's origin changes again, and then I approve both
Then superpowers is pinned at B, not C, and the skill's source copy is the held snapshot, not the origin's new content
```

**AC4-26 Several at once, --all, --dry-run**
```gherkin
Given three held candidates
When I run colctl approve superpowers skills/twg-jira, or colctl approve --all
Then each named (or every) candidate is approved with its own commit, and sync runs once at the end
And with --dry-run, each prints "would approve <key>: <from> → <to>" and nothing changes
```

## Reject (T7)

**AC4-27 Rejecting**
```gherkin
Given superpowers holds B and skills/twg-jira is held with snapshot hash H
When I run colctl reject superpowers skills/twg-jira (or colctl reject --all)
Then both candidates are gone (ref deleted, snapshot deleted), nothing installs, and B and H are recorded as rejected
And a later colctl update with upstream at B prints "superpowers: B was rejected; nothing held" and holds nothing
And a later colctl refresh with the origin at H holds nothing
And when upstream moves past B, or the origin changes again, the new version is held as usual
```

**AC4-28 Removing an item drops its candidate**
```gherkin
Given superpowers and skills/twg-jira are held and have rejected versions
When I run colctl rm source superpowers and colctl rm skill twg-jira
Then their candidates, snapshots and rejections are removed from the state too
```

## Status (T8)

**AC4-29 Held candidates show in status**
```gherkin
Given held candidates
When I run colctl status
Then the source section lists each as "pending-review  <key>  <from> → <to>  → colctl review <name>"
And status --json has a "quarantine" array (see interfaces.md)
And the exit code is 1, as for origin-changed; in_sync is false
```

**AC4-30 List plugins are unreviewed**
```gherkin
Given plugins installed from [plugins] or [codex.plugins]
When I run colctl status
Then each is "unreviewed", with the hint "updates come from <marketplace>, not through colctl review"
  plus "; colctl adopt <harness> plugin <id> to hold them" when its source repo is known
And unreviewed doesn't make the exit code 1 or in_sync false, like unsupported
And source plugins, from-aggregator and plugin-missing are unchanged
```

**AC4-31 origin-changed leaves out what's held or rejected**
```gherkin
Given a taken-over skill whose origin's content is held, and another whose origin's content was rejected
When I run colctl status
Then neither is listed as origin-changed
```

## Docs and traceability (T9)

**AC4-32 Docs cover quarantine**
```gherkin
Given the repository
Then docs/reference has review.md, approve.md and reject.md
And update.md, refresh.md, status.md (pending-review, unreviewed, "quarantine") and state.md describe the new behaviour
And docs/how-to/review-an-update.md and docs/explanation/quarantine.md exist, and commands.md lists the new commands
And every relative link in the docs resolves
```

**AC4-33 Every criterion is tested, and earlier suites are intact**
```gherkin
Given the AC4/EC4 IDs defined in this cycle
Then each has a test named after it
And each earlier suite still has its tests, none ignored; any test changed because update and refresh now hold is
  listed in the cycle review
```
