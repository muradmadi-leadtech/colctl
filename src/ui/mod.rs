//! Every byte of human-readable output (cycle 6, docs/cli-output-design/components.md). Commands say
//! what to show; this module decides how it looks and whether to ask. `--json` doesn't come here.

pub mod ask;
pub mod progress;
pub mod render;
pub mod style;
pub mod term;

use crate::core::Observer;
use crate::error::CliError;
use std::io::Write;
use std::sync::{Arc, Mutex};
use term::{Caps, Facts, Settings};

/// The handle commands print through. With a [`Sink`] (the core API, cycle 8), nothing reaches the
/// terminal: lines are kept, warnings and progress go to the sink's observer, and nothing is asked.
#[derive(Clone)]
pub struct Out {
    pub caps: Caps,
    sink: Option<Arc<Sink>>,
}

impl std::fmt::Debug for Out {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Out")
            .field("caps", &self.caps)
            .field("captured", &self.sink.is_some())
            .finish()
    }
}

/// Where a captured run's output goes.
pub struct Sink {
    lines: Mutex<Vec<String>>,
    errors: Mutex<Vec<String>>,
    observer: Mutex<Box<dyn Observer>>,
}

impl Sink {
    /// The result lines so far (what stdout would have shown).
    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Error lines (what stderr would have shown, warnings aside).
    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().map(|l| l.clone()).unwrap_or_default()
    }

    fn observe(&self, f: impl FnOnce(&mut dyn Observer)) {
        if let Ok(mut o) = self.observer.lock() {
            f(o.as_mut());
        }
    }
}

impl Default for Out {
    fn default() -> Self {
        Out {
            caps: Caps::plain(),
            sink: None,
        }
    }
}

impl Out {
    /// From the global flags and the real terminal.
    pub fn new(settings: &Settings) -> Self {
        Out {
            caps: Caps::new(settings, &Facts::detect()),
            sink: None,
        }
    }

    /// Captures everything at `width` columns, without colour, prompts or pager; confirmations
    /// answer yes (the front end asked before calling).
    pub fn capture(width: usize, observer: Box<dyn Observer>) -> (Self, Arc<Sink>) {
        let mut caps = Caps::plain();
        caps.width = width;
        caps.yes = true;
        let sink = Arc::new(Sink {
            lines: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
            observer: Mutex::new(observer),
        });
        (
            Out {
                caps,
                sink: Some(sink.clone()),
            },
            sink,
        )
    }

    /// True when this run is captured (no terminal of its own).
    pub fn is_captured(&self) -> bool {
        self.sink.is_some()
    }

    /// Progress: `profile` is on step `n` of `total`. Only a captured run hears it here; the CLI
    /// draws its own ([`progress::Run`]).
    pub fn step(&self, profile: &str, what: &str, n: usize, total: usize) {
        if let Some(s) = &self.sink {
            s.observe(|o| o.step(profile, what, n, total));
        }
    }

    /// Progress: `profile` is done.
    pub fn done(&self, profile: &str, summary: &str, ok: bool) {
        if let Some(s) = &self.sink {
            s.observe(|o| o.done(profile, summary, ok));
        }
    }

    /// True when the watcher asked to stop before the next unit of work.
    pub fn cancelled(&self) -> bool {
        self.sink
            .as_ref()
            .is_some_and(|s| s.observer.lock().map(|o| o.cancelled()).unwrap_or(false))
    }

    /// A line of results on stdout.
    pub fn line(&self, text: impl AsRef<str>) {
        if let Some(s) = &self.sink {
            if let Ok(mut l) = s.lines.lock() {
                l.push(text.as_ref().to_string());
            }
            return;
        }
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{}", text.as_ref()).map_err(broken_pipe);
    }

    /// Several result lines.
    pub fn lines<I: IntoIterator<Item = S>, S: AsRef<str>>(&self, lines: I) {
        for l in lines {
            self.line(l);
        }
    }

