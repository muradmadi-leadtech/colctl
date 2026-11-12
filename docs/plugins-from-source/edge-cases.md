# Edge cases: Plugins from source

_Cycle 3 · Refine · 2026-11-12_

IDs are `EC3-NN`; each needs a test whose name contains `ec3_NN`.

## Empty / invalid

| ID | Case | Expected |
|---|---|---|
| EC3-01 | No `[[source]]` entries | No clones, no source items; behaviour as in 1.1 |
| EC3-02 | Pinned rev doesn't exist in the repo, even after fetching | Source invalid: "rev <short> not found in <repo>"; nothing installed for it |
| EC3-03 | Repo unreachable on the first clone | Source invalid with git's stderr; every other item still syncs; exit 1; no half-made clone left |
| EC3-04 | Repo has no Claude or Codex packaging for the plugin | `add source`/`adopt` exit 2 and remove the clone; an existing source shows invalid |
| EC3-05 | Manifest lists several plugins | `plugin` (default: the source name) picks one; a name that isn't listed makes the source invalid: "no plugin <p> in <manifest>" |
| EC3-06 | A manifest isn't valid JSON | That tool's packaging is invalid with the parse error; the other tool still works |

## Boundary / ownership

| ID | Case | Expected |
|---|---|---|
| EC3-07 | A marketplace with the source's marketplace name already exists in a harness from another origin | If colctl added it for a list plugin that is no longer declared, it's removed before the local one is added. Otherwise the source is invalid in that harness with the hint "marketplace <m> already comes from <origin>" |
| EC3-08 | `plugins/<name>` exists but isn't a git repo, or its `origin` remote isn't the declared repo | Source invalid; the folder is never changed |
| EC3-09 | Source name `.manifests`, or any invalid item name | Config error, exit 2 |
| EC3-10 | A `replaces` id isn't installed in a harness | Nothing happens there |
| EC3-11 | Directory binding with a subpath (`github.com/o/r#plugins/x`) | The plugin is taken from that subfolder through a colctl-written manifest |
| EC3-12 | A plugin with the source's id is also in `[plugins]` or `[codex.plugins]` | Config error, exit 2: "<id> is both a source plugin and a list plugin" (checked once the clone's packaging is known; reported as invalid at plan time) |

## Failure

| ID | Case | Expected |
|---|---|---|
| EC3-13 | `claude plugin update` (or `codex plugin add`) fails during an update | Reported in full; the recorded rev stays the old one, so the next sync retries |
| EC3-14 | `git checkout` of the pin fails | The clone stays where it was; source invalid with git's stderr |
| EC3-15 | `update` can't reach upstream | That source reports the error; the pin and the clone don't change; exit 1 |

## Timing / compatibility

| ID | Case | Expected |
|---|---|---|
| EC3-16 | Upstream force-pushed so the pinned commit is gone remotely, but it's still in the local clone | sync works offline as before; `update` moves to the new head |
| EC3-17 | A 1.1 `state.json` | Loads; `sources` entries are added beside it |
| EC3-18 | A Codex marketplace snapshot that isn't a git checkout (local marketplace) during adopt | Pins to the source repo's default-branch head and says so ("installed commit unknown; pinned to <short>") |
