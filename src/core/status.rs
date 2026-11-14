//! What every profile has and what needs the person (AC8-01): the data behind `colctl status` and
//! the TUI's Overview, Profiles and Items tabs.

use crate::config::Kind;
use crate::ctx::Ctx;
use crate::error::CliError;
use std::path::PathBuf;

/// How a line reads: the state tokens of the output system (`ui::style::Token`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    Ok,
    Attention,
    Failed,
    Info,
}

/// The first line: what state things are in.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Verdict {
    pub tone: Tone,
    pub text: String,
    pub facts: Vec<String>,
}

/// One item in one profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ItemRow {
    pub kind: String,
    pub name: String,
    pub state: String,
    pub tone: Tone,
    pub hint: Option<String>,
}

/// One registered profile.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Profile {
    pub name: String,
    pub kind: Kind,
    pub path: PathBuf,
    /// The path with the home folder written as `~`.
    pub display_path: String,
    /// False when the profile's folder is missing or isn't a folder.
    pub usable: bool,
    /// Why it isn't usable, and what to do: (`harness-missing`, hint).
    pub problem: Option<(String, String)>,
    pub mark: Tone,
    /// What it has, then its problems: ("32 skills", Info), ("1 missing", Attention)…
    pub contents: Vec<(String, Tone)>,
    pub items: Vec<ItemRow>,
}

/// Something that needs the person, with its fix.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Attention {
    /// A profile name, the source folder, or `held`.
    pub place: String,
    pub kind: String,
    pub name: String,
    pub what: String,
    /// The command that resolves it (may be empty).
    pub fix: String,
}

/// A version held for review.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Held {
    pub name: String,
    pub kind: String,
    /// `v1 → v2`, an origin path, or the conversion it is.
    pub change: String,
}

/// Everything `colctl status` knows, at one moment.
#[derive(Debug)]
pub struct Snapshot {
    pub verdict: Verdict,
    pub profiles: Vec<Profile>,
    pub attention: Vec<Attention>,
    pub held: Vec<Held>,
    /// The number of declared source plugins.
    pub sources: usize,
    /// `colctl status`'s exit code for this state.
    pub exit: i32,
}

/// Reads the source and every profile (or only `filter`). Never writes.
pub fn snapshot(_ctx: &Ctx, _filter: Option<&str>) -> Result<Snapshot, CliError> {
    Err(CliError::usage("not built yet"))
}
