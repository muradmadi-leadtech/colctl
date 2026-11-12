# colctl adopt

```
colctl adopt [--dry-run]
colctl adopt <harness> [--dry-run]
colctl adopt <harness> skills/<name> | agents/<name>.md | CLAUDE.md | AGENTS.md | plugin <name@marketplace> [--dry-run]
```

Moves foreign items into the source, one at a time, then runs [sync](sync.md). With no arguments it takes every
foreign item in every harness; with a harness, every foreign item there.

| Item | What happens |
|---|---|
| skill / agent | Copied into the source; the copy's hash is checked against the original; only then is the original replaced by a link. If the source already has an identical item it is just linked; if it differs, it is refused. |
| `CLAUDE.md` | Its text becomes `~/.colctl/AGENTS.md` (refused if a different `AGENTS.md` exists) and it is replaced by the generated import. |
| plugin | Added to `[plugins].enabled`; its marketplace's origin is read from the profile's `known_marketplaces.json` and added to `[marketplaces]` if missing. It is not recorded as installed by colctl, so colctl won't uninstall that copy later. |
| `AGENTS.md` (codex harness) | Its text becomes `~/.colctl/AGENTS.md` (refused if a different one exists) and it is replaced by a link to it. |
| plugin (codex harness) | Added to `[codex.plugins]`; its marketplace's `source_type`/`source` are read from the profile's `config.toml` (`git` or `local`) and added to `[codex.marketplaces]`. Committed as `colctl: adopt codex plugin <id>`. |
| `agents/<name>.toml` (codex harness) | Refused: Codex agents can't be converted back to `.md`. Recreate it in `~/.colctl/agents`. |

## Skills a tool linked in from its own folder

Some tools install skills into a folder of their own and symlink them into a harness. twg, for example, keeps
`~/.config/twg/skills/twg-jira` and links it into `~/.agents/skills`. `status` shows such a link as
`linked-elsewhere` with its target. `adopt` takes it over:

1. The target is resolved (through any chain of links) and copied into `~/.colctl/skills/<name>`.
2. The copy's tree hash is checked against the target.
3. Only then is the harness link pointed at the source copy. The tool's folder is never touched.
4. The origin and its hash are recorded in `.state`, so `colctl status` can report `origin-changed` when the tool
   updates its copy, and [`colctl refresh`](refresh.md) can pull the update in.

If the tool re-links over colctl later, `adopt` points the link back at the source copy without copying, and says
when the origin has changes to refresh. Dangling links, links to the wrong type of thing, and a `CLAUDE.md` or
`AGENTS.md` that is itself a link are refused.

Each adopted item is committed as `colctl: adopt <kind> <name>` (`colctl: adopt instructions <harness>` for
`CLAUDE.md`). Symlinks pointing outside the source are refused.

Output: one line per item, `adopted …`, `linked (already identical) …` or `refused …: <reason>`.
Exit code: **1** if anything was refused or the sync afterwards isn't clean, otherwise **0**; **2** for an unknown
harness or item.

## Plugins with a known source

When `adopt` meets a plugin (or a `from-aggregator` one) whose own repo it can find, it makes it a **source plugin**
instead of adding it to a plugin list. It can find the repo through Claude's plugin-directory bindings, a marketplace
manifest that points at the plugin's repo or a folder in its own repo, or a Codex marketplace.

1. The repo is cloned and pinned to the **commit that's installed now** (or, if no tool recorded one, to the default
   branch's head, which adopt says).
2. A `[[source]]` is added with the adopted id in `replaces`, and the id leaves `[plugins]`/`[codex.plugins]` if it was
   listed. Commit: `colctl: adopt plugin <id> from source`.
3. Sync installs the source everywhere its repo packages for and uninstalls the aggregator's copy. If the plugin's
   marketplace was the plugin's own repo served remotely, that marketplace is re-pointed at the clone.

The same plugin from another aggregator, in another harness, just joins that source's `replaces`. A plugin whose
source can't be found is adopted into the plugin list as before.
