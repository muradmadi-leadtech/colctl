//! Quarantine (cycle 4): `update` and `refresh` hold new versions as candidates; `review`, `approve`
//! and `reject` decide them. Nothing held reaches a harness until it's approved.

use super::{Copier, EXIT_DRIFT, Workspace, hash_item, load, plugins_dir, real_copy};
use crate::config;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::fsx;
use crate::git::Git;
use crate::state::{self, Candidate, HeldCommit, HeldCopy, Lock, State};
use crate::vault::{self, Vault};
use std::fs;
use std::path::Path;

/// The candidate key of a source plugin.
pub(super) fn source_key(name: &str) -> String {
    format!("sources/{name}")
}

/// "n new commits" or "goes back n commits" from `from` to `to`.
fn distance(vault: &Vault, name: &str, from: &str, to: &str) -> String {
    let ahead = vault.count_between(name, from, to);
    if ahead == 0 {
        let back = vault.count_between(name, to, from);
        let word = if back == 1 { "commit" } else { "commits" };
        return format!("goes back {back} {word}");
    }
    let word = if ahead == 1 { "commit" } else { "commits" };
    format!("{ahead} new {word}")
}

/// What `update` found for one source.
enum Found {
    UpToDate,
    Rejected,
    AlreadyHeld,
    New { replaces: Option<String> },
}

fn classify(ws: &Workspace, s: &config::Source, new: &str) -> Found {
    let key = source_key(&s.name);
    if new == s.rev {
        return Found::UpToDate;
    }
    if ws.state.is_rejected(&key, new) {
        return Found::Rejected;
    }
    match ws.state.quarantine.get(&key) {
        Some(Candidate::Source(held)) if held.to == new => Found::AlreadyHeld,
        Some(Candidate::Source(held)) => Found::New {
            replaces: Some(held.to.clone()),
        },
        _ => Found::New { replaces: None },
    }
}

/// `colctl update` (AC4-11…16): fetches each source and holds a newer commit for review. It never
/// moves a pin, commits, or calls `claude`/`codex`; `approve` does that.
pub fn update(
    ctx: &Ctx,
    names: &[String],
    check: bool,
    rev: Option<&str>,
    dry_run: bool,
) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    for n in names {
        if !ws.config.sources.iter().any(|s| &s.name == n) {
            return Err(CliError::usage(format!("no source named \"{n}\"")));
        }
    }
    if rev.is_some() && names.len() != 1 {
        return Err(CliError::usage("--rev needs exactly one source name"));
    }
    if ws.config.sources.is_empty() {
        println!("no sources declared");
        return Ok(0);
    }
    let _lock = if check || dry_run {
        None
    } else {
        Some(Lock::acquire(&ctx.source)?)
    };
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    let mut st = state::load(&ctx.source)?;
    let mut failures = 0;
    let mut newer = 0;
    for s in ws
        .config
        .sources
        .iter()
        .filter(|s| names.is_empty() || names.contains(&s.name))
    {
        let looked = vault
            .fetch_clone(&s.name, &s.clone_url())
            .and_then(|_| match rev {
                Some(r) => vault.resolve(&s.name, r),
                None => vault.upstream_head(&s.name),
            });
        let new = match looked {
            Ok(new) => new,
            Err(e) if rev.is_some() => return Err(CliError::usage(e)),
            Err(e) => {
                failures += 1;
                println!("{}: {e}", s.name);
                continue;
            }
        };
        let found = classify(&ws, s, &new);
        if matches!(found, Found::UpToDate) {
            println!("{}: up to date", s.name);
            continue;
        }
        let change = format!(
            "{}: {} → {} ({})",
            s.name,
            vault::short(&s.rev),
            vault::short(&new),
            distance(&vault, &s.name, &s.rev, &new)
        );
        if check {
            let note = match found {
                Found::Rejected => " (rejected)",
                Found::AlreadyHeld => " (held for review)",
                _ => "",
            };
            if !matches!(found, Found::Rejected) {
                newer += 1;
            }
            println!("{change}{note}");
            continue;
        }
        match found {
            Found::Rejected => {
                println!(
                    "{}: {} was rejected; nothing held",
                    s.name,
                    vault::short(&new)
                );
            }
            Found::AlreadyHeld => {
                println!("{}: {} already held for review", s.name, vault::short(&new));
            }
            Found::New { replaces } => {
                let replacing = replaces
                    .map(|old| format!(" (replaces {})", vault::short(&old)))
                    .unwrap_or_default();
                if dry_run {
                    println!("would hold {change}{replacing}");
                    continue;
                }
                if let Err(e) = vault.hold(&s.name, &new) {
                    failures += 1;
                    println!("{}: {e}", s.name);
                    continue;
                }
                st.quarantine.insert(
                    source_key(&s.name),
                    Candidate::Source(HeldCommit {
                        from: s.rev.clone(),
                        to: new.clone(),
                        held_at: state::now(),
                    }),
                );
                state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
                println!(
                    "{change}, held for review{replacing}; colctl review {}",
                    s.name
                );
            }
            Found::UpToDate => {}
        }
    }
    Ok(if failures > 0 || (check && newer > 0) {
        EXIT_DRIFT
    } else {
        0
    })
}

