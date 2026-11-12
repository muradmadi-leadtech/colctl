//! Quarantine (cycle 4): `update` and `refresh` hold new versions as candidates; `review`, `approve`
//! and `reject` decide them. Nothing held reaches a harness until it's approved.

use super::{
    Copier, EXIT_DRIFT, Workspace, commit, commit_pending, hash_item, load, plugins_dir, real_copy,
    run_sync, write_config,
};
use crate::config;
use crate::ctx::Ctx;
use crate::error::CliError;
use crate::fsx;
use crate::git::Git;
use crate::review::{self, Direction, Review, escape, escape_body};
use crate::state::{self, Candidate, HeldCommit, HeldCopy, Lock, State};
use crate::vault::{self, CloneState, Vault};
use serde_json::{Value, json};
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

/// A held candidate as `review`, `status` and `approve` see it.
pub(super) struct View {
    pub key: String,
    pub name: String,
    pub candidate: Candidate,
    pub repo: Option<String>,
    pub review: Option<Review>,
    /// Why the candidate can't be used (EC4-12, EC4-13).
    pub error: Option<String>,
    /// Something to know before approving (EC4-14).
    pub warning: Option<String>,
}

/// The short name shown for a key: the source name, or the key itself for skills and agents.
fn name_of(key: &str) -> String {
    key.strip_prefix("sources/").unwrap_or(key).to_string()
}

/// Turns a name from the command line into a candidate key, checking it names something colctl
/// knows (EC4-03).
pub(super) fn resolve(ws: &Workspace, name: &str) -> Result<String, CliError> {
    if name.contains('/') {
        if ws.state.origins.contains_key(name) || ws.state.quarantine.contains_key(name) {
            return Ok(name.to_string());
        }
        return Err(CliError::usage(format!(
            "no taken-over item \"{name}\"; use a source name, skills/<name> or agents/<name>.md"
        )));
    }
    if ws.config.sources.iter().any(|s| s.name == name) {
        return Ok(source_key(name));
    }
    Err(CliError::usage(format!("no source named \"{name}\"")))
}

