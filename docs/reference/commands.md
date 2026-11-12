# Commands

| Command | Page | Changes things |
|---|---|---|
| `colctl init [--dry-run]` | [init](init.md) | yes |
| `colctl status [--harness <name>] [--json]` | [status](status.md) | never |
| `colctl sync [--harness <name>] [--dry-run]` | [sync](sync.md) | yes |
| `colctl add skill\|agent\|plugin …` | [add](add.md) | yes |
| `colctl rm skill\|agent\|plugin <name> [--dry-run]` | [rm](rm.md) | yes |
| `colctl adopt [<harness> [<item>]] [--dry-run]` | [adopt](adopt.md) | yes |
| `colctl refresh [<item>…] [--dry-run]` | [refresh](refresh.md) | holds new versions |
| `colctl update [<name>…] [--check] [--rev <commit-ish>] [--dry-run]` | [update](update.md) | holds new versions (not with `--check`) |
| `colctl review [<name>] [--json]` | [review](review.md) | never |
| `colctl approve (<name>… \| --all) [--dry-run]` | [approve](approve.md) | yes |
| `colctl reject (<name>… \| --all) [--dry-run]` | [reject](reject.md) | yes (state only) |
| `colctl version`, `colctl --version` | — | never |

Every command that changes something:

- takes `--dry-run`, which prints the plan and changes nothing at all (no files, links, state, lock, commits or
  `claude` calls);
- needs `git` on `PATH` and exits 2 without it;
- holds a lock (`~/.colctl/.state/lock`) while it runs, and exits 3 if another colctl holds it;
- commits any edits you made in `~/.colctl` first, as `colctl: sync` (`update`, `refresh` and `reject` commit
  nothing, so they skip this).

## Environment

| Variable | Effect |
|---|---|
| `COLCTL_HOME` | Use this absolute path as the source instead of `~/.colctl`. Generated `CLAUDE.md` files then import it by its absolute path. |
| `COLCTL_TOOL_TIMEOUT_SECS` | Seconds one `claude` or `codex` call may take before it is killed (default 300). |
| `COLCTL_CLAUDE_TIMEOUT_SECS` | The 1.0 name of the same setting; used when `COLCTL_TOOL_TIMEOUT_SECS` isn't set. |
| `HOME` | Required. `~` in `config.toml` expands to it. |

colctl removes `CLAUDE_CONFIG_DIR`, `CLAUDE_SECURESTORAGE_CONFIG_DIR` and every `ANTHROPIC_*` variable before calling
`claude`, then sets `CLAUDE_CONFIG_DIR` to the profile being synced (except for `~/.claude`). Running colctl from
inside a Claude Code session is safe. For `codex` it removes `CODEX_HOME` and every `OPENAI_*` variable, then sets
`CODEX_HOME` to the profile (except for `~/.codex`), so running inside a Codex session is safe too.

## Quarantine

New versions of things colctl already manages are held, not installed: [`update`](update.md) holds a source plugin's
new commit, [`refresh`](refresh.md) holds a taken-over skill or agent's new content. [`review`](review.md) shows them
with risk flags; [`approve`](approve.md) installs exactly what was held; [`reject`](reject.md) drops it for good.
`status` lists held versions as `pending-review`. See [Quarantine](../explanation/quarantine.md).

## Network

Only `colctl update`, `colctl adopt` (when it clones a plugin's source), `colctl add source`, and the first `sync` after a
source is declared contact upstream repos. `status`, `review`, `approve`, `reject` and every other `sync` work offline.
