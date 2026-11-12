# Commands

| Command | Page | Changes things |
|---|---|---|
| `colctl init [--dry-run]` | [init](init.md) | yes |
| `colctl status [--harness <name>] [--json]` | [status](status.md) | never |
| `colctl sync [--harness <name>] [--dry-run]` | [sync](sync.md) | yes |
| `colctl add skill\|agent\|plugin …` | [add](add.md) | yes |
| `colctl rm skill\|agent\|plugin <name> [--dry-run]` | [rm](rm.md) | yes |
| `colctl adopt [<harness> [<item>]] [--dry-run]` | [adopt](adopt.md) | yes |
| `colctl version`, `colctl --version` | — | never |

Every command that changes something:

- takes `--dry-run`, which prints the plan and changes nothing at all (no files, links, state, lock, commits or
  `claude` calls);
- needs `git` on `PATH` and exits 2 without it;
- holds a lock (`~/.colctl/.state/lock`) while it runs, and exits 3 if another colctl holds it;
- commits any edits you made in `~/.colctl` first, as `colctl: sync`.

## Environment

| Variable | Effect |
|---|---|
| `COLCTL_HOME` | Use this absolute path as the source instead of `~/.colctl`. Generated `CLAUDE.md` files then import it by its absolute path. |
| `COLCTL_CLAUDE_TIMEOUT_SECS` | Seconds one `claude` call may take before it is killed (default 300). |
| `HOME` | Required. `~` in `config.toml` expands to it. |

colctl removes `CLAUDE_CONFIG_DIR`, `CLAUDE_SECURESTORAGE_CONFIG_DIR` and every `ANTHROPIC_*` variable before calling
`claude`, then sets `CLAUDE_CONFIG_DIR` to the profile being synced (except for `~/.claude`). Running colctl from
inside a Claude Code session is safe.
