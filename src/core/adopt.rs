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
pub fn candidates(_ctx: &Ctx, _profile: &str) -> Result<Vec<AdoptCandidate>, CliError> {
    Ok(Vec::new())
}