    /// Text exactly as given (already newline-terminated), e.g. a diff or a generated script.
    pub fn raw(&self, text: &str) {
        if self.sink.is_some() {
            for l in text.lines() {
                self.line(l);
            }
            return;
        }
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes()).map_err(broken_pipe);
    }

    /// A line on stderr (progress, warnings, errors).
    pub fn err_line(&self, text: impl AsRef<str>) {
        if let Some(s) = &self.sink {
            if let Ok(mut l) = s.errors.lock() {
                l.push(text.as_ref().to_string());
            }
            return;
        }
        let _ = writeln!(std::io::stderr().lock(), "{}", text.as_ref());
    }

    /// A warning on stderr.
    pub fn warn(&self, text: &str) {
        if let Some(s) = &self.sink {
            s.observe(|o| o.warn(text));
            return;
        }
        self.err_line(render::warning(&self.caps, text));
    }

    /// An error on stderr in the error layout (AC6-06), with `fallback` as the hint when the error
    /// has none.
    pub fn error(&self, err: &CliError, fallback: &str) {
        let hint = err.hint.as_deref().unwrap_or(fallback);
        let (what, cause) = match err.message.split_once('\n') {
            Some((w, c)) => (w, Some(c)),
            None => (err.message.as_str(), None),
        };
        for l in render::error(&self.caps, what, cause, hint) {
            self.err_line(l);
        }
    }
}

impl Out {
    /// Asks a yes/no question in a terminal (AC6-18); see [`ask::confirm`].
    pub fn confirm(&self, question: &str) -> ask::Answer {
        ask::confirm(&self.caps, question, &ask::terminal_confirm)
    }

    /// A multi-select picker in a terminal (AC6-19); `None` without one or when interrupted.
    pub fn pick_many(&self, title: &str, items: &[String]) -> Option<Vec<usize>> {
        ask::pick(&self.caps, title, items, &ask::terminal_pick_many)
    }

    /// A fuzzy single-select picker in a terminal (AC6-19).
    pub fn pick_one(&self, title: &str, items: &[String]) -> Option<usize> {
        ask::pick(&self.caps, title, items, &ask::terminal_pick_one)
            .and_then(|v| v.first().copied())
    }

    /// Prints `lines`, through the pager when they're taller than the terminal (AC6-20, EC6-15).
    pub fn page(&self, lines: Vec<String>) {
        if self.sink.is_none() && ask::should_page(&self.caps, lines.len()) {
            let command = ask::pager_command(std::env::var("PAGER").ok().as_deref());
            let mut text = lines.join("\n");
            text.push('\n');
            if ask::run_pager(&command, &text).is_ok() {
                return;
            }
        }
        self.lines(lines);
    }
}

/// Writing to a closed pipe ends the run quietly (EC5-24, EC6-20), as a SIGPIPE would.
fn broken_pipe(e: std::io::Error) {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        std::process::exit(141);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::observer::{Event, Recorder};
    use std::sync::{Arc, Mutex};

    #[test]
    fn ac8_02_a_captured_out_never_prints_or_asks() {
        let rec = Arc::new(Mutex::new(Recorder::default()));
        let (out, sink) = Out::capture(60, Box::new(rec.clone()));
        assert!(out.is_captured());
        assert_eq!(out.caps.width, 60);
        assert!(!out.caps.color_out && !out.caps.progress && !out.caps.interactive);
        out.line("one");
        out.lines(["two"]);
        out.raw("three\nfour\n");
        out.page(vec!["five".into()]);
        out.err_line("bad");
        out.error(&CliError::usage("worse"), "colctl --help");
        out.warn("careful");
        out.step("claude", "linking", 1, 2);
        out.done("claude", "1 change", true);
        assert!(matches!(out.confirm("Sure?"), ask::Answer::Yes));
        assert_eq!(out.pick_one("Which?", &["a".into()]), None);
        assert!(!out.cancelled());
        assert_eq!(sink.lines(), ["one", "two", "three", "four", "five"]);
        assert_eq!(sink.errors()[0], "bad");
        assert!(sink.errors().iter().any(|l| l.contains("worse")));
        let ev = rec.lock().unwrap().events.clone();
        assert_eq!(
            ev,
            [
                Event::Warn("careful".into()),
                Event::Step {
                    profile: "claude".into(),
                    action: "linking".into(),
                    n: 1,
                    total: 2
                },
                Event::Done {
                    profile: "claude".into(),
                    summary: "1 change".into(),
                    ok: true
                },
            ]
        );
        assert!(format!("{out:?}").contains("captured: true"));
        assert!(!Out::default().is_captured());
        Out::default().step("x", "y", 1, 1);
        Out::default().done("x", "y", true);
        assert!(!Out::default().cancelled());
    }
}
