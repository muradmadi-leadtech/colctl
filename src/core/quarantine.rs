//! What's held for review (AC8-04): the list the TUI's Review tab shows, and one held version in
//! full, exactly as `colctl review` shows them.

use super::observer::Quiet;
use crate::commands::quarantine as q;
use crate::commands::{self};
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::git::Git;
use crate::ui::Out;

/// One held version, as `colctl review` lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HeldSummary {
    pub name: String,
    /// `source`, `skill`, `agent` or `conversion`.
    pub kind: String,
    /// `v1 → v2`, an origin path, or the conversion it is.
    pub change: String,
    /// `3 commits`, `goes back 2 commits`, `4 files`; empty when it can't be read.
    pub size: String,
    /// Risk flags found.
    pub flags: usize,
    /// Why it can't be approved, when it can't.
    pub error: Option<String>,
}

/// Every held version, in `colctl review`'s order.
pub fn held(ctx: &Ctx) -> Result<Vec<HeldSummary>, CliError> {
    let git = Git::for_ctx(ctx)?;
    let ws = commands::load(ctx)?;
    let snap_held = |v: &q::View| super::status::held_of(ctx, v);
    Ok(q::held_views(ctx, &ws, Some(&git))
        .iter()
        .map(|v| {
            let h = snap_held(v);
            HeldSummary {
                name: h.name,
                kind: h.kind,
                change: h.change,
                size: v
                    .review
                    .as_ref()
                    .map(|r| q::size_text(v, r))
                    .unwrap_or_default(),
                flags: v.review.as_ref().map_or(0, |r| r.flags.len()),
                error: v.error.clone(),
            }
        })
        .collect())
}

/// `colctl review <name>` as plain lines at `width`: the verdict, commits, files, risk flags and the
/// diff.
pub fn review(ctx: &Ctx, name: &str, width: usize) -> Result<Vec<String>, CliError> {
    let (out, sink) = Out::capture(width, Box::new(Quiet));
    let mut ctx = ctx.clone();
    ctx.out = out;
    commands::review(&ctx, Some(name), false, false)?;
    Ok(sink.lines())
}
