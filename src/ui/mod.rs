//! Every byte of human-readable output (cycle 6, docs/cli-output-design/components.md). Commands say
//! what to show; this module decides how it looks and whether to ask. `--json` doesn't come here.

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

/// Writing to a closed pipe ends the run quietly (EC5-24, EC6-20), as a SIGPIPE would.
fn broken_pipe(e: std::io::Error) {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        std::process::exit(141);
    }
}
