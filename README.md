# colctl

Keep every AI coding harness on your machine in sync with one source of truth: one `AGENTS.md`, one copy of each
skill, one plugin list.

If you run more than one Claude Code profile (your normal `~/.claude` plus a proxy or gateway profile such as
kilo-proxy's), each one keeps its own skills, agents, instructions and plugins, and they drift apart. colctl keeps a
single copy of all of it in `~/.colctl` and points every profile at it:

- **Skills and agents** are symlinked, so there is one copy on disk and an edit shows up everywhere at once.
- **`AGENTS.md`** is imported by a two-line `CLAUDE.md` that colctl writes in each profile.
- **Plugins and marketplaces** are listed once in `~/.colctl/config.toml` and installed into each profile with
  Claude Code's own `claude plugin` commands.
- **`~/.colctl` is a local git repo**, committed after every change, so you can see and undo what changed.

colctl never overwrites or deletes anything it didn't create. What it finds that isn't its own is reported, and
`colctl adopt` moves it into `~/.colctl` when you ask.

## Install

```sh
mise use -g github:muradmadi-leadtech/colctl
```

Builds for macOS on Apple silicon and Linux x86_64 are attached to each
[release](https://github.com/muradmadi-leadtech/colctl/releases), each with a build attestation:
`gh attestation verify colctl-<target>.tar.gz --repo muradmadi-leadtech/colctl`. colctl needs `git`; plugin sync also
needs Claude Code's `claude` on `PATH`.

## Use

```sh
colctl init      # create ~/.colctl and register the Claude profiles found on this machine
colctl adopt     # move the skills, agents, CLAUDE.md and plugins you already have into ~/.colctl
colctl status    # what's in sync and what isn't (exit 1 if anything needs attention)
colctl sync      # make every profile match ~/.colctl
```

After that, edit `~/.colctl/AGENTS.md` and `~/.colctl/skills/*` directly, or use `colctl add skill|agent|plugin` and
`colctl rm`. Start with the [tutorial](docs/tutorials/first-sync.md).

## Docs

The docs follow [Diataxis](https://diataxis.fr):

- **Tutorial:** [Your first sync](docs/tutorials/first-sync.md)
- **How-to:** [Add a Claude profile](docs/how-to/add-a-claude-profile.md) ·
  [Adopt what you already have](docs/how-to/adopt-what-you-have.md) · [Add a plugin](docs/how-to/add-a-plugin.md)
- **Reference:** [commands](docs/reference/commands.md) · [`config.toml`](docs/reference/config.md) ·
  [exit codes](docs/reference/exit-codes.md) · [the `.state` folder](docs/reference/state.md)
- **Explanation:** [Links, not copies](docs/explanation/links-not-copies.md) ·
  [What "owned" means](docs/explanation/ownership.md)

colctl is built in [FORGE](https://github.com/scottfeltham/forge-framework) cycles. Each cycle's intent (PRD,
architecture, acceptance criteria) is in `docs/prd/`, `docs/tasks/` and `docs/<cycle>/`.

## Development

Requires [mise](https://mise.jdx.dev).

```sh
mise install    # the tools pinned in mise.toml
just setup      # commit guard
just check      # the gate CI runs: format, lint, tests with a 90% coverage floor, audits
just coverage   # an HTML coverage report
```

Tests never touch your real `~/.claude`: they run the binary against a temporary home with a fake `claude`
(`examples/fake_claude.rs`). Every acceptance criterion and edge case has a test named after it (`ac_09_…`, `ec_14_…`).
