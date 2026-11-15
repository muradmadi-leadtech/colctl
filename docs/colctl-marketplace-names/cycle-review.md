# Cycle review: colctl marketplace names (HIL fix, 2.6.1)

_HIL cycle after cycle 9 · Evaluate · 2026-11-15_

| | |
|---|---|
| **Cycle** | `20261115-colctl-marketplace-names` (FORGE, HIL: Refine → Generate → Evaluate) |
| **Asked for** | The owner, at cycle 9's accept: Discover listed `qdrant` (Claude) and `colctl-qdrant-skills` (Codex) for one source; every source should be `colctl-<source>` |
| **Scope delivered** | ACH2-01 – 03, ECH2-01 – 04, each with a named test |
| **Released** | v2.6.1 |
| **PRs** | #137 (RED, then GREEN), release PR |
| **Disposition** | _Yours_ |

## What was going on

There was always one clone and one copy of each skill. The two rows were two marketplaces, one per tool. The repo
ships its own Claude marketplace, `qdrant`, so colctl used it as is. It has no Codex packaging, so colctl wrapped
the converted package as `colctl-qdrant-skills`. The name said whether the author had a marketplace, which you
shouldn't need to know.

## What changed

- **Every source installs from `colctl-<source>`** (ACH2-01). The repo's manifests still find the plugin's folder,
  and every field of the author's entry is kept (description, `strict`, component paths). Only where it installs
  from changes: colctl's one-plugin manifest in `~/.colctl/plugins/.manifests/<source>/`, pointing at the folder in
  the pinned clone.
- **Existing installs move on the next `sync`** (ACH2-02). colctl installs `<plugin>@colctl-<source>`, then uninstalls
  `<plugin>@<repo name>` and removes the marketplace it had added, through the same path 2.2 used (AC6-32). A repo
  marketplace that a plain plugin-list entry still uses stays (ECH2-04, new).
- **An entry pointing outside the clone** (`../x`) is never wrapped. The repo's own marketplace is used as before
  (ECH2-02).

## Found on the way: a same-version gap for wrapped sources

When a source's pin moves, 2.5.0 reinstalls a Claude plugin whose version didn't change, because Claude keeps a
plugin by its version (AC8-40). It detects that by comparing the commit Claude records with the pin. That check
was skipped for `colctl-` marketplaces, because there Claude records `~/.colctl`'s commit, not the plugin's. So
**databricks and typescript-lsp could stay on an old commit** after an approve, whenever the plugin's version number
didn't change. With every source wrapped, that gap would have reached qdrant and superpowers too.

Fixed: for a `colctl-` marketplace, a moved pin always reinstalls in Claude. It costs one uninstall and install per
Claude profile, and only when a pin moves.

## Tests changed

Assertions that named the repo's marketplace (`superpowers@superpowers-dev`, `qdrant@qdrant`, `dbx@dbx-skills`, the
manifest path). They now expect `colctl-<source>`. Behaviour tests whose premise changed were rewritten to keep
their point:

| Test | Now |
|---|---|
| EC3-07 (marketplace name taken) | `colctl-superpowers` taken by something else |
| EC3-12 (source id is also a list plugin) | the list plugin is `superpowers@colctl-superpowers` |
| AC6-32 (repo manifest moves to the clone) | stays `colctl-dbx` through the repo's switch; the move itself is ACH2-02 |
| AC6-33 (no convergence) | uses an entry outside the clone, the one case where colctl can still compare commits |
| AC8-40, EC8-28 (same version) | check the reinstall happens and status is in sync, not the recorded commit |
| Catalog AC5-04 | a leftover repo-named marketplace from a clone adds no second entry |

## What you'll see on your machine

On the first `colctl sync` after installing 2.6.1, in both Claude profiles:
- qdrant moves from `qdrant@qdrant` to `qdrant@colctl-qdrant-skills`.
- superpowers moves from `superpowers@superpowers-dev` to `superpowers@colctl-superpowers` (Codex profiles too).

Discover's Marketplaces then lists only `colctl-` names for your sources.

## Reviewer checklist

- [ ] `mise use -g github:muradmadi-leadtech/colctl@2.6.1`
- [ ] `colctl sync`: qdrant and superpowers say they moved, and `colctl status` is in sync
- [ ] `colctl` → Discover → Ctrl-V twice: your sources' marketplaces are all `colctl-…`
- [ ] Start a Claude session: qdrant's and superpowers' skills are there
