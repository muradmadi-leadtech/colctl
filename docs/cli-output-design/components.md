# Components (C4 level 3): CLI output design

```mermaid
flowchart TB
    commands["commands::*"]
    out["ui::Out<br/>the handle every command gets"]
    term["ui::term<br/>Caps: tty, width, colour, interactive"]
    style["ui::style<br/>semantic styles and symbols"]
    render["ui::render<br/>pure components → String"]
    progress["ui::progress<br/>live steps; plain lines when piped"]
    ask["ui::ask<br/>confirm, pick, pager"]
    errors["ui::errors<br/>CliError and clap errors → error/cause/hint"]

    commands --> out
    out --> term & render & progress & ask & errors
    render --> style
    progress --> style
```

| Component | One job |
|---|---|
| `ui::term` | Works out `Caps` once: is stdout/stderr/stdin a terminal, width (default 80 when unknown, clamped 40–160), colour on/off from `--color`, `NO_COLOR`, `CLICOLOR_FORCE`, `TERM=dumb`, and whether prompts are allowed (all terminals, not `--json`, not `CI`) |
| `ui::style` | The palette as meaning, not colour: `ok`, `attention`, `failed`, `info`, `dim`, `name`, `command`, `heading`, each with a symbol (✓ ! ✗ ·). Plain when colour is off |
| `ui::render` | Pure functions from data + `Caps` to text: verdict line, section heading, key–value block, table (measures, truncates with "…", aligns), item line with state badge, hint, warning, error block, summary footer. All unit- and snapshot-tested |
| `ui::progress` | A live view of a multi-step run: one line per profile with a spinner and the current step, collapsing to a ✓/✗ line when done; in a pipe, one plain line per finished step. `-q` hides it, `-v` adds each tool call and its time |
| `ui::ask` | `confirm` (default No, `--yes` answers yes, never asks without a terminal: it refuses with a hint to pass `--yes`), `pick` (select one or many from a list; without a terminal the command errors as before), `page` (pipes long text to `$PAGER` or `less -R` in a terminal, unless `--no-pager`) |
| `ui::errors` | One rendering for every error: `error: <what>`, indented cause, `hint: <fix>`. Wraps clap's errors (unknown command, missing argument) in the same look, keeping exit codes |
| `ui::Out` | Carries `Caps` and `-q`/`-v`, and exposes `verdict`, `section`, `table`, `item`, `hint`, `warn`, `progress`, `confirm`, `pick`, `page`. Writes to stdout (results) and stderr (progress, warnings, errors, prompts) |

**Seams for testing:** `render` is pure, so snapshot tests run at fixed widths with colour on and off. `Out` can write
into a buffer with chosen `Caps`, so command-level snapshots don't need a real terminal. Interaction is tested
through `Caps` (non-interactive paths in integration tests; pickers and confirms via injected answers).
