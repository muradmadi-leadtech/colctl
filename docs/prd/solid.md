# PRD: solid

_Cycle 9 · Focus · 2026-11-15_

## Problem statement

colctl 2.5 does the whole daily loop, but two things still make you wait or leave the TUI:

1. **Held versions are slow to look at.** `colctl status` and `colctl review` inspect every held version from scratch
   on each run: the commit log, every changed file and its risk flags. With two big updates held (62 and 51 commits)
   that took about 14 s, and the TUI's Overview and Review tabs wait for it. With nothing held, `status` takes 0.36 s
   today.
2. **The TUI covers only the core loop.** `adopt`, `convert` (and `convert --undo`), `rm`, `refresh`, `marketplace`,
   `connectors` and `developers` have no screen, so using them means quitting to the shell.

Small rough edges carried over from earlier cycles also make the output noisier than it should be.

## Target users

- **The owner**, daily, in a terminal on the work Mac, keeping 5 profiles in sync.
- Scripts and CI, which use the CLI and `--json`. Their output must not change shape.

## Success criteria

| ID | Criterion | How it's measured |
|---|---|---|
| S1 | `status` and `review` answer in under 1 s with the two large updates held, after the first inspection | Timed on a copy of the real profiles with databricks and qdrant-skills held |
| S2 | The first inspection after `update` happens once, during `update`, not on every later run | A test counts inspections across `update`, `status`, `review` |
| S3 | A cached review is never stale: changing the hold (new update, approve, reject, pin change, edited held copy) recomputes it | Tests per invalidation case |
| S4 | The TUI opens on the last known state immediately and refreshes in the background | First frame drawn before the snapshot returns |
| S5 | Every command except `init`, `completions`, `version` and `help` can be done from the TUI | A parity test per command: the TUI's action runs the same core call as the CLI |
| S6 | Every carried follow-up is closed or dropped on purpose, with the reason written down | The cycle review lists each one |
| S7 | `--json` output keeps its shape | Existing JSON contract tests pass unchanged |

## Carried follow-ups (S6)

- `allowed-tools` flags are noisy: one per file, the same message repeated.
- `-v` output prints full paths instead of `~/…`.
- The `ETXTBSY` test flake (a fake tool executed while still being written).
- `connectors` shows empty fields.
- `sync`'s "needs attention" wording.
- qdrant in Claude profiles installed from `qdrant@qdrant`, not colctl's clone: status should name `colctl adopt` as
  the fix instead of `colctl sync`.

## Boundaries

**In:** a review cache, the TUI opening on cached state, TUI screens or actions for the commands above, a way to reach
every action quickly, and the follow-ups.

**Out:** new kinds of things to manage (MCP servers, commands, hooks: cycle 10), tags and cost (cycle 11), themes or a
TUI config file, changes to what the recommendation decides.

## Constraints

- CLI first: a screen shows only what a command does, through `src/core`.
- The cache lives in `~/.colctl`'s state, is safe to delete, and is never committed with secrets.
- Strict TDD, AC/EC-named tests, ≥ 90% coverage, a smoke test on a copy of the real profiles.
