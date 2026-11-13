//! Every byte of human-readable output (cycle 6, docs/cli-output-design/components.md). Commands say
//! what to show; this module decides how it looks and whether to ask. `--json` doesn't come here.

pub mod ask;
pub mod progress;
pub mod render;
pub mod style;
pub mod term;

use crate::error::CliError;
use std::io::Write;
use term::{Caps, Facts, Settings};

/// The handle commands print through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Out {
    pub caps: Caps,
}

impl Default for Out {
    fn default() -> Self {
        Out {
            caps: Caps::plain(),
        }
    }
}

impl Out {
    /// From the global flags and the real terminal.
    pub fn new(settings: &Settings) -> Self {
        Out {
            caps: Caps::new(settings, &Facts::detect()),
        }
    }

    /// A line of results on stdout.
    pub fn line(&self, text: impl AsRef<str>) {
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
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(text.as_bytes()).map_err(broken_pipe);
    }

    /// A line on stderr (progress, warnings, errors).
    pub fn err_line(&self, text: impl AsRef<str>) {
        let _ = writeln!(std::io::stderr().lock(), "{}", text.as_ref());
    }

    /// A warning on stderr.
    pub fn warn(&self, text: &str) {
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
        if ask::should_page(&self.caps, lines.len()) {
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