/// Reads one candidate: checks it's intact, and gathers its review (with the diff when asked).
pub(super) fn inspect(
    ctx: &Ctx,
    ws: &Workspace,
    git: &Git,
    key: &str,
    candidate: &Candidate,
    with_diff: bool,
) -> View {
    let name = name_of(key);
    let mut view = View {
        key: key.to_string(),
        name: name.clone(),
        candidate: candidate.clone(),
        repo: None,
        review: None,
        error: None,
        warning: None,
    };
    let result = match candidate {
        Candidate::Source(c) => {
            let declared = ws.config.sources.iter().find(|s| s.name == name);
            view.repo = declared.map(|s| s.repo.clone());
            if let Some(s) = declared.filter(|s| s.rev != c.from) {
                view.warning = Some(format!(
                    "{name}'s pin changed since it was held (now {}); run colctl update {name} again",
                    vault::short(&s.rev)
                ));
            }
            let vault = Vault {
                git,
                dir: plugins_dir(ctx),
            };
            if vault.held(&name).as_deref() != Some(c.to.as_str()) {
                Err(format!(
                    "the held commit for {name} is missing; run colctl update {name}"
                ))
            } else {
                review::commits(git, &vault.path(&name), &c.from, &c.to, with_diff)
            }
        }
        Candidate::Skill(c) | Candidate::Agent(c) => {
            let is_dir = matches!(candidate, Candidate::Skill(_));
            let held = state::snapshot_path(&ctx.source, key);
            if hash_item(&held, is_dir).ok().as_deref() != Some(c.hash.as_str()) {
                Err(format!(
                    "the held copy of {key} changed since it was held; reject it and refresh again"
                ))
            } else {
                review::copies(
                    git,
                    &ctx.source,
                    &ctx.source.join(key),
                    &held,
                    is_dir,
                    with_diff,
                )
            }
        }
    };
    match result {
        Ok(r) => view.review = Some(r),
        Err(e) => view.error = Some(e),
    }
    view
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "from → to" for a candidate: short commits, or the origin for a held copy.
fn change_text(ctx: &Ctx, c: &Candidate) -> String {
    match c {
        Candidate::Source(c) => format!("{} → {}", vault::short(&c.from), vault::short(&c.to)),
        Candidate::Skill(c) | Candidate::Agent(c) => ctx.tilde(&c.origin),
    }
}

fn summary_line(ctx: &Ctx, v: &View) -> String {
    let what = change_text(ctx, &v.candidate);
    let Some(r) = &v.review else {
        return format!(
            "{:<22}{what}   error: {}",
            escape(&v.key),
            escape(v.error.as_deref().unwrap_or_default())
        );
    };
    let size = match (&v.candidate, r.direction) {
        (Candidate::Source(_), Direction::Back) => {
            format!("goes back {}", plural(r.commits.len(), "commit", "commits"))
        }
        (Candidate::Source(_), Direction::Forward) => plural(r.commits.len(), "commit", "commits"),
        _ => plural(r.files.len(), "file", "files"),
    };
    format!(
        "{:<22}{what}   {size}   {}",
        escape(&v.key),
        plural(r.flags.len(), "flag", "flags")
    )
}

fn print_detail(ctx: &Ctx, v: &View) {
    match &v.candidate {
        Candidate::Source(c) => println!(
            "{}  {} → {}  ({})",
            escape(&v.name),
            vault::short(&c.from),
            vault::short(&c.to),
            escape(v.repo.as_deref().unwrap_or("not declared"))
        ),
        Candidate::Skill(c) | Candidate::Agent(c) => {
            println!("{}  from {}", escape(&v.key), escape(&ctx.tilde(&c.origin)));
        }
    }
    if let Some(w) = &v.warning {
        println!("warning: {}", escape(w));
    }
    let Some(r) = &v.review else { return };
    if let Candidate::Source(_) = v.candidate {
        let heading = match r.direction {
            Direction::Forward => format!("commits ({})", r.commits.len()),
            Direction::Back => format!(
                "goes back {} (removed)",
                plural(r.commits.len(), "commit", "commits")
            ),
        };
        println!("\n{heading}");
        for c in &r.commits {
            println!("  {} {}", vault::short(&c.rev), escape(&c.subject));
        }
    }
    println!("\nfiles ({})", r.files.len());
    for f in &r.files {
        let mut extra = String::new();
        if let Some(t) = &f.symlink {
            extra = format!("    -> {}", escape(t));
        } else if f.executable {
            extra = "    (+x)".into();
        }
        let renamed = f
            .old_path
            .as_ref()
            .map(|o| format!("{} → ", escape(o)))
            .unwrap_or_default();
        println!(
            "  {}  {renamed}{}{extra}",
            f.status.letter(),
            escape(&f.path)
        );
    }
    println!("\nflags ({})", r.flags.len());
    for f in &r.flags {
        println!(
            "  {:<14}{:<32} {}",
            f.kind.as_str(),
            escape(&f.path),
            escape(&f.reason)
        );
    }
    if let Some(diff) = &r.diff {
        println!("\ndiff");
        print!("{}", escape_body(diff));
    }
}

/// The JSON object for one candidate (interfaces.md § JSON).
pub(super) fn view_json(v: &View, full: bool) -> Value {
    let (from, to, origin, held_at) = match &v.candidate {
        Candidate::Source(c) => (c.from.clone(), c.to.clone(), None, c.held_at),
        Candidate::Skill(c) | Candidate::Agent(c) => (
            c.base.clone(),
            c.hash.clone(),
            Some(c.origin.display().to_string()),
            c.held_at,
        ),
    };
    let mut out = json!({
        "key": v.key,
        "kind": v.candidate.kind(),
        "name": match &v.candidate {
            Candidate::Source(_) => v.name.clone(),
            _ => v.key.rsplit('/').next().unwrap_or_default().trim_end_matches(".md").to_string(),
        },
        "from": from,
        "to": to,
        "held_at": held_at,
        "error": v.error,
        "warning": v.warning,
    });
    if let Candidate::Source(_) = v.candidate {
        out["repo"] = json!(v.repo);
        out["direction"] = json!(v.review.as_ref().map(|r| r.direction));
    } else {
        out["origin"] = json!(origin);
    }
    let r = v.review.as_ref();
    if full {
        out["commits"] = json!(r.map(|r| r.commits.clone()).unwrap_or_default());
        out["files"] = json!(r.map(|r| r.files.clone()).unwrap_or_default());
        out["flags"] = json!(r.map(|r| r.flags.clone()).unwrap_or_default());
    } else {
        out["flags"] = json!(r.map_or(0, |r| r.flags.len()));
    }
    if let Some(diff) = r.and_then(|r| r.diff.clone()) {
        out["diff"] = json!(diff);
    }
    out
}

/// `colctl review [<name>] [--json]` (AC4-19…22): offline, read-only.
pub fn review(ctx: &Ctx, name: Option<&str>, as_json: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let Some(name) = name else {
        let views: Vec<View> = ws
            .state
            .quarantine
            .iter()
            .map(|(k, c)| inspect(ctx, &ws, &git, k, c, false))
            .collect();
        if as_json {
            let list: Vec<Value> = views.iter().map(|v| view_json(v, true)).collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"version": 1, "candidates": list}))
                    .unwrap_or_default()
            );
        } else if views.is_empty() {
            println!("nothing held for review");
        } else {
            for v in &views {
                println!("{}", summary_line(ctx, v));
            }
        }
        return Ok(if views.is_empty() { 0 } else { EXIT_DRIFT });
    };
    let key = resolve(&ws, name)?;
    let Some(candidate) = ws.state.quarantine.get(&key) else {
        return Err(CliError::usage(format!("nothing held for {name}")));
    };
    let view = inspect(ctx, &ws, &git, &key, candidate, true);
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &json!({"version": 1, "candidates": [view_json(&view, true)]})
            )
            .unwrap_or_default()
        );
    } else {
        print_detail(ctx, &view);
        if let Some(e) = &view.error {
            println!("refused {}: {}", escape(&view.key), escape(e));
        }
    }
    Ok(if view.error.is_some() { EXIT_DRIFT } else { 0 })
}