/// Copies `origin` into `snapshot` through a staging path and checks the copy hashes to `expected`.
/// On any failure nothing is left behind (EC4-18).
pub(super) fn snapshot(
    origin: &Path,
    snapshot: &Path,
    is_dir: bool,
    expected: &str,
    copy: &Copier,
) -> Result<(), String> {
    let parent = snapshot
        .parent()
        .ok_or_else(|| format!("{} has no parent", snapshot.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
    let name = snapshot
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let staging = parent.join(format!("{}hold-{name}", fsx::TMP_PREFIX));
    let _ = fsx::remove_any(&staging);
    let copied = copy(origin, &staging)
        .ok()
        .and_then(|()| hash_item(&staging, is_dir).ok());
    if copied.as_deref() != Some(expected) {
        let _ = fsx::remove_any(&staging);
        return Err("the copy didn't match the origin; nothing held".into());
    }
    let _ = fsx::remove_any(snapshot);
    fs::rename(&staging, snapshot).map_err(|e| {
        let _ = fsx::remove_any(&staging);
        format!("moving the snapshot into place: {e}")
    })
}

/// What `refresh` does with one taken-over item.
fn refresh_one(ctx: &Ctx, st: &mut State, key: &str, copy: &Copier) -> Result<String, String> {
    let Some(origin) = st.origins.get(key).cloned() else {
        return Err(format!("{key} has no recorded origin"));
    };
    let is_dir = key.starts_with("skills/");
    let source_item = ctx.source.join(key);
    if !fsx::exists_any(&origin.path) {
        return Err(format!("origin is gone ({})", origin.path.display()));
    }
    let (Ok(theirs), Ok(ours)) = (
        hash_item(&origin.path, is_dir),
        hash_item(&source_item, is_dir),
    ) else {
        return Err("couldn't read the origin or the source copy".into());
    };
    if theirs == origin.hash {
        return Ok(format!(
            "{key} is up to date with {}",
            origin.path.display()
        ));
    }
    if ours != origin.hash {
        return Err(format!(
            "changed in both places; compare {} and {}, keep one, then adopt again",
            origin.path.display(),
            source_item.display()
        ));
    }
    if st.is_rejected(key, &theirs) {
        return Ok(format!("{key}: this version was rejected; nothing held"));
    }
    let replacing = match st.quarantine.get(key) {
        Some(Candidate::Skill(c) | Candidate::Agent(c)) if c.hash == theirs => {
            return Ok(format!("{key} already held for review"));
        }
        Some(_) => " (replaces the copy held before)",
        None => "",
    };
    snapshot(
        &origin.path,
        &state::snapshot_path(&ctx.source, key),
        is_dir,
        &theirs,
        copy,
    )?;
    let held = HeldCopy {
        origin: origin.path.clone(),
        base: ours,
        hash: theirs,
        held_at: state::now(),
    };
    st.quarantine.insert(
        key.to_string(),
        if is_dir {
            Candidate::Skill(held)
        } else {
            Candidate::Agent(held)
        },
    );
    Ok(format!(
        "held {key} for review{replacing}; colctl review {key}"
    ))
}

/// `colctl refresh` (AC4-17, AC4-18): snapshots taken-over items whose origin changed and holds them
/// for review. The source copy, its recorded hash and the history stay as they are.
pub fn refresh(ctx: &Ctx, items: &[String], dry_run: bool) -> Result<i32, CliError> {
    let ws = load(ctx)?;
    let keys: Vec<String> = if items.is_empty() {
        ws.state.origins.keys().cloned().collect()
    } else {
        items.to_vec()
    };
    for key in &keys {
        if !ws.state.origins.contains_key(key) {
            return Err(CliError::usage(format!("{key} has no recorded origin")));
        }
    }
    if keys.is_empty() {
        println!("nothing to refresh: no adopted item has a recorded origin");
    }
    if dry_run {
        for (key, origin) in ws.origin_changed.iter().filter(|(k, _)| keys.contains(k)) {
            println!("would hold {key} from {}", origin.display());
        }
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    let mut st = state::load(&ctx.source)?;
    let mut refused = 0;
    for key in &keys {
        let is_dir = key.starts_with("skills/");
        match refresh_one(ctx, &mut st, key, &real_copy(is_dir)) {
            Ok(line) => println!("{line}"),
            Err(why) => {
                refused += 1;
                println!("refused {key}: {why}");
            }
        }
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    Ok(if refused > 0 { EXIT_DRIFT } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ec4_18_a_bad_snapshot_copy_holds_nothing_and_leaves_nothing() {
        let t = tempfile::tempdir().unwrap();
        let origin = t.path().join("o/skill");
        fs::create_dir_all(&origin).unwrap();
        fs::write(origin.join("SKILL.md"), "real").unwrap();
        let expected = fsx::hash_tree(&origin).unwrap();
        let snap = t.path().join("q/skills/skill");
        let corrupting = |from: &Path, to: &Path| -> anyhow::Result<()> {
            fsx::copy_tree(from, to, false)?;
            fs::write(to.join("SKILL.md"), "tampered")?;
            Ok(())
        };
        assert_eq!(
            snapshot(&origin, &snap, true, &expected, &corrupting),
            Err("the copy didn't match the origin; nothing held".into())
        );
        let failing = |_: &Path, _: &Path| -> anyhow::Result<()> { anyhow::bail!("disk full") };
        assert!(snapshot(&origin, &snap, true, &expected, &failing).is_err());
        assert!(!snap.exists());
        assert!(fsx::find_temps(snap.parent().unwrap()).is_empty());
        snapshot(&origin, &snap, true, &expected, &real_copy(true)).unwrap();
        assert_eq!(fs::read_to_string(snap.join("SKILL.md")).unwrap(), "real");
        // A second hold replaces the first.
        fs::write(origin.join("SKILL.md"), "newer").unwrap();
        let newer = fsx::hash_tree(&origin).unwrap();
        snapshot(&origin, &snap, true, &newer, &real_copy(true)).unwrap();
        assert_eq!(fs::read_to_string(snap.join("SKILL.md")).unwrap(), "newer");
    }
}
