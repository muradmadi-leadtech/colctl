# Components: TUI (C4 level 3)

## Core API (`src/core`, new: the CLI gaps)

| Component | Responsibility | Used by |
|---|---|---|
| `core::status` | `snapshot(ctx, filter) -> Snapshot`: profiles, their items and states, source problems, held items, the verdict and the exit code. `status --json` serialises it, byte-identical to 2.3 | `status`, Overview, Profiles, Items |
| `core::Observer` | Trait for progress: `step(profile, action, n, total)`, `done(profile, summary)`, `note(text)`, `warn(text)`. The CLI's implementation is `ui::progress::Run` and `ctx.out`. The TUI's sends events over a channel. Tests record them | every run below |
| `core::sync` | `sync(ctx, filter, &mut dyn Observer) -> SyncReport` (per-profile results, notes, the after-snapshot) | `sync`, Overview |
| `core::quarantine` | `held`, `review(name) -> View`, `update(names, obs) -> UpdateReport`, `approve(names, obs)`, `reject(names)` | `update`, `review`, `approve`, `reject`, Review tab |
| `core::discover` | `catalog(ctx) -> Catalog`, `rank(plugins, query) -> Vec<Hit>` (nucleo), `show(id) -> PluginDetail`, `add(id, obs) -> AddReport` | `search`, `show`, `add`, Discover tab |

Rules: core functions never print, prompt or read the terminal. Confirmation is the front end's job, before it calls.
Anything that was a stray warning on stdout becomes part of the report.

## TUI front end (`src/tui`, new)

| Component | Responsibility |
|---|---|
| `tui::launch` | The launch rule. A terminal guard that enters the alternate screen, raw mode and mouse capture, and restores them on drop, panic hook and Ctrl-C |
| `tui::app` | Elm-style `Model` (tabs, selections, filters, overlays, running job), `Msg`, and a pure `update(model, msg) -> (model, Vec<Cmd>)`. Tests drive it with keys and never touch a terminal |
| `tui::worker` | Runs a `Cmd` (snapshot, sync, update, approve, reject, catalog, add) on a thread with a channel `Observer`, and posts `Msg`s back |
| `tui::view` | Pure render of a `Model` into a `Frame`: tab bar, key bar, one module per tab, overlays (help, confirm, detail, progress, error), the too-small screen |
| `tui::theme` | Colour tokens and symbols from the design spec, mapped from the CLI's `ui::style::Token`. `NO_COLOR` gives a monochrome set |
| `tui::motion` | tachyonfx effects for tab switches, overlays opening and closing, and list updates. All off with `COLCTL_REDUCED_MOTION=1` or in tests |
| `tui::keys` | The keymap: one table drives handling, the key bar and the `?` overlay. Mouse hits are mapped to the same `Msg`s |

## Changed

| Component | Change |
|---|---|
| `cli::run` | The launch rule for bare `colctl` |
| `commands::status`, `sync`, `update`, `review`, `approve`, `reject`, `search`, `show`, `add` | Call core, then render. Text and JSON stay as in 2.3; `search` ranking follows the shared nucleo ranker (the one visible CLI change, documented) |
| `ui::progress::Run` | Implements `core::Observer` |
