# What "owned" means

colctl's main promise is that it never destroys something it didn't create. Everything it does follows from being
able to prove which items are its own.

| Item | colctl owns it when… | Recorded where |
|---|---|---|
| A skill or agent in a profile | It's a symlink whose target is inside the source | Nowhere; the link itself is the proof |
| A profile's `CLAUDE.md` | Its hash matches the one colctl recorded when it wrote it, or its content is exactly the generated import | `.state/state.json` → `generated` |
| A marketplace or plugin in a profile | colctl ran the `claude plugin` command that added it | `.state/state.json` → `marketplaces`, `plugins` |

Everything else is **foreign**. colctl reports foreign items and leaves them exactly as they are. To hand one over,
run `colctl adopt`, which copies it into the source, checks the copy, and only then replaces the original with a link.

## Consequences

- **Name clashes don't overwrite.** If a profile has its own `skills/foo` folder and the source also has `foo`, sync
  skips that profile's `foo` and tells you.
- **Hand edits survive.** Change a generated `CLAUDE.md` and it becomes `edited`. Sync won't overwrite it until you
  move the text into `AGENTS.md` and delete the file.
- **Plugins you installed yourself stay.** Removing a plugin from `config.toml` uninstalls it only where colctl
  installed it.
- **Links you made yourself stay.** A symlink to somewhere outside the source is foreign, and `adopt` refuses it
  rather than guessing.
- **Writes are atomic.** Links and files are created under a temporary name (`.colctl-tmp-…`) and renamed into
  place, so an interrupted run leaves a temp entry (removed next time), never a half-written file.
