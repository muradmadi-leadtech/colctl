//! What a profile has that `~/.colctl` doesn't (AC9-16): what `colctl adopt <profile>` would take,
//! read before it takes anything.

use crate::ctx::Ctx;
use crate::error::CliError;

/// One thing `adopt` would take from a profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AdoptCandidate {
    /// What `adopt` calls it: `skills/<name>`, `agents/<name>.md`, `CLAUDE.md`, `plugin <id>`.
    pub address: String,
    /// `adopt`, `move` (a plugin to its source) or `take over` (a link to another tool's folder).
    pub action: String,
    /// Where a link points, for `take over`.
    pub from: Option<String>,
}

/// What `colctl adopt <profile>` would take, without taking it.
pub fn candidates(ctx: &Ctx, profile: &str) -> Result<Vec<AdoptCandidate>, CliError> {
    let ws = crate::commands::load(ctx)?;
    crate::commands::check_harness(&ws.config, Some(profile))?;
    let mut list: Vec<AdoptCandidate> =
        crate::commands::adoptable(ctx, &ws, Some(profile), &|_| true)?
            .into_iter()
            .map(|a| AdoptCandidate {
                address: a.address,
                action: a.action.to_string(),
                from: a.from.map(|f| ctx.tilde(std::path::Path::new(&f))),
            })
            .collect();
    list.sort_by(|a, b| a.address.cmp(&b.address));
    Ok(list)
}
