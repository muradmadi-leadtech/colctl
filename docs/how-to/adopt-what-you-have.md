# Adopt what you already have

`colctl adopt` moves items a profile already has into `~/.colctl` and replaces them with links, then syncs.

```sh
colctl adopt --dry-run                        # everything foreign, everywhere
colctl adopt                                  # do it
colctl adopt kilo-claude                      # one profile
colctl adopt claude skills/forge              # one skill
colctl adopt claude agents/reviewer.md        # one agent
colctl adopt claude CLAUDE.md                 # instructions → ~/.colctl/AGENTS.md
colctl adopt claude plugin sadr@sadr          # a plugin (and its marketplace) → config.toml
```

Each adopted item is its own commit in `~/.colctl` (`colctl: adopt skill forge`).

## When adopt refuses

| Message | What to do |
|---|---|
| `differs from the source copy; both kept: <a> and <b>` | The same name exists in `~/.colctl` with other content. Keep the one you want, delete the other, re-run. |
| `<AGENTS.md> already exists with different text` | Merge the profile's `CLAUDE.md` into `~/.colctl/AGENTS.md` by hand, delete the `CLAUDE.md`, run `colctl sync`. |
| `remove or replace this link yourself` | The item is a symlink you made to somewhere else; colctl won't guess what you meant. |
| `can't tell where marketplace "m" comes from` | Use `colctl add plugin <id> --marketplace <source>` instead. |
| `the copy didn't match the original; nothing changed` | The copy was incomplete (disk full, files changing). Nothing was touched; try again. |

The command exits 1 when anything was refused, and 0 otherwise.

## What it never adopts

`skills/synced/` (skills synced from your claude.ai account) and anything starting with a dot are ignored entirely.
