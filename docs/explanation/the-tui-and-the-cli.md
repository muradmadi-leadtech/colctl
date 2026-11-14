# The TUI and the CLI

colctl has two front ends over one core. The CLI is for scripts, CI and quick answers. The TUI is for looking around
and acting on what you see. Neither can do anything the other can't.

## One logic

Every screen reads from `colctl`'s core, the same functions the commands use. Every action is a command:

| In the TUI | The command it runs |
|---|---|
| `s` on Overview | `colctl sync` |
| `s` on a profile | `colctl sync --harness <name>` |
| `u` on Review | `colctl update` |
| `a`, `x`, `A` on Review | `colctl approve <name>`, `reject <name>`, `approve --all` |
| `a` on Discover | the add command `colctl show` names (`add plugin …` or `add source …`) |
| ⏎ on a held version | `colctl review <name>` |
| ⏎ on a plugin | `colctl show <id>` |

The test suite runs each action both ways on identical machines and checks that `~/.colctl`, its state, its git
history and every profile end up the same.

## CLI first

When the TUI needs something the CLI can't do or show, the CLI gets it first. Building the TUI changed the CLI in
two ways:

- **The core API.** `status`, `sync`, `update`, `approve`, `reject`, `review`, `search`, `show` and `add` used to work
  out their answer and print it in one step. Now they return what happened, and the CLI and the TUI each decide how it
  looks. Their output didn't change.
- **Fuzzy search.** Typing in Discover finds `superpowers` from `sprpw`. So does `colctl search sprpw`, because a
  screen can't know something the command doesn't.

## Why not only a TUI

A command can be scripted, piped, run in CI and pasted into an issue. Its output and exit code are the contract, and
`--json` is stable. The TUI is built on those commands, so nothing is lost when you leave it.
