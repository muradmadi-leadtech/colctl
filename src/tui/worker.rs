//! Runs what the app asks for (`Cmd`) off the UI thread and posts the results back as `Msg`s
//! (components.md § tui::worker). Tests use [`run_now`], which does the same work inline.

use super::app::{Cmd, JobEvent, Load, Loaded, Msg};
use crate::core::observer::Observer;
use crate::core::run::{self, Job};
use crate::core::{discover, quarantine, status};
use crate::ctx::Ctx;
use crate::error::CliError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;

/// Sends a job's progress to the app.
struct Channel {
    tx: Sender<Msg>,
    cancel: Arc<AtomicBool>,
}

impl Observer for Channel {
    fn step(&mut self, profile: &str, action: &str, n: usize, total: usize) {
        let _ = self.tx.send(Msg::Job(JobEvent::Step {
            profile: profile.into(),
            action: action.into(),
            n,
            total,
        }));
    }
    fn done(&mut self, profile: &str, summary: &str, ok: bool) {
        let _ = self.tx.send(Msg::Job(JobEvent::Done {
            profile: profile.into(),
            summary: summary.into(),
            ok,
        }));
    }
    fn note(&mut self, text: &str) {
        let _ = self.tx.send(Msg::Job(JobEvent::Note(text.into())));
    }
    fn warn(&mut self, text: &str) {
        let _ = self.tx.send(Msg::Job(JobEvent::Warn(text.into())));
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

/// A panic inside a core call, as the error the app shows (EC8-15).
fn bug(payload: Box<dyn std::any::Any + Send>) -> CliError {
    let what = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown".into());
    CliError::refused(format!("colctl hit a bug\n{what}"))
        .hint("please report it at https://github.com/muradmadi-leadtech/colctl/issues")
}

fn guarded<T>(f: impl FnOnce() -> Result<T, CliError>) -> Result<T, CliError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|p| Err(bug(p)))
}

/// Does one load.
pub fn load(ctx: &Ctx, what: &Load, width: usize) -> Loaded {
    match what {
        Load::Snapshot => Loaded::Snapshot(Box::new(guarded(|| status::snapshot(ctx, None)))),
        Load::Held => Loaded::Held(guarded(|| quarantine::held(ctx))),
        Load::Catalog => Loaded::Catalog(guarded(|| discover::catalog(ctx))),
        Load::Review(name) => Loaded::Detail {
            title: name.clone(),
            lines: guarded(|| quarantine::review(ctx, name, width)),
        },
        Load::Show(id) => Loaded::Detail {
            title: id.clone(),
            lines: guarded(|| discover::show(ctx, id, width)),
        },
    }
}

/// Runs a job to the end, sending progress and the result.
pub fn job(ctx: &Ctx, job: &Job, width: usize, tx: Sender<Msg>, cancel: Arc<AtomicBool>) {
    let observer = Channel {
        tx: tx.clone(),
        cancel,
    };
    let result = guarded(|| run::run(ctx, job, width, Box::new(observer)));
    let _ = tx.send(Msg::Job(JobEvent::Finished(result)));
}

/// The runtime half that owns threads.
pub struct Worker {
    pub ctx: Ctx,
    pub tx: Sender<Msg>,
    cancel: Arc<AtomicBool>,
}

impl Worker {
    pub fn new(ctx: Ctx, tx: Sender<Msg>) -> Self {
        Worker {
            ctx,
            tx,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts `cmd` on a thread of its own. `width` is the text width for captured output.
    pub fn spawn(&mut self, cmd: Cmd, width: usize) {
        let (ctx, tx) = (self.ctx.clone(), self.tx.clone());
        match cmd {
            Cmd::Load(what) => {
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Loaded(load(&ctx, &what, width)));
                });
            }
            Cmd::Run(j) => {
                self.cancel = Arc::new(AtomicBool::new(false));
                let cancel = self.cancel.clone();
                std::thread::spawn(move || job(&ctx, &j, width, tx, cancel));
            }
            Cmd::Cancel => self.cancel.store(true, Ordering::SeqCst),
            Cmd::Quit => {}
        }
    }
}

/// Does `cmd` inline and returns the messages it would have sent (tests).
pub fn run_now(ctx: &Ctx, cmd: &Cmd, width: usize) -> Vec<Msg> {
    match cmd {
        Cmd::Load(what) => vec![Msg::Loaded(load(ctx, what, width))],
        Cmd::Run(j) => {
            let (tx, rx) = std::sync::mpsc::channel();
            job(ctx, j, width, tx, Arc::new(AtomicBool::new(false)));
            rx.try_iter().collect()
        }
        Cmd::Cancel | Cmd::Quit => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec8_15_a_panic_in_a_core_call_is_an_error() {
        let r: Result<(), CliError> = guarded(|| panic!("boom"));
        let e = r.unwrap_err();
        assert!(e.message.starts_with("colctl hit a bug\nboom"), "{e:?}");
        assert!(e.hint.unwrap().contains("report"));
        let r: Result<(), CliError> = guarded(|| panic!("{}", String::from("owned")));
        assert!(r.unwrap_err().message.ends_with("owned"));
    }
}
