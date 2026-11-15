# Acceptance criteria: colctl marketplace names (HIL fix, 2.6.1)

_HIL cycle after cycle 9 · Refine · 2026-11-15. IDs are `ACH2-NN` / `ECH2-NN` (H2: the second HIL fix cycle)._

Raised by the owner at cycle 9's accept: Discover's Marketplaces view listed `qdrant` (Claude) and
`colctl-qdrant-skills` (Codex) for one source. A source whose repo ships its own marketplace kept that name; one colctl
wrapped was `colctl-<source>`. The name should say who manages it, the same way for every source.

**ACH2-01 Every source installs from colctl-<source>**
```gherkin
Given a source whose repo ships its own Claude and Codex marketplaces, listing the plugin from inside the repo
When colctl syncs
Then each tool installs <plugin>@colctl-<source> from ~/.colctl/plugins/.manifests/<source>, whose entry points at
  the plugin's folder in the pinned clone
And the entry keeps every field of the repo's entry except where it comes from
```

**ACH2-02 An existing install moves once**
```gherkin
Given a profile where colctl installed the source as <plugin>@<the repo's marketplace>, from the clone
When colctl syncs
Then it installs <plugin>@colctl-<source>, uninstalls the old id and removes the old marketplace colctl added
And status is in sync afterwards, and a second sync changes nothing
```

**ACH2-03 The docs say so**
```gherkin
Then docs/reference/config.md and docs/explanation/plugins-from-source.md say every source's marketplace is
  colctl-<source>, and why
```
