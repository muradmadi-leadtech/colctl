# Interfaces: TUI

## Launch

```
colctl                      # in an interactive terminal: the TUI
colctl --color never        # the TUI without colour
colctl | cat                # the welcome help, as in 2.3
```

The TUI opens when there is no command, stdin and stdout are both terminals, `TERM` isn't `dumb` and `CI` is unset.
Global flags apply as in the CLI: `--color` and `NO_COLOR` set colour, while `-q`, `-v`, `-y` and `--no-pager` have no
effect in the TUI. There's no confirm skip.

| Variable | Effect |
|---|---|
| `NO_COLOR` | No colour (presence, any value) |
| `COLCTL_REDUCED_MOTION` | `1` (anything other than empty or `0`) turns effects off |
| `COLCTL_HOME`, `COLCTL_TOOL_TIMEOUT_SECS` | As in the CLI |
| `CI`, `TERM=dumb` | Never open the TUI |

**Exit codes:** 0 after q or Ctrl-C (whatever the state of the profiles: the TUI isn't a check, `colctl status` is),
101 after a panic. Before the UI starts, the CLI's errors and codes apply.

## Keys and mouse

As in the design spec's keymap. It's one table, `tui::keys::KEYMAP: &[Binding]`, where `Binding { key, scope, action,
label }` and `scope` is global or one tab or overlay. The key bar and `?` are generated from it, so a key that isn't in
the table doesn't exist.

## Core API (`src/core`)

```rust
pub trait Observer {
    fn step(&mut self, profile: &str, action: &str, n: usize, total: usize);
    fn done(&mut self, profile: &str, summary: &str, ok: bool);
    fn note(&mut self, text: &str);
    fn warn(&mut self, text: &str);
    /// Checked between units of work (sources in update). True stops before the next one.
    fn cancelled(&self) -> bool { false }
}

pub mod status {
    pub struct Snapshot { pub profiles: Vec<Profile>, pub source: SourceHealth, pub held: Vec<Held>,
                          pub verdict: Verdict, pub exit: i32 }
    pub fn snapshot(ctx: &Ctx, filter: Option<&str>) -> Result<Snapshot, CliError>;
}
pub mod sync {
    pub fn sync(ctx: &Ctx, filter: Option<&str>, obs: &mut dyn Observer) -> Result<SyncReport, CliError>;
}
pub mod quarantine {
    pub fn held(ctx: &Ctx) -> Result<Vec<Held>, CliError>;
    pub fn review(ctx: &Ctx, name: &str) -> Result<HeldDetail, CliError>;
    pub fn update(ctx: &Ctx, names: &[String], obs: &mut dyn Observer) -> Result<UpdateReport, CliError>;
    pub fn approve(ctx: &Ctx, names: &[String], obs: &mut dyn Observer) -> Result<ApproveReport, CliError>;
    pub fn reject(ctx: &Ctx, names: &[String], obs: &mut dyn Observer) -> Result<RejectReport, CliError>;
}
pub mod discover {
    pub fn catalog(ctx: &Ctx) -> Result<Catalog, CliError>;          // local files only, no network
    pub fn rank(plugins: &[Plugin], query: &Query) -> (Vec<Hit>, usize);
    pub fn show(ctx: &Ctx, id: &str) -> Result<PluginDetail, CliError>;
    pub fn add(ctx: &Ctx, id: &str, obs: &mut dyn Observer) -> Result<AddReport, CliError>;
}
```

Names are a guide. The rules are fixed: no function in `core` prints, prompts, reads the terminal or exits. Every
report is `Serialize`, and where the CLI has `--json` for it, that JSON is the report's serialisation (byte-identical to
2.3, per AC8-01 and AC8-03). Errors are the CLI's `CliError`, so both front ends show the same error, cause and hint.

## CLI changes

| Command | Change |
|---|---|
| `colctl` (no command) | Opens the TUI in an interactive terminal (above) |
| `colctl search <terms>` | Each term matches fuzzily (its letters in order, case-insensitive) in name, display name, description, keywords, developer or category. Ranking: exact name, name prefix, then fuzzy score. Filters and `--json` are unchanged. More results can match than in 2.3. That's the only visible change |

Everything else (text, `--json`, exit codes) is unchanged.

## TUI internals (for tests)

```rust
pub enum Msg { Key(KeyEvent), Mouse(MouseEvent), Resize(u16, u16), Tick, Job(JobEvent), Loaded(Load) }
pub enum Cmd { Load(LoadWhat), Run(Job), Quit }
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd>;   // pure: no I/O
pub fn view(model: &Model, frame: &mut Frame);            // pure: no I/O
```

Tests build a `Model` from a fixture's snapshot, feed `Msg`s, run the returned `Cmd`s with a synchronous worker, and
snapshot `view` into a `TestBackend`.
