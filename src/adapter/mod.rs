//! Reading harnesses. Types and link scanning shared by every kind live here; each kind's
//! specifics are in its own module.
//!
//! Only the read side lives here; deciding what to do is [`crate::plan`] and doing it is
//! [`crate::apply`].

pub mod claude;

pub use claude::{CLAUDE_MD, generated_claude_md, marketplace_source, read_plugins, scan};

use crate::fsx;
use crate::source::visible_entries;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The harness folder itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    Ok,
    /// Doesn't exist (EC-06).
    Missing,
    /// Exists but isn't a real folder (EC-07).
    Invalid,
}

/// One `skills/<name>` or `agents/<name>.md` entry in a harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A symlink into the source. `correct` when it points at the item of the same name and that
    /// item exists.
    Owned { target: PathBuf, correct: bool },
    /// A symlink pointing anywhere else (EC-14).
    ForeignLink(PathBuf),
    /// A real file or folder.
    Real,
}

/// The harness's `CLAUDE.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instructions {
    Absent,
    Link,
    File { content: Vec<u8>, hash: String },
}

/// Plugins and marketplaces as Claude recorded them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugins {
    pub installed: BTreeSet<String>,
    /// Marketplace name → its entry in `known_marketplaces.json`.
    pub marketplaces: BTreeMap<String, Value>,
}

/// Everything colctl needs to know about one harness.
#[derive(Debug, Clone, PartialEq)]
pub struct Scan {
    pub root: Root,
    pub instructions: Instructions,
    pub skills: BTreeMap<String, Entry>,
    pub agents: BTreeMap<String, Entry>,
    /// `Err` with the reason when Claude's plugin files can't be read (EC-13).
    pub plugins: Result<Plugins, String>,
    /// Leftovers from an interrupted run (EC-21).
    pub temps: Vec<PathBuf>,
}

impl Scan {
    pub(crate) fn empty(root: Root) -> Self {
        Self {
            root,
            instructions: Instructions::Absent,
            skills: BTreeMap::new(),
            agents: BTreeMap::new(),
            plugins: Ok(Plugins::default()),
            temps: Vec::new(),
        }
    }
}

/// `Some(Missing | Invalid)` when the harness folder can't be used, `None` when it can.
pub(crate) fn root_of(harness: &Path) -> Option<Root> {
    match fs::symlink_metadata(harness) {
        Err(_) => Some(Root::Missing),
        Ok(meta) if !meta.is_dir() => Some(Root::Invalid),
        Ok(_) => None,
    }
}

/// A test for "this link target is inside the source", by path as written or canonical.
pub(crate) fn owned_by(source: &Path) -> impl Fn(&Path) -> bool + use<> {
    let source = source.to_path_buf();
    let canonical = fsx::canonical_or_normal(&source);
    move |target: &Path| target.starts_with(&source) || target.starts_with(&canonical)
}

/// Scans `dir` for entries named `<name><suffix>` (dot-entries and `skip` ignored) and classifies
/// each against `<source_dir>/<name><suffix>`.
pub(crate) fn link_entries(
    dir: &Path,
    source_dir: &Path,
    suffix: &str,
    skip: &[&str],
    owned: &dyn Fn(&Path) -> bool,
    kind_ok: fn(&Path) -> bool,
) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    for file in visible_entries(dir) {
        if skip.contains(&file.as_str()) {
            continue;
        }
        let Some(name) = file.strip_suffix(suffix) else {
            continue;
        };
        out.insert(
            name.to_string(),
            entry(&dir.join(&file), &source_dir.join(&file), owned, kind_ok),
        );
    }
    out
}

/// Leftover temporary entries in the harness folder and the given subfolders (EC-21).
pub(crate) fn temps_in(harness: &Path, subdirs: &[&str]) -> Vec<PathBuf> {
    let mut temps = fsx::find_temps(harness);
    for sub in subdirs {
        temps.extend(fsx::find_temps(&harness.join(sub)));
    }
    temps
}

pub(crate) fn entry(
    path: &Path,
    expected: &Path,
    owned_root: &dyn Fn(&Path) -> bool,
    kind_ok: fn(&Path) -> bool,
) -> Entry {
    match fsx::link_target(path) {
        Some(target) if owned_root(&target) => {
            let correct = (target == expected
                || fsx::canonical_or_normal(&target) == fsx::canonical_or_normal(expected))
                && kind_ok(expected);
            Entry::Owned { target, correct }
        }
        Some(target) => Entry::ForeignLink(target),
        None => Entry::Real,
    }
}
