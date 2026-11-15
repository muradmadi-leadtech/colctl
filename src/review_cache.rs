//! Saved checks of held versions (cycle 9, interfaces.md § review_cache): what `review` found for
//! one held version (commits, files, risk flags), so `status`, `review` and the TUI don't read every
//! changed file again on each run. Derived data only, in `.state/cache/reviews/`: a missing,
//! unreadable or mismatched entry is a miss, and deleting the folder costs one slow run.

use crate::ctx::Ctx;
use crate::review::{Commit, Direction, FileChange, Review};
use crate::risk::Flag;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// The layout of an entry; bumped when it changes.
const SCHEMA: u32 = 1;

/// Exactly what was held: a saved check is only used for the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Identity {
    /// A source plugin's held commit, against its pin.
    Source { from: String, to: String },
    /// A taken-over skill's or agent's snapshot, by content hash.
    Copy { hash: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    schema: u32,
    colctl: String,
    rules: u32,
    key: String,
    identity: Identity,
    direction: Direction,
    commits: Vec<Commit>,
    files: Vec<FileChange>,
    flags: Vec<Flag>,
}

fn dir_in(source: &Path) -> PathBuf {
    source.join(".state/cache/reviews")
}

fn path_in(source: &Path, key: &str) -> PathBuf {
    dir_in(source).join(format!("{}.json", key.replace('/', "__")))
}

fn path(ctx: &Ctx, key: &str) -> PathBuf {
    path_in(&ctx.source, key)
}

/// The saved check for `key` held as `identity`, written by this colctl with today's risk rules.
pub fn get(ctx: &Ctx, key: &str, identity: &Identity) -> Option<Review> {
    let text = fs::read_to_string(path(ctx, key)).ok()?;
    let e: Entry = serde_json::from_str(&text).ok()?;
    let current = e.schema == SCHEMA
        && e.colctl == env!("CARGO_PKG_VERSION")
        && e.rules == crate::risk::RULES_VERSION
        && e.key == key
        && &e.identity == identity;
    current.then_some(Review {
        direction: e.direction,
        commits: e.commits,
        files: e.files,
        flags: e.flags,
        diff: None,
    })
}

/// Saves a check. The diff is never saved (AC9-09). A failure only costs speed, so it's noted
/// with `-v` and otherwise ignored (EC9-04).
pub fn put(ctx: &Ctx, key: &str, identity: &Identity, review: &Review) {
    let e = Entry {
        schema: SCHEMA,
        colctl: env!("CARGO_PKG_VERSION").to_string(),
        rules: crate::risk::RULES_VERSION,
        key: key.to_string(),
        identity: identity.clone(),
        direction: review.direction,
        commits: review.commits.clone(),
        files: review.files.clone(),
        flags: review.flags.clone(),
    };
    let bytes = serde_json::to_vec(&e).unwrap_or_default();
    if let Err(err) = crate::fsx::atomic_write(&path(ctx, key), &bytes) {
        note(ctx, &format!("couldn't save the check of {key}: {err:#}"));
    }
}

/// Drops saved checks for versions no longer held (AC9-06, EC9-09). Called whenever state is saved,
/// so approve, reject and a new hold forget the old check.
pub fn prune_in<'a>(source: &Path, held: impl IntoIterator<Item = &'a String>) {
    let keep: std::collections::BTreeSet<PathBuf> =
        held.into_iter().map(|k| path_in(source, k)).collect();
    let Ok(list) = fs::read_dir(dir_in(source)) else {
        return;
    };
    for entry in list.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "json") && !keep.contains(&p) {
            let _ = fs::remove_file(p);
        }
    }
}

fn note(ctx: &Ctx, text: &str) {
    if ctx.out.caps.verbose {
        ctx.out.err_line(format!("  · {text}"));
    }
}