/// The candidate keys named on the command line, or every held one with `--all` (EC4-02, EC4-03).
fn chosen(
    ws: &Workspace,
    names: &[String],
    all: bool,
    verb: &str,
) -> Result<Vec<String>, CliError> {
    match (names.is_empty(), all) {
        (true, false) => {
            return Err(CliError::usage(format!(
                "name what to {verb} (a source name, skills/<name> or agents/<name>.md), or use --all"
            )));
        }
        (false, true) => {
            return Err(CliError::usage(format!(
                "{verb} takes names or --all, not both"
            )));
        }
        (true, true) => return Ok(ws.state.quarantine.keys().cloned().collect()),
        (false, false) => {}
    }
    let mut keys = Vec::new();
    for name in names {
        let key = resolve(ws, name)?;
        if !ws.state.quarantine.contains_key(&key) {
            return Err(CliError::usage(format!("nothing held for {name}")));
        }
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// Approves one candidate: checks it, then moves the pin or replaces the source copy, and commits.
fn approve_one(
    ctx: &Ctx,
    ws: &Workspace,
    git: &Git,
    st: &mut State,
    config_text: &mut String,
    key: &str,
) -> Result<String, String> {
    let Some(candidate) = st.quarantine.get(key).cloned() else {
        return Err("nothing held".into());
    };
    let view = inspect(ctx, ws, git, key, &candidate, false);
    if let Some(e) = view.error {
        return Err(e);
    }
    let commit_err = |e: CliError| e.message;
    match &candidate {
        Candidate::Source(c) => {
            let name = &view.name;
            if view.warning.is_some() {
                return Err(format!(
                    "{name}'s pin changed since it was held; run colctl update {name} again"
                ));
            }
            let declared = ws
                .config
                .sources
                .iter()
                .find(|s| &s.name == name)
                .ok_or_else(|| format!("no source named {name}"))?;
            let vault = Vault {
                git,
                dir: plugins_dir(ctx),
            };
            match vault.inspect(name, &declared.clone_url(), &declared.rev) {
                CloneState::Dirty => {
                    return Err(format!(
                        "the clone at {} has local changes",
                        ctx.tilde(&vault.path(name))
                    ));
                }
                CloneState::Invalid(reason) => return Err(reason),
                _ => {}
            }
            let text = config::set_source_rev(config_text, name, &c.to)?;
            write_config(ctx, &text).map_err(commit_err)?;
            *config_text = text;
            commit(
                git,
                ctx,
                &format!("colctl: approve source {name} at {}", vault::short(&c.to)),
            )
            .map_err(commit_err)?;
            st.quarantine.remove(key);
            Ok(format!(
                "approved {name}: {} → {}",
                vault::short(&c.from),
                vault::short(&c.to)
            ))
        }
        Candidate::Skill(c) | Candidate::Agent(c) => {
            let is_dir = matches!(candidate, Candidate::Skill(_));
            let item = ctx.source.join(key);
            if hash_item(&item, is_dir).ok().as_deref() != Some(c.base.as_str()) {
                return Err(format!(
                    "changed in both places; {} was edited since {key} was held. Reject it, then refresh",
                    ctx.tilde(&item)
                ));
            }
            let held = state::snapshot_path(&ctx.source, key);
            snapshot(
                &held,
                &staging_for(&item),
                is_dir,
                &c.hash,
                &real_copy(is_dir),
            )?;
            fsx::remove_any(&item)
                .and_then(|()| fs::rename(staging_for(&item), &item).map_err(Into::into))
                .map_err(|e| format!("{e:#}"))?;
            let _ = fsx::remove_any(&held);
            if let Some(o) = st.origins.get_mut(key) {
                o.hash = c.hash.clone();
            }
            st.quarantine.remove(key);
            state::save(&ctx.source, st).map_err(|e| format!("{e:#}"))?;
            let (kind, stem) = if is_dir {
                ("skill", key.trim_start_matches("skills/"))
            } else {
                (
                    "agent",
                    key.trim_start_matches("agents/").trim_end_matches(".md"),
                )
            };
            commit(git, ctx, &format!("colctl: approve {kind} {stem}")).map_err(commit_err)?;
            Ok(format!("approved {key} from {}", ctx.tilde(&c.origin)))
        }
    }
}

/// Where an approved copy is staged before it replaces the source copy.
fn staging_for(item: &Path) -> std::path::PathBuf {
    let name = item
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    item.with_file_name(format!("{}approve-{name}", fsx::TMP_PREFIX))
}

/// `colctl approve` (AC4-23…26): installs exactly the held candidates, one commit each, then syncs.
pub fn approve(ctx: &Ctx, names: &[String], all: bool, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let keys = chosen(&ws, names, all, "approve")?;
    if keys.is_empty() {
        println!("nothing held");
        return Ok(0);
    }
    if dry_run {
        for key in &keys {
            println!(
                "would approve {key}: {}",
                change_text(ctx, &ws.state.quarantine[key])
            );
        }
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    commit_pending(&git, ctx)?;
    let ws = load(ctx)?;
    let mut st = ws.state.clone();
    let mut text = ws.config_text.clone();
    let mut refused = 0;
    let mut approved = Vec::new();
    for key in &keys {
        match approve_one(ctx, &ws, &git, &mut st, &mut text, key) {
            Ok(line) => {
                println!("{line}");
                approved.push(key.clone());
            }
            Err(why) => {
                refused += 1;
                println!("refused {key}: {why}");
            }
        }
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    if approved.is_empty() {
        return Ok(EXIT_DRIFT);
    }
    let code = run_sync(ctx, &git, None)?;
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    for key in approved.iter().filter_map(|k| k.strip_prefix("sources/")) {
        let _ = vault.release(key);
    }
    Ok(if refused > 0 { EXIT_DRIFT } else { code })
}

/// `colctl reject` (AC4-27): drops held candidates and remembers their versions, so `update` and
/// `refresh` don't hold them again.
pub fn reject(ctx: &Ctx, names: &[String], all: bool, dry_run: bool) -> Result<i32, CliError> {
    let git = Git::find(&ctx.path_var)?;
    let ws = load(ctx)?;
    let keys = chosen(&ws, names, all, "reject")?;
    if keys.is_empty() {
        println!("nothing held");
        return Ok(0);
    }
    if dry_run {
        for key in &keys {
            println!(
                "would reject {key}: {}",
                change_text(ctx, &ws.state.quarantine[key])
            );
        }
        return Ok(0);
    }
    let _lock = Lock::acquire(&ctx.source)?;
    let mut st = state::load(&ctx.source)?;
    let vault = Vault {
        git: &git,
        dir: plugins_dir(ctx),
    };
    for key in &keys {
        let Some(candidate) = st.quarantine.remove(key) else {
            continue;
        };
        st.rejected
            .entry(key.clone())
            .or_default()
            .insert(candidate.version().to_string());
        drop_held(ctx, &vault, key);
        println!("rejected {key}: {}", change_text(ctx, &candidate));
    }
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))?;
    Ok(0)
}

/// Deletes what keeps a candidate: the clone's ref, or the snapshot.
fn drop_held(ctx: &Ctx, vault: &Vault, key: &str) {
    match key.strip_prefix("sources/") {
        Some(name) => {
            let _ = vault.release(name);
        }
        None => {
            let _ = fsx::remove_any(&state::snapshot_path(&ctx.source, key));
        }
    }
}

/// Forgets everything held or rejected for an item that's being removed (AC4-28).
pub(super) fn forget(ctx: &Ctx, git: &Git, key: &str) -> Result<(), CliError> {
    let mut st = state::load(&ctx.source)?;
    if !st.quarantine.contains_key(key) && !st.rejected.contains_key(key) {
        return Ok(());
    }
    let vault = Vault {
        git,
        dir: plugins_dir(ctx),
    };
    drop_held(ctx, &vault, key);
    st.forget(key);
    state::save(&ctx.source, &st).map_err(|e| CliError::usage(format!("{e:#}")))
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
