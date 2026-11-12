# colctl - FORGE Context

## Project Overview

colctl keeps every AI coding harness on a machine (Claude Code, and later Codex, OpenCode…) working from one
source of truth in `~/.colctl`: one `AGENTS.md`, one copy of each skill and agent, and one list of plugins.
Harnesses link to that source instead of keeping their own copies. Public repo:
`github.com/muradmadi-leadtech/colctl`, installed with `mise use -g github:muradmadi-leadtech/colctl`.

## Architecture Decisions

- **Rust**, one binary crate. Set up with `sadr init`; binary releases added on top (sadr does not generate them).
- **Link where the format is identical, generate where it must be converted, and let the harness's own CLI write its
  settings.** colctl never writes `settings.json` or Claude's plugin files.
- **Plan / apply split.** A pure planner turns (desired, actual) into actions; `apply` executes them. `--dry-run` prints the plan.
- **Ownership is provable.** A link into `~/.colctl` is colctl's; generated files and installed plugins are recorded in
  `.state/` with hashes. colctl never removes or overwrites anything it can't prove it owns.
- **Shell out to `git` and `claude`** rather than linking libgit2 or re-implementing plugin installs.

## Vocabulary

- **Source**: `~/.colctl` (or `$COLCTL_HOME`), a local git repo.
- **Harness**: one agent profile, a config folder plus a `kind` (e.g. `~/.claude`, kilo's `profiles/claude`).
- **Item**: an instructions file, skill, agent, marketplace or plugin.
- **Owned / foreign**: an item colctl can prove it put there / anything else.

## Conventions

- **FORGE documents at FORGE's default paths**: `docs/prd/<cycle>.md`, `docs/tasks/<cycle>.md`, `docs/<cycle>/…`.
- **User docs follow Diataxis**, next to them: `docs/tutorials`, `docs/how-to`, `docs/reference`, `docs/explanation`.
  A feature isn't done until its reference page exists.
- **Test-heavy.** Strict RED → GREEN → REFACTOR. Every acceptance criterion `AC-NN` and edge case `EC-NN` has at least
  one test whose name contains its ID. Coverage gate ≥ 90% lines (above FORGE's 80% default), enforced in `just check`.
- **Commits**: sadr enforces Conventional Commit PR titles (squash-only), so FORGE's phase prefix goes in the scope:
  `docs(focus): …`, `docs(refine): …`, `test(generate): …`, `feat(generate): …`.
- **Never touch the real `~/.claude` in tests.** Tests run against a temp `HOME` and a fake `claude` on `PATH`.

## Operating policy (autonomous)

Set by the user on 2026-11-12 for cycle 1: run Generate to completion without pausing, open PRs and squash-auto-merge
them once `check` passes, and cut the v1 release.

- **Always autonomous:** phase docs, tests, RED → GREEN → REFACTOR, refactors, PRs into `main`, merging green PRs,
  release-please release PRs, phase advancement.
- **STOP and get a human:** anything touching the user's real `~/.claude` or kilo profile beyond read-only checks
  (the Evaluate smoke test runs against a copy), deleting the repo or rewriting `main` history, changing repo
  settings beyond what `sadr init` sets, secrets or tokens.
