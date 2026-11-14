//! Runs that change things (AC8-02): sync, update, approve, reject. Each calls the code its CLI
//! command calls, with output captured instead of printed and progress sent to an observer, so
//! both front ends end in the same state (AC8-30).

use super::Observer;
use crate::commands;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::ui::Out;

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    /// The CLI command's exit code for the same run.
    pub exit: i32,
    /// The result block the CLI would print, plain, at the width asked for. The first line is the
    /// verdict.
    pub lines: Vec<String>,
    /// Error lines the CLI would print on stderr.
    pub errors: Vec<String>,
}

impl Report {
    /// The verdict line.
    pub fn verdict(&self) -> &str {
        self.lines
            .iter()
            .find(|l| !l.trim().is_empty())
            .map_or("", String::as_str)
    }
}

/// What to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    /// `colctl sync [--harness <name>]`
    Sync(Option<String>),
    /// `colctl update [<names>]`
    Update(Vec<String>),
    /// `colctl approve <names>`, or `--all` when empty
    Approve(Vec<String>),
    /// `colctl reject <names>`, or `--all` when empty
    Reject(Vec<String>),
}

impl Job {
    /// The CLI command this job is, for confirmations and parity tests.
    pub fn command(&self) -> String {
        let names = |n: &[String]| {
            if n.is_empty() {
                " --all".to_string()
            } else {
                format!(" {}", n.join(" "))
            }
        };
        match self {
            Job::Sync(None) => "colctl sync".into(),
            Job::Sync(Some(h)) => format!("colctl sync --harness {h}"),
            Job::Update(n) if n.is_empty() => "colctl update".into(),
            Job::Update(n) => format!("colctl update {}", n.join(" ")),
            Job::Approve(n) => format!("colctl approve{}", names(n)),
            Job::Reject(n) => format!("colctl reject{}", names(n)),
        }
    }
}

/// Runs `job` as its CLI command would, without printing: lines are rendered at `width` columns,
/// progress goes to `observer`. A command's refusal (lock held, nothing to approve) is an `Err`
/// with the CLI's error, cause and hint.
pub fn run(
    ctx: &Ctx,
    job: &Job,
    width: usize,
    observer: Box<dyn Observer>,
) -> Result<Report, CliError> {
    let _ = (ctx, job, width, observer, commands::sync, Out::default);
    Err(CliError::usage("not built yet"))
}
