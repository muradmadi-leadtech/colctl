//! Quarantine (cycle 4): `update` and `refresh` hold new versions as candidates; `review`, `approve`
//! and `reject` decide them. Nothing held reaches a harness until it's approved.

use super::{EXIT_DRIFT, Workspace, load, plugins_dir};
use crate::config;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::git::Git;
use crate::state::{self, Candidate, HeldCommit, Lock};
use crate::vault::{self, Vault};

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
