//! Progress from a run, for whoever is watching (AC8-02). The CLI draws it as live lines on stderr;
//! the TUI turns it into events on a channel; tests record it.

/// Receives progress while a core run works.
pub trait Observer: Send {
    /// `profile` (or source) is working on step `n` of `total`.
    fn step(&mut self, profile: &str, action: &str, n: usize, total: usize);
    /// `profile` is done; `ok` is false when any of its steps failed.
    fn done(&mut self, profile: &str, summary: &str, ok: bool);
    /// Something worth knowing that isn't a problem.
    fn note(&mut self, text: &str);
    /// A warning the CLI would print on stderr.
    fn warn(&mut self, text: &str);
    /// Checked between units of work that can stop cleanly (sources in `update`).
    fn cancelled(&self) -> bool {
        false
    }
}

/// An observer that ignores everything.
pub struct Quiet;

impl Observer for Quiet {
    fn step(&mut self, _: &str, _: &str, _: usize, _: usize) {}
    fn done(&mut self, _: &str, _: &str, _: bool) {}
    fn note(&mut self, _: &str) {}
    fn warn(&mut self, _: &str) {}
}

/// One thing an observer was told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Step {
        profile: String,
        action: String,
        n: usize,
        total: usize,
    },
    Done {
        profile: String,
        summary: String,
        ok: bool,
    },
    Note(String),
    Warn(String),
}

/// Keeps every event, in order (tests, and the TUI's synchronous worker).
#[derive(Default)]
pub struct Recorder {
    pub events: Vec<Event>,
    /// When set, `cancelled` says yes once this many steps have been seen.
    pub cancel_after: Option<usize>,
}

impl Observer for Recorder {
    fn step(&mut self, profile: &str, action: &str, n: usize, total: usize) {
        self.events.push(Event::Step {
            profile: profile.into(),
            action: action.into(),
            n,
            total,
        });
    }
    fn done(&mut self, profile: &str, summary: &str, ok: bool) {
        self.events.push(Event::Done {
            profile: profile.into(),
            summary: summary.into(),
            ok,
        });
    }
    fn note(&mut self, text: &str) {
        self.events.push(Event::Note(text.into()));
    }
    fn warn(&mut self, text: &str) {
        self.events.push(Event::Warn(text.into()));
    }
    fn cancelled(&self) -> bool {
        self.cancel_after.is_some_and(|n| {
            self.events
                .iter()
                .filter(|e| matches!(e, Event::Step { .. }))
                .count()
                >= n
        })
    }
}

/// A shared observer: the caller keeps a handle and reads it after the run.
impl<O: Observer> Observer for std::sync::Arc<std::sync::Mutex<O>> {
    fn step(&mut self, profile: &str, action: &str, n: usize, total: usize) {
        if let Ok(mut o) = self.lock() {
            o.step(profile, action, n, total);
        }
    }
    fn done(&mut self, profile: &str, summary: &str, ok: bool) {
        if let Ok(mut o) = self.lock() {
            o.done(profile, summary, ok);
        }
    }
    fn note(&mut self, text: &str) {
        if let Ok(mut o) = self.lock() {
            o.note(text);
        }
    }
    fn warn(&mut self, text: &str) {
        if let Ok(mut o) = self.lock() {
            o.warn(text);
        }
    }
    fn cancelled(&self) -> bool {
        self.lock().map(|o| o.cancelled()).unwrap_or(false)
    }
}
