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
    /// What the run came to: every verdict line (a line that starts with a state symbol), worst
    /// first, joined. A command that leads with its own verdict and ends with sync's (approve, add)
    /// has two; a failure in the second must never hide behind a ✓ in the first.
    pub fn verdict(&self) -> String {
        let rank = |l: &str| match l.chars().next() {
            Some('✗') => 0,
            Some('!') => 1,
            Some('✓') => 2,
            _ => 3,
        };
        let mut verdicts: Vec<&str> = self
            .lines
            .iter()
            .map(String::as_str)
            .filter(|l| rank(l) < 3 || l.starts_with('·'))
            .collect();
        if verdicts.is_empty() {
            return self
                .lines
                .iter()
                .find(|l| !l.trim().is_empty())
                .cloned()
                .unwrap_or_default();
        }
        verdicts.sort_by_key(|l| rank(l));
        verdicts.dedup();
        verdicts.join(" · ")
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
    /// `colctl add …`: what a catalog entry says to run (`colctl add plugin <id>`, `colctl add
    /// source <id>`), without the leading `colctl`.
    Add(Vec<String>),
    /// `colctl rm <kind> <name> [--codex]`
    Rm {
        kind: String,
        name: String,
        codex: bool,
    },
    /// `colctl adopt <profile> [<item>]`, once per item; everything when `items` is empty.
    Adopt { profile: String, items: Vec<String> },
    /// `colctl convert <source>`
    Convert(String),
    /// `colctl convert <source> --undo`
    ConvertUndo(String),
    /// `colctl refresh [<items>]`
    Refresh(Vec<String>),
    /// `colctl marketplace refresh`
    MarketplaceRefresh,
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
            Job::Add(words) => format!("colctl {}", words.join(" ")),
            _ => String::new(),
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
    let (out, sink) = Out::capture(width, observer);
    let mut ctx = ctx.clone();
    ctx.out = out;
    let exit = match job {
        Job::Sync(h) => commands::sync(&ctx, h.as_deref(), false),
        Job::Update(names) => commands::update(&ctx, names, false, None, false),
        Job::Approve(names) => commands::approve(&ctx, names, names.is_empty(), false),
        Job::Reject(names) => commands::reject(&ctx, names, names.is_empty(), false),
        Job::Add(words) if words.first().map(String::as_str) == Some("add") => {
            crate::cli::dispatch_words(&ctx, words)
        }
        Job::Add(words) => Err(CliError::usage(format!(
            "\"{}\" isn't an add command",
            words.join(" ")
        ))),
        _ => Err(CliError::usage("not built yet")),
    }?;
    Ok(Report {
        exit,
        lines: sink.lines(),
        errors: sink.errors(),
    })
}
