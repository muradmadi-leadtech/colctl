//! The full-screen UI (cycle 8, docs/tui): bare `colctl` in an interactive terminal. It shows and
//! runs only what the CLI already does, through [`crate::core`].

pub mod app;
pub mod keys;
pub mod motion;
pub mod tabs;
pub mod terminal;
pub mod theme;
pub mod view;
pub mod worker;

use crate::ctx::Ctx;
use crate::ui::term::{ColorChoice, Facts, Settings};
use app::{Cmd, Model, Msg, Options};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event};
use std::time::{Duration, Instant};

fn set(v: &Option<String>) -> bool {
    v.as_deref().is_some_and(|s| !s.is_empty())
}

/// The launch rule (AC8-06): the TUI opens for bare `colctl` only when stdin and stdout are
/// terminals, `TERM` isn't `dumb` and `CI` is unset. Returns how to open it, or `None` for the
/// welcome help.
pub fn launch(s: &Settings, f: &Facts, reduced_motion: Option<&str>) -> Option<Options> {
    let opens =
        f.stdin_tty && f.stdout_tty && f.term.as_deref() != Some("dumb") && !set(&f.ci) && !s.json;
    opens.then(|| Options {
        color: match s.color {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => !set(&f.no_color),
        },
        motion: reduced_motion.is_none_or(|v| v.is_empty() || v == "0"),
    })
}

/// Runs the TUI until the person quits. Exit code 0, or 101 after a bug (AC8-07).
pub fn run(ctx: &Ctx, opts: Options) -> i32 {
    // A panic restores the terminal first, then is reported on the normal screen.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| terminal::restore()));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_loop(ctx, opts)));
    std::panic::set_hook(previous);
    terminal::restore();
    match result {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => {
            let err = crate::error::CliError::refused(format!("the terminal stopped working\n{e}"));
            ctx.out
                .error(&err, "run colctl status to see where things are");
            1
        }
        Err(payload) => {
            let what = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let err = crate::error::CliError::refused(format!("colctl hit a bug\n{what}"));
            ctx.out.error(
                &err,
                "please report it at https://github.com/muradmadi-leadtech/colctl/issues",
            );
            101
        }
    }
}

fn run_loop(ctx: &Ctx, opts: Options) -> std::io::Result<()> {
    let _guard = terminal::Guard::enter()?;
    let mut term = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
    let size = term.size()?;
    let mut model = Model::new(opts, size.width, size.height);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut worker = worker::Worker::new(ctx.clone(), tx);
    let started = Instant::now();
    let width = |m: &Model| usize::from(m.width.saturating_sub(4)).max(40);
    for cmd in model.start() {
        worker.spawn(cmd, width(&model));
    }
    loop {
        let mut cmds: Vec<Cmd> = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            cmds.extend(app::update(&mut model, msg));
        }
        let now = started.elapsed().as_millis() as u64;
        cmds.extend(app::update(&mut model, Msg::Tick(now)));
        if model.dirty {
            term.draw(|f| view::draw(&model, f))?;
            model.dirty = false;
        }
        for cmd in cmds.drain(..) {
            worker.spawn(cmd, width(&model));
        }
        if model.quit {
            return Ok(());
        }
        // Fast while something moves or runs; otherwise wake rarely (idle, AC8-24).
        let busy = model.job.is_some() || motion::active(&model);
        let wait = Duration::from_millis(if busy { 33 } else { 250 });
        if event::poll(wait)? {
            let msg = match event::read()? {
                Event::Key(k) if k.kind == event::KeyEventKind::Press => Some(Msg::Key(k)),
                Event::Mouse(m) => Some(Msg::Mouse(m)),
                Event::Resize(w, h) => Some(Msg::Resize(w, h)),
                Event::Paste(p) => Some(Msg::Paste(p)),
                _ => None,
            };
            if let Some(msg) = msg {
                for cmd in app::update(&mut model, msg) {
                    worker.spawn(cmd, width(&model));
                }
            }
        }
    }
}
